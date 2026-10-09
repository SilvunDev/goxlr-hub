use goxlr_hub_protocol::{
    ButtonLights, Channel, EffectKey, Fader, FirmwareInfo, MicParamKey, MicType, OutputSet, Packet,
    Request, RoutingInput, SerialInfo, Side, Status, Wheel, decode_mic_level,
};

use crate::{Device, DeviceError, DeviceInfo, DeviceKind};

/// Carries one command to the device and brings its answer back.
pub trait Link: Send {
    fn exchange(&mut self, request: &[u8]) -> Result<Vec<u8>, DeviceError>;
}

/// A conversation with a device: numbers the commands, checks that each
/// answer belongs to its command, and recovers once when they drift apart.
pub struct Session<L: Link> {
    link: L,
    /// Number of the last command sent.
    index: u16,
    info: DeviceInfo,
}

impl<L: Link> Session<L> {
    /// Starts the conversation and reads the identity of the device.
    pub fn open(link: L, kind: DeviceKind) -> Result<Self, DeviceError> {
        let mut session = Self {
            link,
            index: 0,
            info: DeviceInfo {
                kind,
                firmware: String::new(),
                serial: String::new(),
                manufactured: String::new(),
            },
        };
        session.attempt(Request::ResetCommandIndex)?;
        let firmware = FirmwareInfo::decode(&session.request(Request::GetFirmwareInfo)?)?;
        let serial = SerialInfo::decode(&session.request(Request::GetSerialInfo)?)?;
        session.info.firmware = firmware.firmware.to_string();
        session.info.serial = serial.serial;
        session.info.manufactured = serial.manufactured;
        Ok(session)
    }

    fn request(&mut self, request: Request) -> Result<Vec<u8>, DeviceError> {
        match self.attempt(request) {
            Err(DeviceError::OutOfSync { .. }) => {
                self.attempt(Request::ResetCommandIndex)?;
                self.attempt(request)
            }
            result => result,
        }
    }

    fn attempt(&mut self, request: Request) -> Result<Vec<u8>, DeviceError> {
        if request == Request::ResetCommandIndex {
            self.index = 0;
        } else {
            if self.index == u16::MAX {
                self.attempt(Request::ResetCommandIndex)?;
            }
            self.index += 1;
        }

        let packet = Packet {
            command_id: request.command_id(),
            index: self.index,
            body: request.body(),
        };
        let answer = Packet::decode(&self.link.exchange(&packet.encode())?)?;
        if answer.index != self.index {
            return Err(DeviceError::OutOfSync {
                expected: self.index,
                received: answer.index,
            });
        }
        Ok(answer.body)
    }

    fn send(&mut self, request: Request) -> Result<(), DeviceError> {
        self.request(request).map(drop)
    }
}

impl<L: Link> Device for Session<L> {
    fn info(&self) -> &DeviceInfo {
        &self.info
    }

    fn status(&mut self) -> Result<Status, DeviceError> {
        Ok(Status::decode(&self.request(Request::GetStatus)?)?)
    }

    fn mic_level(&mut self) -> Result<u16, DeviceError> {
        Ok(decode_mic_level(&self.request(Request::GetMicLevel)?)?)
    }

    fn set_fader(&mut self, fader: Fader, channel: Channel) -> Result<(), DeviceError> {
        self.send(Request::SetFader { fader, channel })
    }

    fn set_volume(&mut self, channel: Channel, volume: u8) -> Result<(), DeviceError> {
        self.send(Request::SetVolume { channel, volume })
    }

    fn set_muted(&mut self, channel: Channel, muted: bool) -> Result<(), DeviceError> {
        self.send(Request::SetMuted { channel, muted })
    }

    fn set_encoder(&mut self, wheel: Wheel, value: i8) -> Result<(), DeviceError> {
        self.send(Request::SetEncoderValue { wheel, value })
    }

    fn set_mic_input_muted(&mut self, muted: bool) -> Result<(), DeviceError> {
        self.send(Request::SetMicInputMuted { muted })
    }

    fn set_button_lights(&mut self, lights: ButtonLights) -> Result<(), DeviceError> {
        self.send(Request::SetButtonLights { lights })
    }

    fn set_routing(&mut self, input: RoutingInput, outputs: OutputSet) -> Result<(), DeviceError> {
        for side in Side::BOTH {
            self.send(Request::SetRouting {
                input,
                side,
                outputs,
            })?;
        }
        Ok(())
    }

    fn set_mic_gain(&mut self, mic_type: MicType, gain: u16) -> Result<(), DeviceError> {
        self.send(Request::SetMicGain { mic_type, gain })
    }

    fn set_effect(&mut self, key: EffectKey, value: i32) -> Result<(), DeviceError> {
        self.send(Request::SetEffect { key, value })
    }

    fn set_mic_param(&mut self, key: MicParamKey, value: f32) -> Result<(), DeviceError> {
        self.send(Request::SetMicParam { key, value })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use goxlr_hub_protocol::ProtocolError;

    use super::*;
    use crate::VirtualGoXlr;

    /// A link driven by a closure, to script what the device answers.
    struct FnLink<F>(F);

    impl<F: FnMut(&[u8]) -> Result<Vec<u8>, DeviceError> + Send> Link for FnLink<F> {
        fn exchange(&mut self, request: &[u8]) -> Result<Vec<u8>, DeviceError> {
            (self.0)(request)
        }
    }

    /// The virtual device, with a log of the command indexes it received and
    /// a way to tamper with its answers.
    fn tampered(
        mut tamper: impl FnMut(usize, Vec<u8>) -> Vec<u8> + Send,
    ) -> (impl Link, Arc<Mutex<Vec<u16>>>) {
        let (mut device, _) = VirtualGoXlr::new();
        let log = Arc::new(Mutex::new(Vec::new()));
        let seen = log.clone();
        let link = FnLink(move |request: &[u8]| {
            let mut seen = seen.lock().unwrap();
            seen.push(Packet::decode(request).unwrap().index);
            let answer = device.exchange(request)?;
            Ok(tamper(seen.len(), answer))
        });
        (link, log)
    }

    #[test]
    fn opening_resets_the_counter_then_numbers_each_command() {
        let (link, log) = tampered(|_, answer| answer);
        let mut session = Session::open(link, DeviceKind::Virtual).unwrap();
        session.status().unwrap();
        session.set_volume(Channel::Mic, 10).unwrap();
        // Reset, firmware, serial, status, volume.
        assert_eq!(*log.lock().unwrap(), [0, 1, 2, 3, 4]);
    }

    #[test]
    fn a_shifted_answer_is_recovered_by_resetting_and_asking_again() {
        // The fourth exchange (the status request) gets an answer numbered
        // for another command.
        let (link, log) = tampered(|exchange, mut answer| {
            if exchange == 4 {
                answer[6] = 0x63;
            }
            answer
        });
        let mut session = Session::open(link, DeviceKind::Virtual).unwrap();
        assert_eq!(session.status(), Ok(Status::default()));
        // Reset, firmware, serial, status (shifted), reset, status again.
        assert_eq!(*log.lock().unwrap(), [0, 1, 2, 3, 0, 1]);
    }

    #[test]
    fn an_answer_that_stays_shifted_is_an_error_not_a_wrong_value() {
        let (link, _) = tampered(|exchange, mut answer| {
            if exchange >= 4 && answer.len() > 16 {
                answer[6] = 0x63;
            }
            answer
        });
        let mut session = Session::open(link, DeviceKind::Virtual).unwrap();
        assert_eq!(
            session.status(),
            Err(DeviceError::OutOfSync {
                expected: 1,
                received: 0x63
            })
        );
    }

    #[test]
    fn a_truncated_answer_is_an_error() {
        // Cut inside the header.
        let (link, _) = tampered(|exchange, mut answer| {
            if exchange == 4 {
                answer.truncate(10);
            }
            answer
        });
        let mut session = Session::open(link, DeviceKind::Virtual).unwrap();
        assert_eq!(
            session.status(),
            Err(DeviceError::Protocol(ProtocolError::TooShort {
                expected: 16,
                actual: 10
            }))
        );

        // Cut inside the body.
        let (link, _) = tampered(|exchange, mut answer| {
            if exchange == 4 {
                answer.pop();
            }
            answer
        });
        let mut session = Session::open(link, DeviceKind::Virtual).unwrap();
        assert!(matches!(
            session.status(),
            Err(DeviceError::Protocol(ProtocolError::LengthMismatch { .. }))
        ));

        // A well-formed packet whose body is too small for a status.
        let (link, _) = tampered(|exchange, mut answer| {
            if exchange == 4 {
                answer.truncate(20);
                answer[4] = 4;
            }
            answer
        });
        let mut session = Session::open(link, DeviceKind::Virtual).unwrap();
        assert_eq!(
            session.status(),
            Err(DeviceError::Protocol(ProtocolError::TooShort {
                expected: 12,
                actual: 4
            }))
        );
    }

    #[test]
    fn an_identity_that_cannot_be_read_fails_the_opening() {
        let link = FnLink(|request: &[u8]| {
            let mut answer = request[..16].to_vec();
            answer[4] = 0;
            Ok(answer)
        });
        assert!(matches!(
            Session::open(link, DeviceKind::Hardware),
            Err(DeviceError::Protocol(ProtocolError::TooShort { .. }))
        ));
    }

    #[test]
    fn a_failing_link_is_reported() {
        let link = FnLink(|_: &[u8]| Err(DeviceError::Link("unplugged".into())));
        assert!(matches!(
            Session::open(link, DeviceKind::Hardware),
            Err(DeviceError::Link(_))
        ));
    }

    #[test]
    fn the_counter_restarts_before_it_overflows() {
        let (link, log) = tampered(|_, answer| answer);
        let mut session = Session::open(link, DeviceKind::Virtual).unwrap();
        session.index = u16::MAX - 1;
        session.status().unwrap();
        session.status().unwrap();
        session.status().unwrap();
        let log = log.lock().unwrap();
        // 65535, then a reset, then counting again from 1.
        assert_eq!(log[3..], [u16::MAX, 0, 1, 2]);
    }

    #[test]
    fn routing_is_sent_for_both_sides() {
        let (link, log) = tampered(|_, answer| answer);
        let mut session = Session::open(link, DeviceKind::Virtual).unwrap();
        session
            .set_routing(RoutingInput::Chat, OutputSet::default())
            .unwrap();
        assert_eq!(log.lock().unwrap().len(), 5);
    }
}
