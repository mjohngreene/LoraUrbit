pub mod encoder;
pub mod keys;

use std::fmt;

/// LoRaWAN MAC Header (MHDR) - Message Type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MType {
    JoinRequest,
    JoinAccept,
    UnconfirmedDataUp,
    UnconfirmedDataDown,
    ConfirmedDataUp,
    ConfirmedDataDown,
    RejoinRequest,
    Proprietary,
}

impl TryFrom<u8> for MType {
    type Error = anyhow::Error;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match (value >> 5) & 0x07 {
            0b000 => Ok(MType::JoinRequest),
            0b001 => Ok(MType::JoinAccept),
            0b010 => Ok(MType::UnconfirmedDataUp),
            0b011 => Ok(MType::UnconfirmedDataDown),
            0b100 => Ok(MType::ConfirmedDataUp),
            0b101 => Ok(MType::ConfirmedDataDown),
            0b110 => Ok(MType::RejoinRequest),
            0b111 => Ok(MType::Proprietary),
            _ => unreachable!(),
        }
    }
}

impl fmt::Display for MType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MType::JoinRequest => write!(f, "JoinRequest"),
            MType::JoinAccept => write!(f, "JoinAccept"),
            MType::UnconfirmedDataUp => write!(f, "UnconfirmedDataUp"),
            MType::UnconfirmedDataDown => write!(f, "UnconfirmedDataDown"),
            MType::ConfirmedDataUp => write!(f, "ConfirmedDataUp"),
            MType::ConfirmedDataDown => write!(f, "ConfirmedDataDown"),
            MType::RejoinRequest => write!(f, "RejoinRequest"),
            MType::Proprietary => write!(f, "Proprietary"),
        }
    }
}

/// LoRaWAN Major version
#[derive(Debug, Clone, Copy)]
pub enum Major {
    LoRaWANR1,
    Unknown(u8),
}

/// Frame Control byte (FCtrl) for uplink
#[derive(Debug, Clone)]
pub struct FCtrl {
    pub adr: bool,
    pub adr_ack_req: bool,
    pub ack: bool,
    pub class_b: bool,
    pub f_opts_len: u8,
}

/// Decoded LoRaWAN MAC frame
#[derive(Debug, Clone)]
pub enum LoRaWANFrame {
    /// Data frame (up or down)
    Data {
        mtype: MType,
        dev_addr: u32,
        fctrl: FCtrl,
        fcnt: u16,
        f_opts: Vec<u8>,
        f_port: Option<u8>,
        frm_payload: Vec<u8>,
        mic: u32,
    },
    /// Join Request
    JoinRequest {
        app_eui: u64,
        dev_eui: u64,
        dev_nonce: u16,
        mic: u32,
    },
    /// Join Accept (encrypted, not decoded further without keys)
    JoinAccept {
        encrypted_payload: Vec<u8>,
    },
    /// Proprietary frame
    Proprietary {
        payload: Vec<u8>,
    },
}

impl fmt::Display for LoRaWANFrame {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LoRaWANFrame::Data {
                mtype,
                dev_addr,
                fctrl,
                fcnt,
                f_port,
                frm_payload,
                mic,
                ..
            } => {
                write!(
                    f,
                    "{} DevAddr={:08X} FCnt={} FPort={} Payload={} bytes MIC={:08X} ADR={}",
                    mtype,
                    dev_addr,
                    fcnt,
                    f_port.map(|p| p.to_string()).unwrap_or("-".to_string()),
                    frm_payload.len(),
                    mic,
                    fctrl.adr,
                )
            }
            LoRaWANFrame::JoinRequest {
                app_eui,
                dev_eui,
                dev_nonce,
                mic,
            } => {
                write!(
                    f,
                    "JoinRequest AppEUI={:016X} DevEUI={:016X} DevNonce={} MIC={:08X}",
                    app_eui, dev_eui, dev_nonce, mic
                )
            }
            LoRaWANFrame::JoinAccept { encrypted_payload } => {
                write!(
                    f,
                    "JoinAccept (encrypted, {} bytes)",
                    encrypted_payload.len()
                )
            }
            LoRaWANFrame::Proprietary { payload } => {
                write!(f, "Proprietary ({} bytes)", payload.len())
            }
        }
    }
}

/// Decode a LoRaWAN PHY payload (raw bytes after base64 decode)
pub fn decode_phy_payload(data: &[u8]) -> anyhow::Result<LoRaWANFrame> {
    if data.is_empty() {
        return Err(anyhow::anyhow!("Empty PHY payload"));
    }

    let mhdr = data[0];
    let mtype = MType::try_from(mhdr)?;

    match mtype {
        MType::JoinRequest => decode_join_request(data),
        MType::JoinAccept => Ok(LoRaWANFrame::JoinAccept {
            encrypted_payload: data[1..].to_vec(),
        }),
        MType::UnconfirmedDataUp
        | MType::UnconfirmedDataDown
        | MType::ConfirmedDataUp
        | MType::ConfirmedDataDown => decode_data_frame(mtype, data),
        MType::Proprietary => Ok(LoRaWANFrame::Proprietary {
            payload: data[1..].to_vec(),
        }),
        MType::RejoinRequest => Err(anyhow::anyhow!("RejoinRequest not yet supported")),
    }
}

fn decode_join_request(data: &[u8]) -> anyhow::Result<LoRaWANFrame> {
    // MHDR(1) + AppEUI(8) + DevEUI(8) + DevNonce(2) + MIC(4) = 23 bytes
    if data.len() != 23 {
        return Err(anyhow::anyhow!(
            "JoinRequest must be 23 bytes, got {}",
            data.len()
        ));
    }

    let app_eui = u64::from_le_bytes(data[1..9].try_into()?);
    let dev_eui = u64::from_le_bytes(data[9..17].try_into()?);
    let dev_nonce = u16::from_le_bytes(data[17..19].try_into()?);
    let mic = u32::from_le_bytes(data[19..23].try_into()?);

    Ok(LoRaWANFrame::JoinRequest {
        app_eui,
        dev_eui,
        dev_nonce,
        mic,
    })
}

fn decode_data_frame(mtype: MType, data: &[u8]) -> anyhow::Result<LoRaWANFrame> {
    // Minimum: MHDR(1) + DevAddr(4) + FCtrl(1) + FCnt(2) + MIC(4) = 12 bytes
    if data.len() < 12 {
        return Err(anyhow::anyhow!(
            "Data frame too short: {} bytes (minimum 12)",
            data.len()
        ));
    }

    // DevAddr is little-endian
    let dev_addr = u32::from_le_bytes(data[1..5].try_into()?);

    // FCtrl
    let fctrl_byte = data[5];
    let fctrl = FCtrl {
        adr: (fctrl_byte & 0x80) != 0,
        adr_ack_req: (fctrl_byte & 0x40) != 0,
        ack: (fctrl_byte & 0x20) != 0,
        class_b: (fctrl_byte & 0x10) != 0,
        f_opts_len: fctrl_byte & 0x0F,
    };

    // FCnt (16-bit, little-endian)
    let fcnt = u16::from_le_bytes(data[6..8].try_into()?);

    // FOpts
    let f_opts_end = 8 + fctrl.f_opts_len as usize;
    if f_opts_end > data.len() - 4 {
        return Err(anyhow::anyhow!(
            "FOpts length {} exceeds available data",
            fctrl.f_opts_len
        ));
    }
    let f_opts = data[8..f_opts_end].to_vec();

    // FPort + FRMPayload (optional, only present if there's data beyond FOpts + MIC)
    let mic_start = data.len() - 4;
    let (f_port, frm_payload) = if f_opts_end < mic_start {
        let f_port = Some(data[f_opts_end]);
        let frm_payload = data[f_opts_end + 1..mic_start].to_vec();
        (f_port, frm_payload)
    } else {
        (None, vec![])
    };

    // MIC (last 4 bytes)
    let mic = u32::from_le_bytes(data[mic_start..].try_into()?);

    Ok(LoRaWANFrame::Data {
        mtype,
        dev_addr,
        fctrl,
        fcnt,
        f_opts,
        f_port,
        frm_payload,
        mic,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_unconfirmed_data_up() {
        // Minimal unconfirmed data uplink frame
        // MHDR=0x40 (UnconfirmedDataUp, LoRaWAN R1)
        // DevAddr=0x01020304 (LE: 04 03 02 01)
        // FCtrl=0x00 (no ADR, no ACK, FOptsLen=0)
        // FCnt=0x0001 (LE: 01 00)
        // FPort=0x01
        // FRMPayload=0xAA 0xBB
        // MIC=0xDEADBEEF (LE: EF BE AD DE)
        let data: Vec<u8> = vec![
            0x40, // MHDR
            0x04, 0x03, 0x02, 0x01, // DevAddr (LE)
            0x00, // FCtrl
            0x01, 0x00, // FCnt (LE)
            0x01, // FPort
            0xAA, 0xBB, // FRMPayload
            0xEF, 0xBE, 0xAD, 0xDE, // MIC (LE)
        ];

        let frame = decode_phy_payload(&data).unwrap();
        match frame {
            LoRaWANFrame::Data {
                mtype,
                dev_addr,
                fcnt,
                f_port,
                frm_payload,
                mic,
                ..
            } => {
                assert_eq!(mtype, MType::UnconfirmedDataUp);
                assert_eq!(dev_addr, 0x01020304);
                assert_eq!(fcnt, 1);
                assert_eq!(f_port, Some(1));
                assert_eq!(frm_payload, vec![0xAA, 0xBB]);
                assert_eq!(mic, 0xDEADBEEF);
            }
            _ => panic!("Expected Data frame"),
        }
    }

    #[test]
    fn test_decode_join_request() {
        // JoinRequest: MHDR=0x00
        // AppEUI (8 bytes LE) + DevEUI (8 bytes LE) + DevNonce (2 bytes LE) + MIC (4 bytes LE)
        let data: Vec<u8> = vec![
            0x00, // MHDR (JoinRequest)
            0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, // AppEUI
            0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, // DevEUI
            0x42, 0x00, // DevNonce
            0xEF, 0xBE, 0xAD, 0xDE, // MIC
        ];

        let frame = decode_phy_payload(&data).unwrap();
        match frame {
            LoRaWANFrame::JoinRequest {
                dev_nonce, mic, ..
            } => {
                assert_eq!(dev_nonce, 0x0042);
                assert_eq!(mic, 0xDEADBEEF);
            }
            _ => panic!("Expected JoinRequest frame"),
        }
    }

    #[test]
    fn test_empty_payload_fails() {
        let result = decode_phy_payload(&[]);
        assert!(result.is_err());
    }

    #[test]
    fn test_too_short_data_frame_fails() {
        // Only 5 bytes — way too short
        let data: Vec<u8> = vec![0x40, 0x01, 0x02, 0x03, 0x04];
        let result = decode_phy_payload(&data);
        assert!(result.is_err());
    }
}
