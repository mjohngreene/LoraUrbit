//! LoRaWAN frame encoder for downlink/TX packets
//!
//! Builds raw LoRaWAN MAC frames suitable for transmission.
//! The output bytes are base64-encoded and placed inside the
//! GWMP PULL_RESP txpk JSON.
//!
//! Frame structure (unconfirmed data down):
//!   MHDR(1) | DevAddr(4,LE) | FCtrl(1) | FCnt(2,LE) | [FPort(1) | FRMPayload(N)] | MIC(4,LE)
//!
//! For Phase 3a testing, MIC is set to 0x00000000 (no NwkSKey available).
//! Phase 4 will add proper MIC computation with CMAC-AES128.

use super::MType;

/// Parameters for building a LoRaWAN data frame
#[derive(Debug, Clone)]
pub struct FrameBuilder {
    /// Message type (typically UnconfirmedDataDown for basic downlink)
    pub mtype: MType,
    /// Device address (32-bit)
    pub dev_addr: u32,
    /// Frame counter (16-bit, managed by caller)
    pub fcnt: u16,
    /// FPort (application port, 1-223 for application data)
    pub f_port: u8,
    /// Application payload (raw bytes, unencrypted for Phase 3)
    pub payload: Vec<u8>,
}

impl FrameBuilder {
    /// Create a new frame builder for an unconfirmed downlink
    pub fn new_downlink(dev_addr: u32, fcnt: u16, f_port: u8, payload: Vec<u8>) -> Self {
        Self {
            mtype: MType::UnconfirmedDataDown,
            dev_addr,
            fcnt,
            f_port,
            payload,
        }
    }

    /// Build the raw LoRaWAN PHY payload bytes
    ///
    /// Returns bytes ready for base64 encoding into txpk.data
    pub fn build(&self) -> Vec<u8> {
        let mut frame = Vec::with_capacity(12 + self.payload.len());

        // MHDR: MType(3 bits) | RFU(3 bits) | Major(2 bits)
        // Major = 0b00 (LoRaWAN R1)
        let mhdr = match self.mtype {
            MType::UnconfirmedDataDown => 0x60, // 011_000_00
            MType::ConfirmedDataDown => 0xA0,   // 101_000_00
            MType::UnconfirmedDataUp => 0x40,   // 010_000_00
            MType::ConfirmedDataUp => 0x80,     // 100_000_00
            _ => 0x60, // default to unconfirmed down
        };
        frame.push(mhdr);

        // DevAddr (4 bytes, little-endian)
        frame.extend_from_slice(&self.dev_addr.to_le_bytes());

        // FCtrl: ADR=0, ACK=0, FPending=0, FOptsLen=0
        frame.push(0x00);

        // FCnt (2 bytes, little-endian)
        frame.extend_from_slice(&self.fcnt.to_le_bytes());

        // FPort (only if payload is present)
        if !self.payload.is_empty() {
            frame.push(self.f_port);

            // FRMPayload (raw bytes — no encryption in Phase 3)
            frame.extend_from_slice(&self.payload);
        }

        // MIC (4 bytes, placeholder zeros for Phase 3)
        // Phase 4 will compute CMAC-AES128(NwkSKey, B0 | msg)
        frame.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]);

        frame
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lorawan::{decode_phy_payload, LoRaWANFrame};

    #[test]
    fn test_build_unconfirmed_downlink() {
        let builder = FrameBuilder::new_downlink(
            0x01AB5678,
            42,
            1,
            vec![0x48, 0x65, 0x6C, 0x6C, 0x6F], // "Hello"
        );

        let frame = builder.build();

        // Verify structure:
        // MHDR(1) + DevAddr(4) + FCtrl(1) + FCnt(2) + FPort(1) + Payload(5) + MIC(4) = 18
        assert_eq!(frame.len(), 18);
        assert_eq!(frame[0], 0x60); // UnconfirmedDataDown MHDR

        // DevAddr in little-endian
        assert_eq!(&frame[1..5], &0x01AB5678u32.to_le_bytes());

        // FCtrl
        assert_eq!(frame[5], 0x00);

        // FCnt
        assert_eq!(&frame[6..8], &42u16.to_le_bytes());

        // FPort
        assert_eq!(frame[8], 1);

        // Payload
        assert_eq!(&frame[9..14], &[0x48, 0x65, 0x6C, 0x6C, 0x6F]);

        // MIC (placeholder zeros)
        assert_eq!(&frame[14..18], &[0x00, 0x00, 0x00, 0x00]);
    }

    #[test]
    fn test_build_empty_payload() {
        let builder = FrameBuilder {
            mtype: MType::UnconfirmedDataDown,
            dev_addr: 0x12345678,
            fcnt: 0,
            f_port: 1,
            payload: vec![],
        };

        let frame = builder.build();

        // MHDR(1) + DevAddr(4) + FCtrl(1) + FCnt(2) + MIC(4) = 12 (no FPort, no payload)
        assert_eq!(frame.len(), 12);
    }

    #[test]
    fn test_roundtrip_encode_decode() {
        let builder = FrameBuilder::new_downlink(
            0xDEADBEEF,
            100,
            42,
            vec![0x01, 0x02, 0x03],
        );

        let encoded = builder.build();
        let decoded = decode_phy_payload(&encoded).expect("should decode successfully");

        match decoded {
            LoRaWANFrame::Data {
                mtype,
                dev_addr,
                fcnt,
                f_port,
                frm_payload,
                mic,
                ..
            } => {
                assert_eq!(mtype, MType::UnconfirmedDataDown);
                assert_eq!(dev_addr, 0xDEADBEEF);
                assert_eq!(fcnt, 100);
                assert_eq!(f_port, Some(42));
                assert_eq!(frm_payload, vec![0x01, 0x02, 0x03]);
                assert_eq!(mic, 0x00000000); // placeholder MIC
            }
            _ => panic!("Expected Data frame"),
        }
    }

    #[test]
    fn test_confirmed_downlink() {
        let builder = FrameBuilder {
            mtype: MType::ConfirmedDataDown,
            dev_addr: 0x11223344,
            fcnt: 1,
            f_port: 10,
            payload: vec![0xFF],
        };

        let frame = builder.build();
        assert_eq!(frame[0], 0xA0); // ConfirmedDataDown MHDR

        // Verify it round-trips
        let decoded = decode_phy_payload(&frame).expect("should decode");
        match decoded {
            LoRaWANFrame::Data { mtype, .. } => {
                assert_eq!(mtype, MType::ConfirmedDataDown);
            }
            _ => panic!("Expected Data frame"),
        }
    }
}
