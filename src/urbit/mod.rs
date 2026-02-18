//! Urbit Airlock client for Phase 2+
//!
//! Connects to a local Urbit ship via the Eyre HTTP API (Airlock)
//! and forwards decoded LoRaWAN packets as pokes to %lora-agent.
//!
//! Reference: <https://docs.urbit.org/manual/id/airlock>
//!
//! ## How it works:
//! 1. Authenticate with ship using +code
//! 2. Open an SSE event stream for subscriptions
//! 3. Poke %lora-agent with decoded packet data
//! 4. Subscribe to paths for downlink commands

pub mod types;

use crate::config::UrbitConfig;
use tracing::info;

/// Urbit Airlock client (Phase 2 implementation)
pub struct AirlockClient {
    _config: UrbitConfig,
}

impl AirlockClient {
    /// Create a new Airlock client (does not connect yet)
    pub fn new(config: UrbitConfig) -> Self {
        info!("Urbit Airlock client configured for ship {}", config.ship);
        Self { _config: config }
    }

    // Phase 2 TODOs:
    // - pub async fn connect(&mut self) -> anyhow::Result<()>
    // - pub async fn poke_lora_agent(&self, frame: &LoRaPacket) -> anyhow::Result<()>
    // - pub async fn subscribe(&self, path: &str) -> anyhow::Result<EventStream>
}
