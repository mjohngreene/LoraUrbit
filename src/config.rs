use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub udp: UdpConfig,
    pub lorawan: LorawanConfig,
    pub urbit: Option<UrbitConfig>,
    pub helium: Option<HeliumConfig>,
    pub logging: LoggingConfig,
}

#[derive(Debug, Deserialize)]
pub struct UdpConfig {
    pub bind: String,
}

#[derive(Debug, Deserialize)]
pub struct LorawanConfig {
    pub decrypt_payload: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UrbitConfig {
    pub url: String,
    pub ship: String,
    pub code: String,
    pub agent: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct HeliumConfig {
    pub oui: u64,
    pub net_id: String,
    pub config_host: String,
    pub delegate_keypair: String,
}

#[derive(Debug, Deserialize)]
pub struct LoggingConfig {
    pub level: String,
}

impl Config {
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| anyhow::anyhow!("Failed to read config file {:?}: {}", path, e))?;
        let config: Config = toml::from_str(&content)
            .map_err(|e| anyhow::anyhow!("Failed to parse config file: {}", e))?;
        Ok(config)
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            udp: UdpConfig {
                bind: "0.0.0.0:1680".to_string(),
            },
            lorawan: LorawanConfig {
                decrypt_payload: false,
            },
            urbit: None,
            helium: None,
            logging: LoggingConfig {
                level: "info".to_string(),
            },
        }
    }
}
