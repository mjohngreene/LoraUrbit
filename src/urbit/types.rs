//! Types for Urbit %lora-agent pokes and subscriptions

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A decoded LoRa packet ready to be poked into %lora-agent
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct LoRaPacket {
    /// Device address (from LoRaWAN MAC header)
    pub dev_addr: String,
    /// Frame counter
    pub fcnt: u16,
    /// FPort (application port)
    pub f_port: Option<u8>,
    /// Application payload (hex encoded)
    pub payload: String,
    /// RSSI in dBm
    pub rssi: f64,
    /// Signal-to-noise ratio
    pub snr: Option<f64>,
    /// Frequency in MHz
    pub freq: f64,
    /// Data rate (e.g., "SF7BW125")
    pub data_rate: String,
    /// Gateway EUI that received the packet
    pub gateway_eui: String,
    /// Timestamp of reception
    pub received_at: DateTime<Utc>,
    /// Message type
    pub mtype: String,
    /// Source: "local" (direct gateway) or "helium" (via OUI)
    pub source: PacketSource,
}

/// Where the packet originated
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PacketSource {
    /// Direct from a local LoRa gateway via Semtech UDP
    Local,
    /// Routed through the Helium Network via OUI
    Helium,
}

/// Actions that can be poked into %lora-agent
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action")]
pub enum LoRaAction {
    /// New uplink packet received
    #[serde(rename = "uplink")]
    Uplink(LoRaPacket),

    /// Register a new device
    #[serde(rename = "register-device")]
    RegisterDevice {
        dev_addr: String,
        name: Option<String>,
        description: Option<String>,
    },

    /// Request a downlink to a device
    #[serde(rename = "downlink")]
    Downlink {
        dev_addr: String,
        f_port: u8,
        payload: String, // hex encoded
        confirmed: bool,
    },
}

/// Subscription update from %lora-agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoRaUpdate {
    pub dev_addr: String,
    pub last_seen: DateTime<Utc>,
    pub packet_count: u64,
    pub last_packet: Option<LoRaPacket>,
}

/// An outbound message from the Urbit agent's outbox
///
/// Returned by scrying `/outbox` on %lora-agent.
/// Each message has an ID, destination, payload, and timestamp.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct OutboundMessage {
    /// Unique message ID (assigned by the agent)
    pub id: u64,
    /// Destination Urbit ship (e.g. "~bus")
    pub dest_ship: String,
    /// Destination LoRa device address (e.g. "01AB5678")
    pub dest_addr: String,
    /// Application payload (hex-encoded)
    pub payload: String,
    /// When the message was queued
    pub queued_at: String,
}

/// TX acknowledgment poke — tells the agent a message was sent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TxAck {
    pub action: String,
    #[serde(rename = "msg-id")]
    pub msg_id: u64,
}

impl TxAck {
    pub fn success(msg_id: u64) -> serde_json::Value {
        serde_json::json!({
            "action": "tx-ack",
            "msg-id": msg_id,
        })
    }

    pub fn failure(msg_id: u64) -> serde_json::Value {
        serde_json::json!({
            "action": "tx-fail",
            "msg-id": msg_id,
        })
    }
}
