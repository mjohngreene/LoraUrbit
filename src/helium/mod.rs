//! Helium Network integration (Phase 4+)
//!
//! The Helium IoT Network is a decentralized LoRaWAN network with 300k+ hotspots.
//! By purchasing an OUI (Organizationally Unique Identifier, ~$235), LoraUrbit
//! can receive packets from any Helium hotspot and route them through Urbit.
//!
//! ## How Helium routing works:
//! 1. LoRa device sends uplink → nearest Helium hotspot receives it
//! 2. Hotspot forwards to Helium Packet Router
//! 3. Packet Router checks OUI routes for matching DevAddr/EUI
//! 4. Packet Router forwards to our LNS endpoint (LoraUrbit)
//! 5. LoraUrbit decodes and bridges to Urbit via Airlock
//!
//! ## Key concepts:
//! - **OUI**: License to operate an LNS on Helium ($100 one-time)
//! - **DevAddr slab**: Block of 8 device addresses ($100 one-time)
//! - **Data Credits (DC)**: Pay-per-packet, $1 = 100k DC, minimum 3.5M in escrow
//! - **Config Service**: gRPC API to manage routes, devices, and keys
//! - **Net ID**: Helium Foundation's is 0x00003C
//!
//! ## Rust resources from Helium:
//! - helium/gateway-rs — Helium gateway daemon (Rust, tokio-based)
//! - helium/helium-config-service-cli — Config service CLI (Rust)
//! - helium/proto — Protobuf definitions for Helium services
//!
//! Reference: <https://docs.helium.com/iot/run-an-lns/>

pub mod router;

use crate::config::HeliumConfig;
use tracing::info;

/// Helium network client (Phase 4 implementation)
pub struct HeliumClient {
    _config: HeliumConfig,
}

impl HeliumClient {
    pub fn new(config: HeliumConfig) -> Self {
        info!("Helium client configured for OUI {}", config.oui);
        Self { _config: config }
    }

    // Phase 4 TODOs:
    // - pub async fn connect_config_service(&mut self) -> anyhow::Result<()>
    // - pub async fn register_route(&self, endpoint: &str, port: u16) -> anyhow::Result<()>
    // - pub async fn add_device_eui(&self, dev_eui: &str, app_eui: &str) -> anyhow::Result<()>
    // - pub async fn check_dc_balance(&self) -> anyhow::Result<u64>
}
