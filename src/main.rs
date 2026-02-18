mod config;
mod helium;
mod lorawan;
mod udp;
mod urbit;

use clap::Parser;
use std::path::PathBuf;
use tracing::info;
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

    // Phase 2: Initialize Urbit Airlock client
    if let Some(ref urbit_config) = config.urbit {
        let _airlock = urbit::AirlockClient::new(urbit_config.clone());
        info!("Urbit bridge enabled (Phase 2)");
    } else {
        info!("Urbit bridge not configured (Phase 1 mode)");
    }

    // Phase 4: Initialize Helium client
    if let Some(ref helium_config) = config.helium {
        let _helium = helium::HeliumClient::new(helium_config.clone());
        info!("Helium integration enabled (Phase 4)");
    } else {
        info!("Helium integration not configured");
    }

    // Start the UDP server (Phase 1 core)
    info!("Starting Semtech UDP Packet Forwarder server...");
    udp::run_server(&config).await?;

    Ok(())
}
