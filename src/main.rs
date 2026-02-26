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
    let (poke_tx, urbit_config_clone) = if let Some(ref urbit_config) = config.urbit {
        let (tx, rx) = tokio::sync::mpsc::channel::<urbit::types::LoRaPacket>(256);

        // Spawn the Airlock forwarder task (uplink: LoRa → Urbit)
        let airlock_config = urbit_config.clone();
        tokio::spawn(async move {
            if let Err(e) = run_airlock_task(airlock_config, rx).await {
                error!("Airlock task failed: {}", e);
            }
        });

        info!("Urbit bridge enabled (Phase 2)");
        (Some(tx), Some(urbit_config.clone()))
    } else {
        info!("Urbit bridge not configured (Phase 1 mode)");
        (None, None)
    };

    #[cfg(not(feature = "phase2"))]
    let (poke_tx, urbit_config_clone): (
        Option<tokio::sync::mpsc::Sender<urbit::types::LoRaPacket>>,
        Option<config::UrbitConfig>,
    ) = {
        if config.urbit.is_some() {
            info!("Urbit config found but phase2 feature not enabled");
        }
        info!("Running in Phase 1 mode (decode only)");
        (None, None)
    };

    // Phase 4: Initialize Helium client
    if let Some(ref helium_config) = config.helium {
        let _helium = helium::HeliumClient::new(helium_config.clone());
        info!("Helium integration enabled (Phase 4)");
    } else {
        info!("Helium integration not configured");
    }

    // Start the UDP server (Phase 1 core) — returns a DownlinkSender handle
    info!("Starting Semtech UDP Packet Forwarder server...");
    let downlink_sender = udp::start_server(&config, poke_tx).await?;

    // Phase 3a: Spawn outbound message queue (polls Urbit outbox → sends downlinks)
    #[cfg(feature = "phase2")]
    if let Some(urbit_cfg) = urbit_config_clone {
        let dl_sender = downlink_sender.clone();
        tokio::spawn(async move {
            if let Err(e) = run_outbound_task(urbit_cfg, dl_sender).await {
                error!("Outbound task failed: {}", e);
            }
        });
        info!("Outbound message queue enabled (Phase 3a)");
    }

    // Keep the main task alive (the UDP server runs in a background task now)
    info!("Bridge running. Press Ctrl+C to stop.");
    tokio::signal::ctrl_c().await?;
    info!("Shutting down...");

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

        // Poke: device-tracking uplink (also handles peer-to-peer via Hoon agent)
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

        // Note: peer-to-peer message routing is handled in the Hoon
        // agent's %uplink handler. When DevAddr matches a registered peer,
        // the agent automatically routes the message to the inbox.
        // No separate message-received poke needed from the bridge.
    }

    // Channel closed — shutdown
    info!("Packet channel closed, disconnecting Airlock client...");
    client.disconnect().await;
    Ok(())
}

/// Background task that polls the Urbit agent's outbox and sends downlinks
///
/// Phase 3a: Scry the outbox every 2 seconds, convert pending messages to
/// LoRaWAN frames, send as PULL_RESP to the gateway, and poke tx-ack/tx-fail.
#[cfg(feature = "phase2")]
async fn run_outbound_task(
    config: config::UrbitConfig,
    downlink_sender: udp::DownlinkSender,
) -> anyhow::Result<()> {
    use base64::Engine;
    use urbit::types::{OutboundMessage, TxAck};
    use lorawan::encoder::FrameBuilder;
    use udp::build_txpk;

    let agent = config.agent.clone();
    let mut client = urbit::AirlockClient::new(config);

    // Connect with retry
    client.connect_with_retry(5).await?;
    info!("Outbound task connected, polling outbox every 2s...");

    let mut fcnt: u16 = 0; // Frame counter for downlinks (simple incrementing)

    loop {
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;

        // Scry the outbox
        let outbox = match client.scry(&agent, "/outbox").await {
            Ok(val) => val,
            Err(e) => {
                tracing::warn!("Failed to scry outbox: {}", e);

                // If auth expired, try to reconnect
                if !client.is_connected() {
                    info!("Outbound task: attempting reconnect...");
                    if let Err(re) = client.connect_with_retry(3).await {
                        error!("Outbound task reconnect failed: {}", re);
                    }
                }
                continue;
            }
        };

        // Parse the outbox JSON array
        let messages: Vec<OutboundMessage> = match serde_json::from_value(outbox.clone()) {
            Ok(msgs) => msgs,
            Err(_) => {
                // The scry might return nested JSON — try unwrapping common patterns
                if let Some(arr) = outbox.as_array() {
                    match serde_json::from_value(serde_json::Value::Array(arr.clone())) {
                        Ok(msgs) => msgs,
                        Err(e) => {
                            tracing::debug!("No parseable outbox messages: {}", e);
                            continue;
                        }
                    }
                } else {
                    tracing::debug!("Outbox is not an array: {}", outbox);
                    continue;
                }
            }
        };

        if messages.is_empty() {
            continue;
        }

        info!("Outbox has {} pending message(s)", messages.len());

        for msg in &messages {
            info!(
                "Processing outbound msg #{}: dest={} ({}) payload={}",
                msg.id, msg.dest_ship, msg.dest_addr, msg.payload
            );

            // Use the SENDER's DevAddr in the LoRaWAN frame header.
            // This way, the receiving bridge identifies the source of the message.
            // Fall back to dest_addr if src_addr is not set.
            let addr_hex = if !msg.src_addr.is_empty() { &msg.src_addr } else { &msg.dest_addr };
            let dev_addr = match u32::from_str_radix(addr_hex, 16) {
                Ok(addr) => addr,
                Err(e) => {
                    error!("Invalid addr '{}': {}", addr_hex, e);
                    let _ = client.poke(&agent, "json", TxAck::failure(msg.id)).await;
                    continue;
                }
            };

            // Decode the hex payload
            let payload_bytes = match hex::decode(&msg.payload) {
                Ok(bytes) => bytes,
                Err(e) => {
                    error!("Invalid hex payload '{}': {}", msg.payload, e);
                    let _ = client.poke(&agent, "json", TxAck::failure(msg.id)).await;
                    continue;
                }
            };

            // Build the LoRaWAN frame
            let frame = FrameBuilder::new_downlink(dev_addr, fcnt, 1, payload_bytes);
            let frame_bytes = frame.build();
            fcnt = fcnt.wrapping_add(1);

            // Base64 encode for txpk
            let payload_b64 = base64::engine::general_purpose::STANDARD.encode(&frame_bytes);
            let size = frame_bytes.len() as u16;

            // Build txpk and send PULL_RESP
            let txpk = build_txpk(&payload_b64, size);

            match downlink_sender.send_downlink(&txpk).await {
                Ok(()) => {
                    info!("Downlink sent for msg #{}", msg.id);
                    // Poke tx-ack
                    match client.poke(&agent, "json", TxAck::success(msg.id)).await {
                        Ok(()) => {
                            info!("Poked %{} with tx-ack for msg #{}", agent, msg.id);
                        }
                        Err(e) => {
                            error!("Failed to poke tx-ack for msg #{}: {}", msg.id, e);
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to send downlink for msg #{}: {}", msg.id, e);
                    // Poke tx-fail
                    match client.poke(&agent, "json", TxAck::failure(msg.id)).await {
                        Ok(()) => {
                            info!("Poked %{} with tx-fail for msg #{}", agent, msg.id);
                        }
                        Err(e2) => {
                            error!("Failed to poke tx-fail for msg #{}: {}", msg.id, e2);
                        }
                    }
                }
            }
        }
    }
}
