//! LoRaWAN session key management and MIC verification
//!
//! Phase 4: Full key management for Helium integration
//! - NwkSKey for MIC verification and MAC command encryption
//! - AppSKey for application payload decryption
//! - DevAddr ↔ session key mapping

/// Placeholder for session key storage
/// Will be populated in Phase 4 when we need MIC verification
/// for Helium Packet Router integration
#[derive(Debug, Clone)]
pub struct SessionKeys {
    pub dev_addr: u32,
    pub nwk_s_key: [u8; 16],
    pub app_s_key: [u8; 16],
}

/// Session key store — maps DevAddr to session keys
/// In Phase 4, this will be backed by persistent storage
/// and integrated with ChirpStack/Helium device management
#[derive(Debug, Default)]
pub struct KeyStore {
    pub sessions: Vec<SessionKeys>,
}

impl KeyStore {
    pub fn new() -> Self {
        Self {
            sessions: Vec::new(),
        }
    }

    /// Look up session keys by DevAddr
    /// Note: multiple devices can share a DevAddr (multiplexing)
    /// MIC check is used to disambiguate
    pub fn lookup(&self, dev_addr: u32) -> Vec<&SessionKeys> {
        self.sessions
            .iter()
            .filter(|s| s.dev_addr == dev_addr)
            .collect()
    }
}
