//! What the device answers. Each type reads the body of an answer, and can
//! write it too: that is how the virtual device replies.

use std::fmt;

use crate::{ButtonSet, ProtocolError};

/// The level of a silent microphone, in decibels.
pub const MIC_LEVEL_FLOOR_DB: f32 = -72.2;

fn need(body: &[u8], expected: usize) -> Result<(), ProtocolError> {
    if body.len() < expected {
        return Err(ProtocolError::TooShort {
            expected,
            actual: body.len(),
        });
    }
    Ok(())
}

fn u32_at(body: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([body[at], body[at + 1], body[at + 2], body[at + 3]])
}

/// What the hands are doing on the device right now.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Status {
    pub pressed: ButtonSet,
    /// Pitch, gender, reverb, echo.
    pub encoders: [i8; 4],
    /// Position of each fader, 0 at the bottom, 255 at the top.
    pub faders: [u8; 4],
}

impl Status {
    const LEN: usize = 12;

    pub fn encode(&self) -> Vec<u8> {
        let mut body = Vec::with_capacity(Self::LEN);
        body.extend_from_slice(&self.pressed.bits().to_le_bytes());
        body.extend(self.encoders.map(|value| value as u8));
        body.extend_from_slice(&self.faders);
        body
    }

    pub fn decode(body: &[u8]) -> Result<Self, ProtocolError> {
        need(body, Self::LEN)?;
        Ok(Self {
            pressed: ButtonSet::from_bits(u32_at(body, 0)),
            encoders: [body[4], body[5], body[6], body[7]].map(|value| value as i8),
            faders: [body[8], body[9], body[10], body[11]],
        })
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Version {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
    pub build: u32,
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self {
            major,
            minor,
            patch,
            build,
        } = self;
        write!(f, "{major}.{minor}.{patch}.{build}")
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FirmwareInfo {
    pub firmware: Version,
    pub fpga_count: u32,
    pub dice: Version,
}

impl FirmwareInfo {
    const LEN: usize = 24;

    /// Version fields wider than the wire format allows are truncated.
    pub fn encode(&self) -> Vec<u8> {
        let firmware = ((self.firmware.major & 0xf_ffff) << 12)
            | ((self.firmware.minor & 0xf) << 8)
            | (self.firmware.patch & 0xff);
        let dice = ((self.dice.major & 0xf) << 20)
            | ((self.dice.minor & 0xff) << 12)
            | (self.dice.patch & 0xfff);
        [
            firmware,
            self.firmware.build,
            0,
            self.fpga_count,
            self.dice.build,
            dice,
        ]
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect()
    }

    pub fn decode(body: &[u8]) -> Result<Self, ProtocolError> {
        need(body, Self::LEN)?;
        let firmware = u32_at(body, 0);
        let dice = u32_at(body, 20);
        Ok(Self {
            firmware: Version {
                major: firmware >> 12,
                minor: (firmware >> 8) & 0xf,
                patch: firmware & 0xff,
                build: u32_at(body, 4),
            },
            fpga_count: u32_at(body, 12),
            dice: Version {
                major: (dice >> 20) & 0xf,
                minor: (dice >> 12) & 0xff,
                patch: dice & 0xfff,
                build: u32_at(body, 16),
            },
        })
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SerialInfo {
    pub serial: String,
    /// Manufacturing date, as the device words it.
    pub manufactured: String,
}

impl SerialInfo {
    const SERIAL_LEN: usize = 24;
    const DATE_LEN: usize = 16;

    /// Texts longer than their field are cut.
    pub fn encode(&self) -> Vec<u8> {
        let mut body = vec![0; Self::SERIAL_LEN + Self::DATE_LEN];
        let mut write = |at: usize, len: usize, text: &str| {
            let text = &text.as_bytes()[..text.len().min(len)];
            body[at..at + text.len()].copy_from_slice(text);
        };
        write(0, Self::SERIAL_LEN, &self.serial);
        write(Self::SERIAL_LEN, Self::DATE_LEN, &self.manufactured);
        body
    }

    pub fn decode(body: &[u8]) -> Result<Self, ProtocolError> {
        need(body, Self::SERIAL_LEN)?;
        let text = |field: &[u8]| {
            let end = field
                .iter()
                .position(|byte| *byte == 0)
                .unwrap_or(field.len());
            String::from_utf8_lossy(&field[..end]).into_owned()
        };
        Ok(Self {
            serial: text(&body[..Self::SERIAL_LEN]),
            manufactured: text(&body[Self::SERIAL_LEN..]),
        })
    }
}

pub fn encode_mic_level(level: u16) -> Vec<u8> {
    level.to_le_bytes().to_vec()
}

pub fn decode_mic_level(body: &[u8]) -> Result<u16, ProtocolError> {
    need(body, 2)?;
    Ok(u16::from_le_bytes([body[0], body[1]]))
}

/// Converts a raw microphone level to decibels, between
/// [`MIC_LEVEL_FLOOR_DB`] (silence) and 0 (full scale). Always finite.
pub fn mic_level_db(level: u16) -> f32 {
    if level == 0 {
        return MIC_LEVEL_FLOOR_DB;
    }
    (20.0 * f32::from(level).log10() + MIC_LEVEL_FLOOR_DB).clamp(MIC_LEVEL_FLOOR_DB, 0.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Button;

    #[test]
    fn status_reads_buttons_then_dials_then_faders() {
        let body = [
            0x10, 0x00, 0x80, 0x00, // fader 1 mute and mic mute held
            0xff, 0x02, 0x00, 0x80, // dials: -1, 2, 0, -128
            0, 64, 128, 255, // faders
        ];
        let status = Status::decode(&body).unwrap();
        assert_eq!(
            status.pressed.iter().collect::<Vec<_>>(),
            [Button::Fader1Mute, Button::MicMute]
        );
        assert_eq!(status.encoders, [-1, 2, 0, -128]);
        assert_eq!(status.faders, [0, 64, 128, 255]);
        assert_eq!(status.encode(), body);
    }

    #[test]
    fn status_accepts_trailing_bytes_and_refuses_a_short_body() {
        assert_eq!(Status::decode(&[0; 20]), Ok(Status::default()));
        assert_eq!(
            Status::decode(&[0; 11]),
            Err(ProtocolError::TooShort {
                expected: 12,
                actual: 11
            })
        );
    }

    #[test]
    fn firmware_info_unpacks_both_versions() {
        let body = [
            0x03, 0x14, 0x00, 0x00, // firmware 1.4.3
            110, 0, 0, 0, // build 110
            0, 0, 0, 0, // unused
            7, 0, 0, 0, // fpga count
            110, 0, 0, 0, // dice build
            0x03, 0x40, 0x10, 0x00, // dice 1.4.3
        ];
        let info = FirmwareInfo::decode(&body).unwrap();
        assert_eq!(info.firmware.to_string(), "1.4.3.110");
        assert_eq!(info.fpga_count, 7);
        assert_eq!(info.dice.to_string(), "1.4.3.110");
        assert_eq!(info.encode(), body);
        assert!(FirmwareInfo::decode(&body[..23]).is_err());
    }

    #[test]
    fn serial_info_reads_two_null_terminated_texts() {
        let mut body = vec![0; 40];
        body[..13].copy_from_slice(b"S210401390CQK");
        body[24..34].copy_from_slice(b"2021-05-08");
        let info = SerialInfo::decode(&body).unwrap();
        assert_eq!(info.serial, "S210401390CQK");
        assert_eq!(info.manufactured, "2021-05-08");
        assert_eq!(info.encode(), body);
    }

    #[test]
    fn serial_info_survives_odd_bodies() {
        // No date at all.
        let info = SerialInfo::decode(&[b'A'; 24]).unwrap();
        assert_eq!(info.serial, "A".repeat(24));
        assert_eq!(info.manufactured, "");
        // Not valid UTF-8.
        assert!(SerialInfo::decode(&[0xff; 30]).is_ok());
        assert!(SerialInfo::decode(&[0; 23]).is_err());
        // Too long to fit: cut, not a crash.
        let long = SerialInfo {
            serial: "X".repeat(40),
            manufactured: "Y".repeat(40),
        };
        assert_eq!(long.encode().len(), 40);
    }

    #[test]
    fn mic_level_is_a_little_endian_u16() {
        assert_eq!(decode_mic_level(&[0x34, 0x12]), Ok(0x1234));
        assert_eq!(decode_mic_level(&[0x34, 0x12, 9, 9]), Ok(0x1234));
        assert_eq!(encode_mic_level(0x1234), [0x34, 0x12]);
        assert!(decode_mic_level(&[1]).is_err());
    }

    #[test]
    fn mic_level_in_decibels_is_always_finite_and_bounded() {
        assert_eq!(mic_level_db(0), MIC_LEVEL_FLOOR_DB);
        assert_eq!(mic_level_db(1), MIC_LEVEL_FLOOR_DB);
        assert!((mic_level_db(1000) - -12.2).abs() < 0.01);
        assert_eq!(mic_level_db(u16::MAX), 0.0);
        for level in [0, 1, 2, 100, 4073, 4074, 5000, u16::MAX] {
            let db = mic_level_db(level);
            assert!(db.is_finite() && (MIC_LEVEL_FLOOR_DB..=0.0).contains(&db));
        }
    }
}
