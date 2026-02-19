mod config;
mod helium;
mod lorawan;
mod udp;
mod urbit;

use clap::Parser;
use std::path::PathBuf;
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(name = "lora-urbit")]
#[command(about = "Sovereign LoRaWAN infrastructure powered by Urbit's Ames protocol")]
#[command(version)]
struct Cli {
    /// Path to configuration file
    #[arg(short, long, default_value = "config.toml")]
    config: PathBuf,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Load configuration
    let config = config::Config::load(&cli.config).unwrap_or_else(|e| {
        eprintln!("Warning: Failed to load config from {:?}: {}", cli.config, e);
        eprintln!("Using default configuration");
        config::Config::default()
    });

    // Initialize tracing/logging
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new(&config.logging.level)),
        )
        .init();

    info!("LoraUrbit v{}", env!("CARGO_PKG_VERSION"));
    info!("===========================================");
    info!("Sovereign LoRaWAN ↔ Urbit Ames Bridge");
    info!("===========================================");

    // Phase 2: Set up Urbit Airlock pipeline
    #[cfg(feature = "phase2")]
    let poke_tx = if let Some(ref urbit_config) = config.urbit {
        let (tx, rx) = tokio::sync::mpsc::channel::<urbit::types::LoRaPacket>(256);

        // Spawn the Airlock forwarder task
        let airlock_config = urbit_config.clone();
        tokio::spawn(async move {
            if let Err(e) = run_airlock_task(airlock_config, rx).await {
                error!("Airlock task failed: {}", e);
            }
        });

        info!("Urbit bridge enabled (Phase 2)");
        Some(tx)
    } else {
        info!("Urbit bridge not configured (Phase 1 mode)");
        None
    };

    #[cfg(not(feature = "phase2"))]
    let poke_tx: Option<tokio::sync::mpsc::Sender<urbit::types::LoRaPacket>> = {
        if config.urbit.is_some() {
            info!("Urbit config found but phase2 feature not enabled");
        }
        info!("Running in Phase 1 mode (decode only)");
        None
    };

    // Phase 4: Initialize Helium client
    if let Some(ref helium_config) = config.helium {
        let _helium = helium::HeliumClient::new(helium_config.clone());
        info!("Helium integration enabled (Phase 4)");
    } else {
        info!("Helium integration not configured");
    }

    // Start the UDP server (Phase 1 core)
    info!("Starting Semtech UDP Packet Forwarder server...");
    udp::run_server(&config, poke_tx).await?;

    Ok(())
}

/// Background task that receives decoded LoRa packets and pokes them to Urbit
#[cfg(feature = "phase2")]
async fn run_airlock_task(
    config: config::UrbitConfig,
    mut rx: tokio::sync::mpsc::Receiver<urbit::types::LoRaPacket>,
) -> anyhow::Result<()> {
    use urbit::types::LoRaAction;

    let agent = config.agent.clone();
    let mut client = urbit::AirlockClient::new(config);

    // Connect with retry (up to 5 attempts)
    client.connect_with_retry(5).await?;
    info!("Airlock client connected, waiting for packets...");

    while let Some(packet) = rx.recv().await {
        let dev_addr = packet.dev_addr.clone();

        // Wrap the packet in a LoRaAction::Uplink
        let action = LoRaAction::Uplink(packet);
        let json_data = serde_json::to_value(&action)
            .expect("failed to serialize LoRaAction");

        match client.poke(&agent, "json", json_data).await {
            Ok(()) => {
                info!("Poked %{} with uplink from {}", agent, dev_addr);
            }
            Err(e) => {
                error!(
                    "Failed to poke %{} with uplink from {}: {}",
                    agent, dev_addr, e
                );

                // Try to reconnect for next packet
                if !client.is_connected() {
                    info!("Attempting reconnect for next packet...");
                    if let Err(re) = client.connect_with_retry(3).await {
                        error!("Reconnect failed: {}", re);
                    }
                }
            }
        }
    }

    // Channel closed — shutdown
    info!("Packet channel closed, disconnecting Airlock client...");
    client.disconnect().await;
    Ok(())
}
