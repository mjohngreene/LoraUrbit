pub mod protocol;

use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::UdpSocket;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, error, info, warn};

use crate::config::Config;
use crate::lorawan::{self, LoRaWANFrame};
use crate::urbit::types::{LoRaPacket, PacketSource};
use protocol::{GwmpPacket, PushDataPayload, Rxpk, Txpk, PullRespPayload};

/// Shared state for tracking the gateway's address (learned from PULL_DATA keepalives)
///
/// The gateway sends periodic PULL_DATA packets. The source address from those
/// packets tells us where to send PULL_RESP (downlink) packets.
#[derive(Debug, Clone)]
pub struct GatewayTracker {
    inner: Arc<RwLock<Option<SocketAddr>>>,
}

impl GatewayTracker {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(None)),
        }
    }

    /// Update the tracked gateway address
    pub async fn set(&self, addr: SocketAddr) {
        let mut guard = self.inner.write().await;
        let changed = *guard != Some(addr);
        *guard = Some(addr);
        if changed {
            info!("Gateway address updated: {}", addr);
        }
    }

    /// Get the tracked gateway address (None if no PULL_DATA received yet)
    pub async fn get(&self) -> Option<SocketAddr> {
        *self.inner.read().await
    }
}

/// Handle for sending downlink packets through the UDP socket
///
/// Cloneable handle that the outbound task uses to send PULL_RESP
/// packets to the gateway.
#[derive(Clone)]
pub struct DownlinkSender {
    socket: Arc<UdpSocket>,
    gateway: GatewayTracker,
}

impl DownlinkSender {
    /// Send a PULL_RESP downlink to the tracked gateway
    ///
    /// Returns Ok(()) if sent, Err if no gateway address is known.
    pub async fn send_downlink(&self, txpk: &Txpk) -> anyhow::Result<()> {
        let gw_addr = self.gateway.get().await
            .ok_or_else(|| anyhow::anyhow!("no gateway address known (no PULL_DATA received yet)"))?;

        let payload = PullRespPayload { txpk: txpk.clone() };
        let json = serde_json::to_string(&payload)?;

        // Use a random token for the PULL_RESP
        let token: u16 = rand_token();
        let packet = GwmpPacket::pull_resp(token, &json);

        self.socket.send_to(&packet, gw_addr).await?;
        info!(
            "Sent PULL_RESP to gateway {} (token=0x{:04x}, {} bytes)",
            gw_addr,
            token,
            json.len()
        );

        Ok(())
    }
}

/// Generate a pseudo-random 16-bit token
fn rand_token() -> u16 {
    use std::time::SystemTime;
    let seed = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos();
    (seed & 0xFFFF) as u16
}

/// TX result reported back from the UDP server when we receive TX_ACK
#[derive(Debug, Clone)]
pub enum TxResult {
    /// Gateway confirmed transmission (TX_ACK with no error)
    Success,
    /// Gateway reported a TX error
    Error(String),
}

/// Run the Semtech UDP Packet Forwarder server
///
/// Decoded LoRaWAN packets are sent to `poke_tx` for forwarding to Urbit.
/// If `poke_tx` is `None`, packets are decoded and logged but not forwarded
/// (Phase 1 mode).
///
/// Returns a `DownlinkSender` handle that the outbound task can use to
/// send PULL_RESP packets to the gateway.
pub async fn run_server(
    config: &Config,
    poke_tx: Option<mpsc::Sender<LoRaPacket>>,
) -> anyhow::Result<()> {
    let socket = Arc::new(UdpSocket::bind(&config.udp.bind).await?);
    info!("UDP server listening on {}", config.udp.bind);

    let gateway = GatewayTracker::new();

    let mut buf = vec![0u8; 65535];

    loop {
        let (len, src) = socket.recv_from(&mut buf).await?;
        debug!("Received {} bytes from {}", len, src);

        match GwmpPacket::parse(&buf[..len]) {
            Ok(packet) => {
                handle_packet(&socket, src, packet, &poke_tx, &gateway).await;
            }
            Err(e) => {
                warn!("Failed to parse GWMP packet from {}: {}", src, e);
            }
        }
    }
}

/// Start the UDP server and return a DownlinkSender handle
///
/// Unlike `run_server` which blocks, this spawns the server as a background
/// task and returns immediately with the handle for sending downlinks.
pub async fn start_server(
    config: &Config,
    poke_tx: Option<mpsc::Sender<LoRaPacket>>,
) -> anyhow::Result<DownlinkSender> {
    let socket = Arc::new(UdpSocket::bind(&config.udp.bind).await?);
    info!("UDP server listening on {}", config.udp.bind);

    let gateway = GatewayTracker::new();
    let downlink_sender = DownlinkSender {
        socket: socket.clone(),
        gateway: gateway.clone(),
    };

    // Spawn the receive loop as a background task
    tokio::spawn(async move {
        let mut buf = vec![0u8; 65535];
        loop {
            match socket.recv_from(&mut buf).await {
                Ok((len, src)) => {
                    debug!("Received {} bytes from {}", len, src);
                    match GwmpPacket::parse(&buf[..len]) {
                        Ok(packet) => {
                            handle_packet(&socket, src, packet, &poke_tx, &gateway).await;
                        }
                        Err(e) => {
                            warn!("Failed to parse GWMP packet from {}: {}", src, e);
                        }
                    }
                }
                Err(e) => {
                    error!("UDP recv error: {}", e);
                }
            }
        }
    });

    Ok(downlink_sender)
}

async fn handle_packet(
    socket: &UdpSocket,
    src: SocketAddr,
    packet: GwmpPacket,
    poke_tx: &Option<mpsc::Sender<LoRaPacket>>,
    gateway: &GatewayTracker,
) {
    match packet {
        GwmpPacket::PushData {
            random_token,
            gateway_eui,
            json_payload,
        } => {
            let gw_eui_hex = hex::encode(gateway_eui);
            info!(
                "PUSH_DATA from gateway {} (token: 0x{:04x})",
                gw_eui_hex, random_token
            );

            // Send ACK immediately
            let ack = GwmpPacket::push_ack(random_token);
            if let Err(e) = socket.send_to(&ack, src).await {
                error!("Failed to send PUSH_ACK to {}: {}", src, e);
            }

            // Parse the JSON payload
            match serde_json::from_str::<PushDataPayload>(&json_payload) {
                Ok(payload) => {
                    if let Some(rxpks) = payload.rxpk {
                        for rxpk in rxpks {
                            info!(
                                "  rxpk: freq={} MHz, rssi={} dBm, datr={}, size={} bytes",
                                rxpk.freq, rxpk.rssi, rxpk.datr, rxpk.size
                            );

                            // Decode the LoRaWAN PHY payload
                            match base64_decode(&rxpk.data) {
                                Ok(phy_payload) => {
                                    match lorawan::decode_phy_payload(&phy_payload) {
                                        Ok(frame) => {
                                            info!("  LoRaWAN: {}", frame);

                                            // Forward to Urbit via mpsc channel
                                            if let Some(tx) = poke_tx {
                                                if let Some(lora_pkt) = frame_to_lora_packet(
                                                    &frame,
                                                    &rxpk,
                                                    &gw_eui_hex,
                                                ) {
                                                    if let Err(e) = tx.send(lora_pkt).await {
                                                        error!(
                                                            "Failed to forward packet to Airlock task: {}",
                                                            e
                                                        );
                                                    }
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            warn!("  Failed to decode LoRaWAN frame: {}", e);
                                        }
                                    }
                                }
                                Err(e) => {
                                    warn!("  Failed to base64 decode rxpk data: {}", e);
                                }
                            }
                        }
                    }

                    if let Some(stat) = payload.stat {
                        debug!("  Gateway status: {}", stat);
                    }
                }
                Err(e) => {
                    warn!("Failed to parse PUSH_DATA JSON: {}", e);
                    debug!("  Raw JSON: {}", json_payload);
                }
            }
        }
        GwmpPacket::PullData {
            random_token,
            gateway_eui,
        } => {
            let gw_eui_hex = hex::encode(gateway_eui);
            debug!(
                "PULL_DATA from gateway {} (token: 0x{:04x})",
                gw_eui_hex, random_token
            );

            // Track the gateway address for downlink delivery
            gateway.set(src).await;

            let ack = GwmpPacket::pull_ack(random_token);
            if let Err(e) = socket.send_to(&ack, src).await {
                error!("Failed to send PULL_ACK to {}: {}", src, e);
            }
        }
        GwmpPacket::TxAck {
            random_token,
            gateway_eui,
            json_payload,
        } => {
            let gw_eui_hex = hex::encode(gateway_eui);

            // Check for TX errors in the payload
            if let Some(ref json) = json_payload {
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(json) {
                    if let Some(txpk_ack) = parsed.get("txpk_ack") {
                        let error = txpk_ack.get("error").and_then(|e| e.as_str());
                        match error {
                            None | Some("NONE") => {
                                info!(
                                    "TX_ACK from gateway {} (token: 0x{:04x}): SUCCESS",
                                    gw_eui_hex, random_token
                                );
                            }
                            Some(err) => {
                                warn!(
                                    "TX_ACK from gateway {} (token: 0x{:04x}): ERROR: {}",
                                    gw_eui_hex, random_token, err
                                );
                            }
                        }
                    } else {
                        // TX_ACK with no txpk_ack field — treat as success
                        info!(
                            "TX_ACK from gateway {} (token: 0x{:04x}): OK (no txpk_ack)",
                            gw_eui_hex, random_token
                        );
                    }
                }
            } else {
                // TX_ACK with no JSON payload — treat as success
                info!(
                    "TX_ACK from gateway {} (token: 0x{:04x}): OK",
                    gw_eui_hex, random_token
                );
            }
        }
        GwmpPacket::PushAck { random_token } => {
            debug!("PUSH_ACK (token: 0x{:04x})", random_token);
        }
        GwmpPacket::PullAck { random_token } => {
            debug!("PULL_ACK (token: 0x{:04x})", random_token);
        }
        GwmpPacket::PullResp {
            random_token,
            json_payload,
        } => {
            debug!(
                "PULL_RESP (token: 0x{:04x}): {} bytes",
                random_token,
                json_payload.len()
            );
        }
    }
}

/// Convert a decoded LoRaWAN frame + rxpk metadata into a LoRaPacket for Urbit
fn frame_to_lora_packet(
    frame: &LoRaWANFrame,
    rxpk: &Rxpk,
    gateway_eui: &str,
) -> Option<LoRaPacket> {
    match frame {
        LoRaWANFrame::Data {
            mtype,
            dev_addr,
            fcnt,
            f_port,
            frm_payload,
            ..
        } => Some(LoRaPacket {
            dev_addr: format!("{:08X}", dev_addr),
            fcnt: *fcnt,
            f_port: *f_port,
            payload: hex::encode(frm_payload),
            rssi: rxpk.rssi,
            snr: rxpk.lsnr,
            freq: rxpk.freq,
            data_rate: rxpk.datr.clone(),
            gateway_eui: gateway_eui.to_string(),
            received_at: chrono::Utc::now(),
            mtype: mtype.to_string(),
            source: PacketSource::Local,
        }),
        // JoinRequest, JoinAccept, Proprietary — skip for now
        _ => {
            debug!("Skipping non-data frame for Urbit forwarding");
            None
        }
    }
}

fn base64_decode(input: &str) -> anyhow::Result<Vec<u8>> {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD
        .decode(input)
        .map_err(|e| anyhow::anyhow!("Base64 decode error: {}", e))
}

/// Build a Txpk for a downlink transmission
///
/// Uses US915 Class C defaults: RX2 frequency 923.3 MHz, SF12BW500, 27 dBm.
/// Immediate mode (imme=true) for Class C devices.
pub fn build_txpk(payload_b64: &str, payload_size: u16) -> Txpk {
    Txpk {
        imme: Some(true),          // Immediate TX (Class C)
        tmst: None,                // No timestamp (immediate mode)
        freq: 923.3,               // US915 RX2 frequency
        rfch: Some(0),             // RF chain 0
        powe: Some(27),            // 27 dBm (US915 max EIRP)
        modu: Some("LORA".to_string()),
        datr: "SF12BW500".to_string(), // US915 RX2 default
        codr: Some("4/5".to_string()),
        ipol: Some(true),          // Inverted polarity for downlink
        size: payload_size,
        data: payload_b64.to_string(),
        ncrc: Some(true),          // No CRC for downlink
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gateway_tracker() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let tracker = GatewayTracker::new();
            assert!(tracker.get().await.is_none());

            let addr: SocketAddr = "127.0.0.1:1700".parse().unwrap();
            tracker.set(addr).await;
            assert_eq!(tracker.get().await, Some(addr));

            // Update with new address
            let addr2: SocketAddr = "127.0.0.1:1701".parse().unwrap();
            tracker.set(addr2).await;
            assert_eq!(tracker.get().await, Some(addr2));
        });
    }

    #[test]
    fn test_build_txpk() {
        let txpk = build_txpk("AQIDBA==", 4);
        assert_eq!(txpk.freq, 923.3);
        assert_eq!(txpk.imme, Some(true));
        assert_eq!(txpk.ipol, Some(true));
        assert_eq!(txpk.datr, "SF12BW500");
        assert_eq!(txpk.data, "AQIDBA==");
        assert_eq!(txpk.size, 4);
    }
}
