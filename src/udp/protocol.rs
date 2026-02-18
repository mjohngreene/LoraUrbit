//! Semtech UDP Packet Forwarder Protocol (GWMP)
//!
//! Reference: https://github.com/Lora-net/packet_forwarder/blob/master/PROTOCOL.TXT
//!
//! The protocol uses a simple binary header followed by JSON payload.
//! All multi-byte integers are big-endian (network byte order).

use bytes::{Buf, BufMut, BytesMut};
use serde::{Deserialize, Serialize};

/// Protocol version (always 0x02)
pub const PROTOCOL_VERSION: u8 = 0x02;

/// Packet types (identifier byte)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PacketType {
    PushData = 0x00,
    PushAck = 0x01,
    PullData = 0x02,
    PullResp = 0x03,
    PullAck = 0x04,
    TxAck = 0x05,
}

impl TryFrom<u8> for PacketType {
    type Error = anyhow::Error;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x00 => Ok(PacketType::PushData),
            0x01 => Ok(PacketType::PushAck),
            0x02 => Ok(PacketType::PullData),
            0x03 => Ok(PacketType::PullResp),
            0x04 => Ok(PacketType::PullAck),
            0x05 => Ok(PacketType::TxAck),
            _ => Err(anyhow::anyhow!("Unknown packet type: 0x{:02x}", value)),
        }
    }
}

/// Gateway identifier (EUI-64, 8 bytes)
pub type GatewayEui = [u8; 8];

/// Parsed GWMP packet
#[derive(Debug)]
pub enum GwmpPacket {
    PushData {
        random_token: u16,
        gateway_eui: GatewayEui,
        json_payload: String,
    },
    PullData {
        random_token: u16,
        gateway_eui: GatewayEui,
    },
    TxAck {
        random_token: u16,
        gateway_eui: GatewayEui,
        json_payload: Option<String>,
    },
}

/// Rxpk (received packet) from gateway JSON payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rxpk {
    /// UTC time of packet reception
    pub time: Option<String>,
    /// GPS time (seconds since GPS epoch)
    pub tmst: Option<u64>,
    /// Concentrator timestamp (microseconds)
    pub tmms: Option<u64>,
    /// RF channel
    pub chan: Option<u8>,
    /// Concentrator IF channel
    pub rfch: Option<u8>,
    /// Frequency in MHz
    pub freq: f64,
    /// LoRa signal-to-noise ratio
    pub lsnr: Option<f64>,
    /// RSSI in dBm
    pub rssi: f64,
    /// Modulation (LORA or FSK)
    pub modu: Option<String>,
    /// LoRa datarate identifier (e.g., "SF7BW125")
    pub datr: String,
    /// LoRa coding rate (e.g., "4/5")
    pub codr: Option<String>,
    /// RF packet payload size in bytes
    pub size: u16,
    /// Base64 encoded RF packet payload
    pub data: String,
}

/// Push data JSON wrapper
#[derive(Debug, Deserialize)]
pub struct PushDataPayload {
    pub rxpk: Option<Vec<Rxpk>>,
    pub stat: Option<serde_json::Value>,
}

impl GwmpPacket {
    /// Parse a raw UDP datagram into a GWMP packet
    pub fn parse(data: &[u8]) -> anyhow::Result<Self> {
        if data.len() < 4 {
            return Err(anyhow::anyhow!("Packet too short: {} bytes", data.len()));
        }

        let mut buf = &data[..];

        let version = buf.get_u8();
        if version != PROTOCOL_VERSION {
            return Err(anyhow::anyhow!(
                "Unsupported protocol version: 0x{:02x}",
                version
            ));
        }

        let random_token = buf.get_u16();
        let packet_type = PacketType::try_from(buf.get_u8())?;

        match packet_type {
            PacketType::PushData => {
                if buf.remaining() < 8 {
                    return Err(anyhow::anyhow!("PUSH_DATA too short for gateway EUI"));
                }
                let mut gateway_eui = [0u8; 8];
                buf.copy_to_slice(&mut gateway_eui);

                let json_payload = String::from_utf8(buf.to_vec())
                    .map_err(|e| anyhow::anyhow!("Invalid UTF-8 in JSON payload: {}", e))?;

                Ok(GwmpPacket::PushData {
                    random_token,
                    gateway_eui,
                    json_payload,
                })
            }
            PacketType::PullData => {
                if buf.remaining() < 8 {
                    return Err(anyhow::anyhow!("PULL_DATA too short for gateway EUI"));
                }
                let mut gateway_eui = [0u8; 8];
                buf.copy_to_slice(&mut gateway_eui);

                Ok(GwmpPacket::PullData {
                    random_token,
                    gateway_eui,
                })
            }
            PacketType::TxAck => {
                if buf.remaining() < 8 {
                    return Err(anyhow::anyhow!("TX_ACK too short for gateway EUI"));
                }
                let mut gateway_eui = [0u8; 8];
                buf.copy_to_slice(&mut gateway_eui);

                let json_payload = if buf.has_remaining() {
                    Some(
                        String::from_utf8(buf.to_vec())
                            .map_err(|e| anyhow::anyhow!("Invalid UTF-8: {}", e))?,
                    )
                } else {
                    None
                };

                Ok(GwmpPacket::TxAck {
                    random_token,
                    gateway_eui,
                    json_payload,
                })
            }
            _ => Err(anyhow::anyhow!(
                "Unexpected packet type for parsing: {:?}",
                packet_type
            )),
        }
    }

    /// Build a PUSH_ACK response
    pub fn push_ack(random_token: u16) -> Vec<u8> {
        let mut buf = BytesMut::with_capacity(4);
        buf.put_u8(PROTOCOL_VERSION);
        buf.put_u16(random_token);
        buf.put_u8(PacketType::PushAck as u8);
        buf.to_vec()
    }

    /// Build a PULL_ACK response
    pub fn pull_ack(random_token: u16) -> Vec<u8> {
        let mut buf = BytesMut::with_capacity(4);
        buf.put_u8(PROTOCOL_VERSION);
        buf.put_u16(random_token);
        buf.put_u8(PacketType::PullAck as u8);
        buf.to_vec()
    }
}
