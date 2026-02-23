pub mod protocol;

use std::net::SocketAddr;
use tokio::net::UdpSocket;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

use crate::config::Config;
use crate::lorawan::{self, LoRaWANFrame};
use crate::urbit::types::{LoRaPacket, PacketSource};
use protocol::{GwmpPacket, PushDataPayload, Rxpk};

/// Run the Semtech UDP Packet Forwarder server
///
/// Decoded LoRaWAN packets are sent to `poke_tx` for forwarding to Urbit.
/// If `poke_tx` is `None`, packets are decoded and logged but not forwarded
/// (Phase 1 mode).
pub async fn run_server(
    config: &Config,
    poke_tx: Option<mpsc::Sender<LoRaPacket>>,
) -> anyhow::Result<()> {
    let socket = UdpSocket::bind(&config.udp.bind).await?;
    info!("UDP server listening on {}", config.udp.bind);

    let mut buf = vec![0u8; 65535];

    loop {
        let (len, src) = socket.recv_from(&mut buf).await?;
        debug!("Received {} bytes from {}", len, src);

        match GwmpPacket::parse(&buf[..len]) {
            Ok(packet) => {
                handle_packet(&socket, src, packet, &poke_tx).await;
            }
            Err(e) => {
                warn!("Failed to parse GWMP packet from {}: {}", src, e);
            }
        }
    }
}

async fn handle_packet(
    socket: &UdpSocket,
    src: SocketAddr,
    packet: GwmpPacket,
    poke_tx: &Option<mpsc::Sender<LoRaPacket>>,
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
            debug!(
                "TX_ACK from gateway {} (token: 0x{:04x}): {:?}",
                gw_eui_hex, random_token, json_payload
            );
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
