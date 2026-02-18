//! Gateway Simulator
//!
//! Simulates a LoRa gateway sending Semtech UDP Packet Forwarder
//! frames to the LoraUrbit server. Useful for testing without hardware.
//!
//! Usage: cargo run --bin gateway-sim [server_addr]

use std::env;
use std::net::SocketAddr;
use tokio::net::UdpSocket;
use tokio::time::{sleep, Duration};

const PROTOCOL_VERSION: u8 = 0x02;
const PUSH_DATA: u8 = 0x00;

/// Fake gateway EUI
const GATEWAY_EUI: [u8; 8] = [0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF, 0x00, 0x11];

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let server_addr: SocketAddr = env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:1680".to_string())
        .parse()?;

    println!("🌊 LoraUrbit Gateway Simulator");
    println!("  Target: {}", server_addr);
    println!("  Gateway EUI: {}", hex::encode(GATEWAY_EUI));
    println!();

    let socket = UdpSocket::bind("0.0.0.0:0").await?;
    let mut token: u16 = 0;

    // Send a mix of packets
    let scenarios = vec![
        ("Unconfirmed Data Up (temperature sensor)", build_unconfirmed_data_up()),
        ("Confirmed Data Up (door sensor)", build_confirmed_data_up()),
        ("Join Request", build_join_request()),
        ("Gateway Status", build_gateway_status()),
        ("Unconfirmed Data Up (humidity sensor)", build_unconfirmed_data_up_2()),
    ];

    for (desc, (rxpk_json, phy_note)) in &scenarios {
        token = token.wrapping_add(1);

        let packet = build_push_data(token, &GATEWAY_EUI, rxpk_json);

        println!("📡 Sending: {}", desc);
        if let Some(note) = phy_note {
            println!("   PHY: {}", note);
        }
        println!("   Size: {} bytes", packet.len());

        socket.send_to(&packet, server_addr).await?;

        // Wait for ACK
        let mut ack_buf = [0u8; 64];
        match tokio::time::timeout(Duration::from_secs(2), socket.recv_from(&mut ack_buf)).await {
            Ok(Ok((len, from))) => {
                if len >= 4 && ack_buf[3] == 0x01 {
                    println!("   ✅ PUSH_ACK received from {}", from);
                } else {
                    println!("   ⚠️  Unexpected response ({} bytes) from {}", len, from);
                }
            }
            Ok(Err(e)) => println!("   ❌ Recv error: {}", e),
            Err(_) => println!("   ⏰ No ACK (timeout)"),
        }
        println!();

        sleep(Duration::from_secs(2)).await;
    }

    println!("✨ Simulation complete!");
    Ok(())
}

fn build_push_data(token: u16, gateway_eui: &[u8; 8], json: &str) -> Vec<u8> {
    let mut packet = Vec::new();
    packet.push(PROTOCOL_VERSION);
    packet.push((token >> 8) as u8);
    packet.push(token as u8);
    packet.push(PUSH_DATA);
    packet.extend_from_slice(gateway_eui);
    packet.extend_from_slice(json.as_bytes());
    packet
}

/// Encode bytes as base64
fn b64(data: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(data)
}

/// Unconfirmed Data Up — simulated temperature sensor
fn build_unconfirmed_data_up() -> (String, Option<String>) {
    // MHDR=0x40 (Unconfirmed Data Up)
    // DevAddr=0x260B1234 (LE: 34 12 0B 26)
    // FCtrl=0x80 (ADR=1)
    // FCnt=0x0042 (LE: 42 00)
    // FPort=0x01
    // Payload: temperature=22.5°C → 0x00E1 (225 in 0.1°C)
    // MIC=0x12345678 (fake)
    let phy: Vec<u8> = vec![
        0x40, 0x34, 0x12, 0x0B, 0x26, 0x80, 0x42, 0x00, 0x01, 0x00, 0xE1, 0x78, 0x56, 0x34, 0x12,
    ];
    let json = format!(
        r#"{{"rxpk":[{{"freq":902.3,"rssi":-65,"lsnr":7.5,"datr":"SF7BW125","codr":"4/5","size":{},"data":"{}"}}]}}"#,
        phy.len(),
        b64(&phy)
    );
    (json, Some("DevAddr=260B1234 FCnt=66 FPort=1 (temp sensor)".to_string()))
}

/// Confirmed Data Up — simulated door sensor
fn build_confirmed_data_up() -> (String, Option<String>) {
    // MHDR=0x80 (Confirmed Data Up)
    // DevAddr=0x260B5678 (LE: 78 56 0B 26)
    // FCtrl=0x00
    // FCnt=0x0007 (LE: 07 00)
    // FPort=0x02
    // Payload: door=open → 0x01
    // MIC=0xAABBCCDD (fake)
    let phy: Vec<u8> = vec![
        0x80, 0x78, 0x56, 0x0B, 0x26, 0x00, 0x07, 0x00, 0x02, 0x01, 0xDD, 0xCC, 0xBB, 0xAA,
    ];
    let json = format!(
        r#"{{"rxpk":[{{"freq":903.9,"rssi":-112,"lsnr":-5.0,"datr":"SF10BW125","codr":"4/5","size":{},"data":"{}"}}]}}"#,
        phy.len(),
        b64(&phy)
    );
    (json, Some("DevAddr=260B5678 FCnt=7 FPort=2 (door sensor)".to_string()))
}

/// Second unconfirmed data up — humidity sensor
fn build_unconfirmed_data_up_2() -> (String, Option<String>) {
    // DevAddr=0x260B1234 (same device as temp)
    // FCnt=0x0043 (next frame)
    // FPort=0x01
    // Payload: humidity=65% → 0x41
    let phy: Vec<u8> = vec![
        0x40, 0x34, 0x12, 0x0B, 0x26, 0x80, 0x43, 0x00, 0x01, 0x41, 0x78, 0x56, 0x34, 0x12,
    ];
    let json = format!(
        r#"{{"rxpk":[{{"freq":902.3,"rssi":-68,"lsnr":6.8,"datr":"SF7BW125","codr":"4/5","size":{},"data":"{}"}}]}}"#,
        phy.len(),
        b64(&phy)
    );
    (json, Some("DevAddr=260B1234 FCnt=67 FPort=1 (humidity)".to_string()))
}

/// Join Request
fn build_join_request() -> (String, Option<String>) {
    // MHDR=0x00 (JoinRequest)
    // AppEUI + DevEUI + DevNonce + MIC = 22 bytes
    let phy: Vec<u8> = vec![
        0x00, // MHDR
        0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, // AppEUI
        0xA1, 0xA2, 0xA3, 0xA4, 0xA5, 0xA6, 0xA7, 0xA8, // DevEUI
        0x42, 0x00, // DevNonce
        0xEF, 0xBE, 0xAD, 0xDE, // MIC
    ];
    let json = format!(
        r#"{{"rxpk":[{{"freq":902.3,"rssi":-90,"lsnr":2.0,"datr":"SF8BW125","codr":"4/5","size":{},"data":"{}"}}]}}"#,
        phy.len(),
        b64(&phy)
    );
    (json, Some("JoinRequest from new device".to_string()))
}

/// Gateway status (no rxpk)
fn build_gateway_status() -> (String, Option<String>) {
    let json = r#"{"stat":{"time":"2026-02-18 17:30:00 UTC","lati":29.7604,"long":-95.3698,"alti":15,"rxnb":47,"rxok":44,"rxfw":44,"ackr":100.0,"dwnb":3,"txnb":3}}"#.to_string();
    (json, None)
}
