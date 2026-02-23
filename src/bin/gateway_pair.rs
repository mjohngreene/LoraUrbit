//! Gateway Pair Simulator
//!
//! Simulates two LoRa gateways linked by a local UDP pipe.
//! Each gateway speaks Semtech GWMP to its bridge and relays
//! uplinks to the other gateway for delivery to the other bridge.
//!
//! Topology:
//!   Bridge A (1680) ↔ Gateway A (1700) ══ Gateway B (1701) ↔ Bridge B (1681)
//!
//! Gateway A receives PUSH_DATA from Bridge A, ACKs it, then re-wraps
//! the payload as a new PUSH_DATA and sends it to Gateway B's bridge
//! (Bridge B at port 1681). Gateway B does the reverse.
//!
//! Each gateway also:
//! - Sends periodic PULL_DATA keepalives to its bridge
//! - Accepts PULL_RESP (downlinks) from its bridge and relays them
//!   to the other gateway's bridge as PUSH_DATA (simulating the
//!   radio path: downlink on one side = uplink on the other)
//!
//! Usage:
//!   cargo run --bin gateway-pair
//!   cargo run --bin gateway-pair -- [options]
//!
//! Options (env vars or defaults):
//!   GW_A_BIND=0.0.0.0:1700       Gateway A listen address
//!   GW_B_BIND=0.0.0.0:1701       Gateway B listen address
//!   BRIDGE_A_ADDR=127.0.0.1:1680 Bridge A address
//!   BRIDGE_B_ADDR=127.0.0.1:1681 Bridge B address

use std::env;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::UdpSocket;
use tokio::sync::Mutex;
use tokio::time::{interval, Duration};

const PROTOCOL_VERSION: u8 = 0x02;

// Packet type identifiers
const PUSH_DATA: u8 = 0x00;
const PUSH_ACK: u8 = 0x01;
const PULL_DATA: u8 = 0x02;
const PULL_RESP: u8 = 0x03;
const PULL_ACK: u8 = 0x04;
const TX_ACK: u8 = 0x05;

/// Gateway EUIs — distinct so bridges can tell them apart
const GATEWAY_A_EUI: [u8; 8] = [0xAA, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01];
const GATEWAY_B_EUI: [u8; 8] = [0xBB, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02];

/// State for one side of the gateway pair
struct GatewayState {
    /// Last known bridge address (updated from PULL_DATA or PUSH_DATA)
    bridge_addr: Option<SocketAddr>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let gw_a_bind: SocketAddr = env::var("GW_A_BIND")
        .unwrap_or_else(|_| "0.0.0.0:1700".to_string())
        .parse()?;
    let gw_b_bind: SocketAddr = env::var("GW_B_BIND")
        .unwrap_or_else(|_| "0.0.0.0:1701".to_string())
        .parse()?;
    let bridge_a_addr: SocketAddr = env::var("BRIDGE_A_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:1680".to_string())
        .parse()?;
    let bridge_b_addr: SocketAddr = env::var("BRIDGE_B_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:1681".to_string())
        .parse()?;

    println!("🌊 LoraUrbit Gateway Pair Simulator");
    println!("══════════════════════════════════════════");
    println!("  Gateway A: {} (EUI: {})", gw_a_bind, hex::encode(GATEWAY_A_EUI));
    println!("    → Bridge A: {}", bridge_a_addr);
    println!("  Gateway B: {} (EUI: {})", gw_b_bind, hex::encode(GATEWAY_B_EUI));
    println!("    → Bridge B: {}", bridge_b_addr);
    println!("══════════════════════════════════════════");
    println!("  Relay: Gateway A ←→ Gateway B (localhost)");
    println!();

    // Bind both gateway sockets
    let sock_a = Arc::new(UdpSocket::bind(gw_a_bind).await?);
    let sock_b = Arc::new(UdpSocket::bind(gw_b_bind).await?);

    println!("✅ Gateway A listening on {}", gw_a_bind);
    println!("✅ Gateway B listening on {}", gw_b_bind);
    println!();

    let state_a = Arc::new(Mutex::new(GatewayState { bridge_addr: None }));
    let state_b = Arc::new(Mutex::new(GatewayState { bridge_addr: None }));

    // Token counter for generated packets (shared across tasks)
    let token_counter = Arc::new(std::sync::atomic::AtomicU16::new(0x1000));

    // Spawn Gateway A receiver
    let sa = sock_a.clone();
    let sb = sock_b.clone();
    let sta = state_a.clone();
    let stb = state_b.clone();
    let tc = token_counter.clone();
    tokio::spawn(async move {
        gateway_recv_loop("A", &GATEWAY_A_EUI, sa, sb, sta, stb, bridge_b_addr, tc).await;
    });

    // Spawn Gateway B receiver
    let sa = sock_a.clone();
    let sb = sock_b.clone();
    let sta = state_a.clone();
    let stb = state_b.clone();
    let tc = token_counter.clone();
    tokio::spawn(async move {
        gateway_recv_loop("B", &GATEWAY_B_EUI, sb, sa, stb, sta, bridge_a_addr, tc).await;
    });

    // Spawn PULL_DATA keepalive senders
    let sa = sock_a.clone();
    let sta = state_a.clone();
    let tc = token_counter.clone();
    tokio::spawn(async move {
        keepalive_loop("A", &GATEWAY_A_EUI, sa, sta, bridge_a_addr, tc).await;
    });

    let sb = sock_b.clone();
    let stb = state_b.clone();
    let tc = token_counter.clone();
    tokio::spawn(async move {
        keepalive_loop("B", &GATEWAY_B_EUI, sb, stb, bridge_b_addr, tc).await;
    });

    println!("🔄 Gateway pair running. Press Ctrl+C to stop.\n");

    // Wait forever
    tokio::signal::ctrl_c().await?;
    println!("\n👋 Gateway pair shutting down.");
    Ok(())
}

/// Main receive loop for one gateway
///
/// - `name`: "A" or "B" (for logging)
/// - `my_eui`: this gateway's EUI
/// - `my_sock`: this gateway's socket
/// - `peer_sock`: the other gateway's socket
/// - `my_state`: this gateway's state
/// - `peer_state`: the other gateway's state
/// - `peer_bridge_addr`: the other side's bridge address (for relay)
/// - `token_counter`: shared counter for generated packet tokens
async fn gateway_recv_loop(
    name: &str,
    my_eui: &[u8; 8],
    my_sock: Arc<UdpSocket>,
    peer_sock: Arc<UdpSocket>,
    my_state: Arc<Mutex<GatewayState>>,
    peer_state: Arc<Mutex<GatewayState>>,
    peer_bridge_default: SocketAddr,
    token_counter: Arc<std::sync::atomic::AtomicU16>,
) {
    let mut buf = vec![0u8; 65535];

    loop {
        let (len, src) = match my_sock.recv_from(&mut buf).await {
            Ok(r) => r,
            Err(e) => {
                eprintln!("[GW-{}] recv error: {}", name, e);
                continue;
            }
        };

        let data = &buf[..len];
        if len < 4 {
            eprintln!("[GW-{}] packet too short ({} bytes) from {}", name, len, src);
            continue;
        }

        let _version = data[0];
        let token = u16::from_be_bytes([data[1], data[2]]);
        let ptype = data[3];

        match ptype {
            PUSH_DATA => {
                // Uplink from our bridge — ACK it and relay to the other bridge
                if len < 12 {
                    eprintln!("[GW-{}] PUSH_DATA too short from {}", name, src);
                    continue;
                }

                let gw_eui = &data[4..12];
                let json_payload = &data[12..];

                println!(
                    "[GW-{}] 📥 PUSH_DATA from {} (gw_eui={}, {} bytes payload)",
                    name,
                    src,
                    hex::encode(gw_eui),
                    json_payload.len()
                );

                // Update our bridge address
                {
                    let mut state = my_state.lock().await;
                    state.bridge_addr = Some(src);
                }

                // Send PUSH_ACK back to the bridge
                let ack = build_push_ack(token);
                if let Err(e) = my_sock.send_to(&ack, src).await {
                    eprintln!("[GW-{}] failed to send PUSH_ACK: {}", name, e);
                }

                // Relay: re-wrap as PUSH_DATA with our peer's EUI and send to peer's bridge
                let peer_eui = if name == "A" { &GATEWAY_B_EUI } else { &GATEWAY_A_EUI };
                let relay_token = token_counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

                // Determine peer bridge address: use stored address or default
                let peer_bridge = {
                    let state = peer_state.lock().await;
                    state.bridge_addr.unwrap_or(peer_bridge_default)
                };

                let relay_pkt = build_push_data(relay_token, peer_eui, json_payload);
                match peer_sock.send_to(&relay_pkt, peer_bridge).await {
                    Ok(_) => {
                        println!(
                            "[GW-{}] 📤 Relayed to peer bridge {} (token=0x{:04x})",
                            name, peer_bridge, relay_token
                        );
                    }
                    Err(e) => {
                        eprintln!("[GW-{}] failed to relay to peer bridge: {}", name, e);
                    }
                }
            }

            PUSH_ACK => {
                // ACK from a relay or bridge — ignore (we don't wait for these)
            }

            PULL_ACK => {
                // ACK from bridge in response to our PULL_DATA keepalive — ignore
            }

            PULL_DATA => {
                // Keepalive from our bridge — record address, send PULL_ACK
                if len < 12 {
                    eprintln!("[GW-{}] PULL_DATA too short from {}", name, src);
                    continue;
                }

                // Update bridge address
                {
                    let mut state = my_state.lock().await;
                    state.bridge_addr = Some(src);
                }

                let ack = build_pull_ack(token);
                if let Err(e) = my_sock.send_to(&ack, src).await {
                    eprintln!("[GW-{}] failed to send PULL_ACK: {}", name, e);
                }
            }

            PULL_RESP => {
                // Downlink from our bridge — "transmit" it
                // In real hardware this would go out over RF.
                // In simulation, we relay it to the other gateway's bridge
                // as a PUSH_DATA (because a downlink on side A = an uplink on side B).
                let json_payload = &data[4..];

                println!(
                    "[GW-{}] 📩 PULL_RESP (downlink) from {} ({} bytes)",
                    name,
                    src,
                    json_payload.len()
                );

                // Parse the txpk to extract the RF payload and re-wrap as rxpk
                match serde_json::from_slice::<serde_json::Value>(json_payload) {
                    Ok(pull_resp_json) => {
                        if let Some(txpk) = pull_resp_json.get("txpk") {
                            // Convert txpk → rxpk for the other side
                            let rxpk_json = txpk_to_rxpk(txpk, my_eui);

                            let peer_eui = if name == "A" { &GATEWAY_B_EUI } else { &GATEWAY_A_EUI };
                            let relay_token = token_counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

                            let peer_bridge = {
                                let state = peer_state.lock().await;
                                state.bridge_addr.unwrap_or(peer_bridge_default)
                            };

                            let relay_pkt = build_push_data(relay_token, peer_eui, rxpk_json.as_bytes());
                            match peer_sock.send_to(&relay_pkt, peer_bridge).await {
                                Ok(_) => {
                                    println!(
                                        "[GW-{}] 📤 Downlink relayed as uplink to peer bridge {} (token=0x{:04x})",
                                        name, peer_bridge, relay_token
                                    );
                                }
                                Err(e) => {
                                    eprintln!("[GW-{}] failed to relay downlink: {}", name, e);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("[GW-{}] failed to parse PULL_RESP JSON: {}", name, e);
                    }
                }

                // Send TX_ACK back to our bridge (success)
                let tx_ack = build_tx_ack(token, my_eui);
                if let Err(e) = my_sock.send_to(&tx_ack, src).await {
                    eprintln!("[GW-{}] failed to send TX_ACK: {}", name, e);
                }
            }

            TX_ACK => {
                // Should not receive TX_ACK on a gateway socket (gateways SEND these)
                eprintln!("[GW-{}] unexpected TX_ACK from {}", name, src);
            }

            _ => {
                eprintln!("[GW-{}] unknown packet type 0x{:02x} from {}", name, ptype, src);
            }
        }
    }
}

/// Periodically send PULL_DATA keepalives to the bridge
async fn keepalive_loop(
    name: &str,
    eui: &[u8; 8],
    sock: Arc<UdpSocket>,
    state: Arc<Mutex<GatewayState>>,
    bridge_default: SocketAddr,
    token_counter: Arc<std::sync::atomic::AtomicU16>,
) {
    let mut tick = interval(Duration::from_secs(10));

    loop {
        tick.tick().await;

        let bridge_addr = {
            let s = state.lock().await;
            s.bridge_addr.unwrap_or(bridge_default)
        };

        let token = token_counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let pkt = build_pull_data(token, eui);

        match sock.send_to(&pkt, bridge_addr).await {
            Ok(_) => {} // Silent keepalives — don't spam the console
            Err(e) => {
                eprintln!("[GW-{}] keepalive failed: {}", name, e);
            }
        }
    }
}

/// Convert a txpk JSON object to an rxpk JSON string (for relay)
///
/// When Gateway A receives a PULL_RESP (downlink), it "transmits" the
/// packet over RF. Gateway B "receives" it as an uplink. So we convert
/// the txpk fields to rxpk format.
fn txpk_to_rxpk(txpk: &serde_json::Value, _source_gw_eui: &[u8; 8]) -> String {
    let freq = txpk.get("freq").and_then(|v| v.as_f64()).unwrap_or(902.3);
    let datr = txpk.get("datr").and_then(|v| v.as_str()).unwrap_or("SF7BW125");
    let codr = txpk.get("codr").and_then(|v| v.as_str()).unwrap_or("4/5");
    let size = txpk.get("size").and_then(|v| v.as_u64()).unwrap_or(0) as u16;
    let data = txpk.get("data").and_then(|v| v.as_str()).unwrap_or("");

    // Simulate reasonable RX parameters
    let rxpk = serde_json::json!({
        "rxpk": [{
            "freq": freq,
            "rssi": -60,        // simulated good signal
            "lsnr": 8.0,        // simulated good SNR
            "datr": datr,
            "codr": codr,
            "size": size,
            "data": data,
            "modu": "LORA",
            "tmst": 0,          // immediate
        }]
    });

    rxpk.to_string()
}

// ── Raw packet builders (minimal, no external deps) ──────────────

fn build_push_ack(token: u16) -> Vec<u8> {
    vec![
        PROTOCOL_VERSION,
        (token >> 8) as u8,
        token as u8,
        PUSH_ACK,
    ]
}

fn build_pull_ack(token: u16) -> Vec<u8> {
    vec![
        PROTOCOL_VERSION,
        (token >> 8) as u8,
        token as u8,
        PULL_ACK,
    ]
}

fn build_push_data(token: u16, gateway_eui: &[u8; 8], json_payload: &[u8]) -> Vec<u8> {
    let mut pkt = Vec::with_capacity(12 + json_payload.len());
    pkt.push(PROTOCOL_VERSION);
    pkt.push((token >> 8) as u8);
    pkt.push(token as u8);
    pkt.push(PUSH_DATA);
    pkt.extend_from_slice(gateway_eui);
    pkt.extend_from_slice(json_payload);
    pkt
}

fn build_pull_data(token: u16, gateway_eui: &[u8; 8]) -> Vec<u8> {
    let mut pkt = Vec::with_capacity(12);
    pkt.push(PROTOCOL_VERSION);
    pkt.push((token >> 8) as u8);
    pkt.push(token as u8);
    pkt.push(PULL_DATA);
    pkt.extend_from_slice(gateway_eui);
    pkt
}

fn build_tx_ack(token: u16, gateway_eui: &[u8; 8]) -> Vec<u8> {
    let mut pkt = Vec::with_capacity(12);
    pkt.push(PROTOCOL_VERSION);
    pkt.push((token >> 8) as u8);
    pkt.push(token as u8);
    pkt.push(TX_ACK);
    pkt.extend_from_slice(gateway_eui);
    pkt
}
