use crate::ProtocolError;

/// Every packet, in both directions, starts with a header of this size.
pub const HEADER_LEN: usize = 16;

/// A command or its answer as it travels on the wire.
///
/// Header layout, little-endian: command id (4 bytes), body length (2),
/// command index (2), then 8 unused bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Packet {
    pub command_id: u32,
    /// Counts the commands of a session; the answer repeats it.
    pub index: u16,
    pub body: Vec<u8>,
}

impl Packet {
    pub fn encode(&self) -> Vec<u8> {
        debug_assert!(self.body.len() <= usize::from(u16::MAX));
        let mut bytes = Vec::with_capacity(HEADER_LEN + self.body.len());
        bytes.extend_from_slice(&self.command_id.to_le_bytes());
        bytes.extend_from_slice(&(self.body.len() as u16).to_le_bytes());
        bytes.extend_from_slice(&self.index.to_le_bytes());
        bytes.extend_from_slice(&[0; 8]);
        bytes.extend_from_slice(&self.body);
        bytes
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, ProtocolError> {
        if bytes.len() < HEADER_LEN {
            return Err(ProtocolError::TooShort {
                expected: HEADER_LEN,
                actual: bytes.len(),
            });
        }
        let declared = usize::from(u16::from_le_bytes([bytes[4], bytes[5]]));
        let body = &bytes[HEADER_LEN..];
        if body.len() != declared {
            return Err(ProtocolError::LengthMismatch {
                declared,
                actual: body.len(),
            });
        }
        Ok(Self {
            command_id: u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
            index: u16::from_le_bytes([bytes[6], bytes[7]]),
            body: body.to_vec(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_the_header_then_the_body() {
        let packet = Packet {
            command_id: 0x0080_6005,
            index: 0x0102,
            body: vec![0xaa, 0xbb, 0xcc],
        };
        assert_eq!(
            packet.encode(),
            [
                0x05, 0x60, 0x80, 0x00, // command id
                0x03, 0x00, // body length
                0x02, 0x01, // command index
                0, 0, 0, 0, 0, 0, 0, 0, // unused
                0xaa, 0xbb, 0xcc,
            ]
        );
    }

    #[test]
    fn decodes_what_it_encodes() {
        let packet = Packet {
            command_id: 0x0080_0000,
            index: 65535,
            body: vec![1, 2, 3, 4],
        };
        assert_eq!(Packet::decode(&packet.encode()), Ok(packet));
    }

    #[test]
    fn refuses_a_truncated_header() {
        assert_eq!(
            Packet::decode(&[0; 15]),
            Err(ProtocolError::TooShort {
                expected: 16,
                actual: 15
            })
        );
        assert!(Packet::decode(&[]).is_err());
    }

    #[test]
    fn refuses_a_body_that_does_not_match_the_announced_length() {
        let mut bytes = Packet {
            command_id: 1,
            index: 1,
            body: vec![9, 9],
        }
        .encode();
        bytes.pop();
        assert_eq!(
            Packet::decode(&bytes),
            Err(ProtocolError::LengthMismatch {
                declared: 2,
                actual: 1
            })
        );
        bytes.extend_from_slice(&[9, 9]);
        assert_eq!(
            Packet::decode(&bytes),
            Err(ProtocolError::LengthMismatch {
                declared: 2,
                actual: 3
            })
        );
    }
}
