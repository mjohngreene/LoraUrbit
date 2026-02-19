//! Urbit Airlock client for Phase 2+
//!
//! Connects to a local Urbit ship via the Eyre HTTP API (Airlock)
//! and forwards decoded LoRaWAN packets as pokes to %lora-agent.
//!
//! Reference: <https://docs.urbit.org/manual/id/airlock>
//!
//! ## How it works:
//! 1. Authenticate with ship using +code
//! 2. Poke %lora-agent with decoded packet data
//! 3. ACK events to keep the channel healthy

pub mod types;

#[cfg(feature = "phase2")]
pub mod airlock;

#[cfg(feature = "phase2")]
pub use airlock::AirlockClient;

#[cfg(not(feature = "phase2"))]
mod stub {
    use crate::config::UrbitConfig;
    use tracing::info;

    /// Stub Airlock client when phase2 feature is not enabled
    pub struct AirlockClient {
        _config: UrbitConfig,
    }

    impl AirlockClient {
        pub fn new(config: UrbitConfig) -> Self {
            info!(
                "Urbit Airlock client configured for ship {} (stub — enable phase2 feature)",
                config.ship
            );
            Self { _config: config }
        }
    }
}

#[cfg(not(feature = "phase2"))]
pub use stub::AirlockClient;
