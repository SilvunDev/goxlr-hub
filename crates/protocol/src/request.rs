use crate::{
    ButtonLights, Channel, EffectKey, Fader, MicParamKey, MicType, OutputSet, ProtocolError,
    RoutingInput, RoutingOutput, Side, Wheel,
};

/// Size of a routing body on a full-size GoXLR.
const ROUTING_LEN: usize = 22;
/// Marks an output as fed by the input of a routing command.
const ROUTED: u8 = 0x20;

const FAMILY_STATUS: u32 = 0x800;
const FAMILY_EFFECTS: u32 = 0x801;
const FAMILY_ROUTING: u32 = 0x804;
const FAMILY_FADER: u32 = 0x805;
const FAMILY_VOLUME: u32 = 0x806;
const FAMILY_ENCODER_VALUE: u32 = 0x80a;
const FAMILY_BUTTON_LIGHTS: u32 = 0x808;
const FAMILY_MUTE: u32 = 0x809;
const FAMILY_MIC_PARAMS: u32 = 0x80b;
const FAMILY_MIC_LEVEL: u32 = 0x80c;
const FAMILY_HARDWARE_INFO: u32 = 0x80f;

const MIC_PARAM_TYPE: u32 = 0;

/// The effect parameter that silences the microphone input.
const EFFECT_MIC_INPUT_MUTE: u32 = 0x0158;

/// A command the app sends to the device.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Request {
    /// Restarts the command counter of the session.
    ResetCommandIndex,
    /// Asks for the buttons held down and the positions of faders and dials.
    GetStatus,
    GetMicLevel,
    GetFirmwareInfo,
    GetSerialInfo,
    /// Puts a channel under a fader.
    SetFader {
        fader: Fader,
        channel: Channel,
    },
    SetVolume {
        channel: Channel,
        volume: u8,
    },
    SetMuted {
        channel: Channel,
        muted: bool,
    },
    /// Puts a dial at a position. The device keeps the dial within the travel
    /// it has in its current mode: the position it reports is what it kept.
    SetEncoderValue {
        wheel: Wheel,
        value: i8,
    },
    /// Sends one side of an input to a set of outputs.
    SetRouting {
        input: RoutingInput,
        side: Side,
        outputs: OutputSet,
    },
    /// Lights every button at once.
    SetButtonLights {
        lights: ButtonLights,
    },
    /// Silences the microphone itself, before any channel or effect.
    SetMicInputMuted {
        muted: bool,
    },
    /// Selects the microphone type and its gain.
    SetMicGain {
        mic_type: MicType,
        gain: u16,
    },
    /// Sets one setting of the microphone processing, as an effect.
    SetEffect {
        key: EffectKey,
        value: i32,
    },
    /// Sets one setting of the gate or of the compressor, as a microphone
    /// parameter.
    SetMicParam {
        key: MicParamKey,
        value: f32,
    },
}

fn id(family: u32, parameter: u8) -> u32 {
    (family << 12) | u32::from(parameter)
}

fn gain_key(mic_type: MicType) -> u32 {
    match mic_type {
        MicType::Dynamic => 1,
        MicType::Condenser => 2,
        MicType::Jack => 3,
    }
}

impl Request {
    pub fn command_id(&self) -> u32 {
        match *self {
            Self::ResetCommandIndex => 0,
            Self::GetStatus => id(FAMILY_STATUS, 0),
            Self::GetMicLevel => id(FAMILY_MIC_LEVEL, 0),
            Self::GetFirmwareInfo => id(FAMILY_HARDWARE_INFO, 0),
            Self::GetSerialInfo => id(FAMILY_HARDWARE_INFO, 1),
            Self::SetFader { fader, .. } => id(FAMILY_FADER, fader.index()),
            Self::SetVolume { channel, .. } => id(FAMILY_VOLUME, channel.index()),
            Self::SetEncoderValue { wheel, .. } => id(FAMILY_ENCODER_VALUE, wheel.index() as u8),
            Self::SetMuted { channel, .. } => id(FAMILY_MUTE, channel.index()),
            Self::SetRouting { input, side, .. } => id(FAMILY_ROUTING, input.id(side)),
            Self::SetButtonLights { .. } => id(FAMILY_BUTTON_LIGHTS, 0),
            Self::SetMicInputMuted { .. } | Self::SetEffect { .. } => id(FAMILY_EFFECTS, 0),
            Self::SetMicGain { .. } | Self::SetMicParam { .. } => id(FAMILY_MIC_PARAMS, 0),
        }
    }

    pub fn body(&self) -> Vec<u8> {
        match *self {
            Self::ResetCommandIndex
            | Self::GetStatus
            | Self::GetMicLevel
            | Self::GetFirmwareInfo
            | Self::GetSerialInfo => Vec::new(),
            Self::SetFader { channel, .. } => vec![channel.index(), 0, 0, 0],
            Self::SetVolume { volume, .. } => vec![volume],
            Self::SetEncoderValue { value, .. } => vec![value.cast_unsigned()],
            Self::SetMuted { muted, .. } => vec![u8::from(muted)],
            Self::SetRouting { side, outputs, .. } => {
                let mut body = vec![0; ROUTING_LEN];
                for output in outputs.iter() {
                    body[output.position(side)] = ROUTED;
                }
                body
            }
            Self::SetButtonLights { lights } => lights.bytes().to_vec(),
            Self::SetMicInputMuted { muted } => {
                let mut body = Vec::with_capacity(8);
                body.extend_from_slice(&EFFECT_MIC_INPUT_MUTE.to_le_bytes());
                body.extend_from_slice(&u32::from(muted).to_le_bytes());
                body
            }
            Self::SetMicGain { mic_type, gain } => {
                let phantom_power = u8::from(mic_type == MicType::Condenser);
                let gain = gain.to_le_bytes();
                let mut body = Vec::with_capacity(16);
                body.extend_from_slice(&MIC_PARAM_TYPE.to_le_bytes());
                body.extend_from_slice(&[phantom_power, 0, 0, 0]);
                body.extend_from_slice(&gain_key(mic_type).to_le_bytes());
                body.extend_from_slice(&[0, 0, gain[0], gain[1]]);
                body
            }
            Self::SetEffect { key, value } => {
                let mut body = Vec::with_capacity(8);
                body.extend_from_slice(&key.id().to_le_bytes());
                body.extend_from_slice(&value.to_le_bytes());
                body
            }
            Self::SetMicParam { key, value } => {
                let mut body = Vec::with_capacity(8);
                body.extend_from_slice(&key.id().to_le_bytes());
                body.extend_from_slice(&value.to_le_bytes());
                body
            }
        }
    }

    /// Reads a command back, the way the device does.
    pub fn decode(command_id: u32, body: &[u8]) -> Result<Self, ProtocolError> {
        let unknown = ProtocolError::UnknownCommand(command_id);
        if command_id == 0 {
            return expect_len(body, 0).map(|()| Self::ResetCommandIndex);
        }
        let family = command_id >> 12;
        let Ok(parameter) = u8::try_from(command_id & 0xfff) else {
            return Err(unknown);
        };

        match (family, parameter) {
            (FAMILY_STATUS, 0) => expect_len(body, 0).map(|()| Self::GetStatus),
            (FAMILY_MIC_LEVEL, 0) => expect_len(body, 0).map(|()| Self::GetMicLevel),
            (FAMILY_HARDWARE_INFO, 0) => expect_len(body, 0).map(|()| Self::GetFirmwareInfo),
            (FAMILY_HARDWARE_INFO, 1) => expect_len(body, 0).map(|()| Self::GetSerialInfo),
            (FAMILY_FADER, _) => {
                let fader = Fader::from_index(parameter).ok_or(unknown)?;
                expect_len(body, 4)?;
                let channel = Channel::from_index(body[0])
                    .ok_or(ProtocolError::InvalidBody("no such channel"))?;
                Ok(Self::SetFader { fader, channel })
            }
            (FAMILY_VOLUME, _) => {
                let channel = Channel::from_index(parameter).ok_or(unknown)?;
                expect_len(body, 1)?;
                Ok(Self::SetVolume {
                    channel,
                    volume: body[0],
                })
            }
            (FAMILY_ENCODER_VALUE, _) => {
                let wheel = Wheel::from_index(parameter).ok_or(unknown)?;
                expect_len(body, 1)?;
                Ok(Self::SetEncoderValue {
                    wheel,
                    value: body[0].cast_signed(),
                })
            }
            (FAMILY_MUTE, _) => {
                let channel = Channel::from_index(parameter).ok_or(unknown)?;
                expect_len(body, 1)?;
                match body[0] {
                    0 => Ok(Self::SetMuted {
                        channel,
                        muted: false,
                    }),
                    1 => Ok(Self::SetMuted {
                        channel,
                        muted: true,
                    }),
                    _ => Err(ProtocolError::InvalidBody("mute state must be 0 or 1")),
                }
            }
            (FAMILY_ROUTING, _) => {
                let (input, side) = RoutingInput::from_id(parameter).ok_or(unknown)?;
                expect_len(body, ROUTING_LEN)?;
                let mut outputs = OutputSet::default();
                for output in RoutingOutput::ALL {
                    if body[output.position(side)] == ROUTED {
                        outputs.insert(output);
                    }
                }
                Ok(Self::SetRouting {
                    input,
                    side,
                    outputs,
                })
            }
            (FAMILY_BUTTON_LIGHTS, 0) => {
                expect_len(body, ButtonLights::LEN)?;
                let lights = ButtonLights::from_bytes(body)
                    .ok_or(ProtocolError::InvalidBody("no such button light"))?;
                Ok(Self::SetButtonLights { lights })
            }
            (FAMILY_EFFECTS, 0) => {
                expect_len(body, 8)?;
                let word = |at: usize| {
                    u32::from_le_bytes([body[at], body[at + 1], body[at + 2], body[at + 3]])
                };
                match (word(0), word(4)) {
                    (EFFECT_MIC_INPUT_MUTE, 0) => Ok(Self::SetMicInputMuted { muted: false }),
                    (EFFECT_MIC_INPUT_MUTE, 1) => Ok(Self::SetMicInputMuted { muted: true }),
                    (EFFECT_MIC_INPUT_MUTE, _) => {
                        Err(ProtocolError::InvalidBody("mute state must be 0 or 1"))
                    }
                    (key, value) => {
                        let key = EffectKey::from_id(key)
                            .ok_or(ProtocolError::InvalidBody("no such effect parameter"))?;
                        Ok(Self::SetEffect {
                            key,
                            value: value.cast_signed(),
                        })
                    }
                }
            }
            (FAMILY_MIC_PARAMS, 0) => {
                let key = |at: usize| {
                    u32::from_le_bytes([body[at], body[at + 1], body[at + 2], body[at + 3]])
                };
                // One parameter alone is a setting of the gate or of the
                // compressor; the gain comes with the microphone type.
                if body.len() == 8 {
                    let value = f32::from_le_bytes([body[4], body[5], body[6], body[7]]);
                    if !value.is_finite() {
                        return Err(ProtocolError::InvalidBody("not a number"));
                    }
                    let key = MicParamKey::from_id(key(0))
                        .ok_or(ProtocolError::InvalidBody("no such microphone parameter"))?;
                    return Ok(Self::SetMicParam { key, value });
                }
                expect_len(body, 16)?;
                if key(0) != MIC_PARAM_TYPE {
                    return Err(ProtocolError::InvalidBody("microphone type expected first"));
                }
                let mic_type = MicType::ALL
                    .into_iter()
                    .find(|mic_type| gain_key(*mic_type) == key(8))
                    .ok_or(ProtocolError::InvalidBody("no such microphone gain"))?;
                Ok(Self::SetMicGain {
                    mic_type,
                    gain: u16::from_le_bytes([body[14], body[15]]),
                })
            }
            _ => Err(unknown),
        }
    }
}

fn expect_len(body: &[u8], expected: usize) -> Result<(), ProtocolError> {
    if body.len() == expected {
        Ok(())
    } else {
        Err(ProtocolError::InvalidBody("unexpected body length"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Button, ButtonLight, EqBand};

    fn bytes(request: Request) -> (u32, Vec<u8>) {
        (request.command_id(), request.body())
    }

    #[test]
    fn queries_have_an_id_and_no_body() {
        assert_eq!(bytes(Request::ResetCommandIndex), (0, vec![]));
        assert_eq!(bytes(Request::GetStatus), (0x0080_0000, vec![]));
        assert_eq!(bytes(Request::GetMicLevel), (0x0080_c000, vec![]));
        assert_eq!(bytes(Request::GetFirmwareInfo), (0x0080_f000, vec![]));
        assert_eq!(bytes(Request::GetSerialInfo), (0x0080_f001, vec![]));
    }

    #[test]
    fn set_fader_names_the_fader_in_the_id_and_the_channel_in_the_body() {
        let request = Request::SetFader {
            fader: Fader::C,
            channel: Channel::Music,
        };
        assert_eq!(bytes(request), (0x0080_5002, vec![7, 0, 0, 0]));
    }

    #[test]
    fn set_volume_names_the_channel_in_the_id() {
        let request = Request::SetVolume {
            channel: Channel::Chat,
            volume: 200,
        };
        assert_eq!(bytes(request), (0x0080_6005, vec![200]));
    }

    #[test]
    fn set_encoder_value_names_the_dial_in_the_id_and_sends_one_signed_byte() {
        let request = Request::SetEncoderValue {
            wheel: Wheel::Gender,
            value: -2,
        };
        assert_eq!(bytes(request), (0x0080_a001, vec![0xfe]));
        let request = Request::SetEncoderValue {
            wheel: Wheel::Echo,
            value: 18,
        };
        assert_eq!(bytes(request), (0x0080_a003, vec![18]));
        for value in [i8::MIN, -1, 0, 1, i8::MAX] {
            let request = Request::SetEncoderValue {
                wheel: Wheel::Pitch,
                value,
            };
            assert_eq!(
                Request::decode(request.command_id(), &request.body()),
                Ok(request)
            );
        }
        assert!(Request::decode(0x0080_a004, &[0]).is_err());
        assert!(Request::decode(0x0080_a000, &[]).is_err());
    }

    #[test]
    fn set_muted_sends_one_for_muted_and_zero_for_open() {
        let muted = Request::SetMuted {
            channel: Channel::LineOut,
            muted: true,
        };
        let open = Request::SetMuted {
            channel: Channel::Mic,
            muted: false,
        };
        assert_eq!(bytes(muted), (0x0080_900a, vec![1]));
        assert_eq!(bytes(open), (0x0080_9000, vec![0]));
    }

    #[test]
    fn set_routing_marks_each_output_at_its_position() {
        let outputs = OutputSet::of(&[RoutingOutput::Headphones, RoutingOutput::LineOut]);

        let left = Request::SetRouting {
            input: RoutingInput::Game,
            side: Side::Left,
            outputs,
        };
        let mut expected = vec![0; 22];
        expected[1] = 0x20;
        expected[17] = 0x20;
        assert_eq!(bytes(left), (0x0080_400a, expected));

        let right = Request::SetRouting {
            input: RoutingInput::Game,
            side: Side::Right,
            outputs,
        };
        let mut expected = vec![0; 22];
        expected[3] = 0x20;
        expected[19] = 0x20;
        assert_eq!(bytes(right), (0x0080_400b, expected));
    }

    #[test]
    fn set_routing_with_no_output_is_all_zeros() {
        let request = Request::SetRouting {
            input: RoutingInput::Mic,
            side: Side::Left,
            outputs: OutputSet::default(),
        };
        assert_eq!(bytes(request), (0x0080_4002, vec![0; 22]));
    }

    #[test]
    fn set_button_lights_sends_one_byte_per_button() {
        let mut lights = ButtonLights::default();
        lights.set(Button::Fader1Mute, ButtonLight::Lit);
        let mut expected = vec![0x02; 24];
        expected[4] = 0x01;
        assert_eq!(
            bytes(Request::SetButtonLights { lights }),
            (0x0080_8000, expected)
        );
    }

    #[test]
    fn set_mic_input_muted_sends_the_parameter_then_its_value() {
        assert_eq!(
            bytes(Request::SetMicInputMuted { muted: true }),
            (0x0080_1000, vec![0x58, 0x01, 0, 0, 1, 0, 0, 0])
        );
        assert_eq!(
            Request::SetMicInputMuted { muted: false }.body(),
            [0x58, 0x01, 0, 0, 0, 0, 0, 0]
        );
    }

    #[test]
    fn set_mic_gain_sends_the_type_then_the_gain_of_that_type() {
        let condenser = Request::SetMicGain {
            mic_type: MicType::Condenser,
            gain: 0x0123,
        };
        assert_eq!(
            bytes(condenser),
            (
                0x0080_b000,
                vec![
                    0, 0, 0, 0, 1, 0, 0, 0, // type: phantom power on
                    2, 0, 0, 0, 0, 0, 0x23, 0x01, // condenser gain
                ]
            )
        );

        let dynamic = Request::SetMicGain {
            mic_type: MicType::Dynamic,
            gain: 40,
        };
        assert_eq!(
            dynamic.body(),
            [0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 40, 0]
        );
        let jack = Request::SetMicGain {
            mic_type: MicType::Jack,
            gain: 0,
        };
        assert_eq!(jack.body()[8], 3);
    }

    #[test]
    fn set_effect_sends_the_key_then_a_signed_value() {
        let threshold = Request::SetEffect {
            key: EffectKey::GateThreshold,
            value: -30,
        };
        assert_eq!(
            bytes(threshold),
            (0x0080_1000, vec![0x11, 0, 0, 0, 0xe2, 0xff, 0xff, 0xff])
        );
        let gain = Request::SetEffect {
            key: EffectKey::EqGain(EqBand::Khz16),
            value: 9,
        };
        assert_eq!(gain.body(), [0x30, 0x01, 0, 0, 9, 0, 0, 0]);
    }

    #[test]
    fn set_mic_param_sends_the_key_then_a_float() {
        let ratio = Request::SetMicParam {
            key: MicParamKey::CompressorRatio,
            value: 2.5,
        };
        assert_eq!(
            bytes(ratio),
            (0x0080_b000, vec![0x00, 0x03, 0x06, 0, 0, 0, 0x20, 0x40])
        );
    }

    #[test]
    fn every_request_decodes_back_to_itself() {
        let mut requests = vec![
            Request::ResetCommandIndex,
            Request::GetStatus,
            Request::GetMicLevel,
            Request::GetFirmwareInfo,
            Request::GetSerialInfo,
        ];
        for fader in Fader::ALL {
            for channel in Channel::ALL {
                requests.push(Request::SetFader { fader, channel });
            }
        }
        for channel in Channel::ALL {
            for volume in [0, 1, 128, 255] {
                requests.push(Request::SetVolume { channel, volume });
            }
            for muted in [false, true] {
                requests.push(Request::SetMuted { channel, muted });
            }
        }
        for input in RoutingInput::ALL {
            for side in Side::BOTH {
                for outputs in [
                    OutputSet::default(),
                    OutputSet::of(&[RoutingOutput::ChatMic]),
                    OutputSet::of(&RoutingOutput::ALL),
                ] {
                    requests.push(Request::SetRouting {
                        input,
                        side,
                        outputs,
                    });
                }
            }
        }
        for muted in [false, true] {
            requests.push(Request::SetMicInputMuted { muted });
        }
        let mut lights = ButtonLights::default();
        requests.push(Request::SetButtonLights { lights });
        lights.set(Button::Bleep, ButtonLight::Lit);
        requests.push(Request::SetButtonLights { lights });
        for mic_type in MicType::ALL {
            for gain in [0, 72, u16::MAX] {
                requests.push(Request::SetMicGain { mic_type, gain });
            }
        }
        for key in EffectKey::all() {
            for value in [i32::MIN, -61, 0, 231] {
                requests.push(Request::SetEffect { key, value });
            }
        }
        for key in MicParamKey::ALL {
            for value in [-61.0, 0.0, 1.1, 64.0] {
                requests.push(Request::SetMicParam { key, value });
            }
        }

        for request in requests {
            assert_eq!(
                Request::decode(request.command_id(), &request.body()),
                Ok(request)
            );
        }
    }

    #[test]
    fn refuses_commands_it_does_not_know() {
        for command_id in [
            0x0080_3000, // lighting, not in this stage
            0x0080_5004, // fader 5
            0x0080_600b, // channel 12
            0x0080_900b,
            0x0080_4001, // no such routing input
            0x0080_f002,
            0x0000_0001,
            u32::MAX,
        ] {
            assert_eq!(
                Request::decode(command_id, &[]),
                Err(ProtocolError::UnknownCommand(command_id)),
                "{command_id:#x}"
            );
        }
    }

    #[test]
    fn refuses_bodies_that_do_not_fit_the_command() {
        let invalid = |command_id: u32, body: &[u8]| {
            assert!(
                matches!(
                    Request::decode(command_id, body),
                    Err(ProtocolError::InvalidBody(_))
                ),
                "{command_id:#x} {body:?}"
            );
        };
        invalid(0, &[1]);
        invalid(0x0080_0000, &[0]);
        invalid(0x0080_5000, &[]);
        invalid(0x0080_5000, &[11, 0, 0, 0]); // channel 12
        invalid(0x0080_6000, &[]);
        invalid(0x0080_6000, &[1, 2]);
        invalid(0x0080_9000, &[2]);
        invalid(0x0080_4002, &[0; 21]);
        invalid(0x0080_4002, &[0; 26]);
        invalid(0x0080_1000, &[0x58, 0x01, 0, 0]);
        invalid(0x0080_1000, &[0x58, 0x01, 0, 0, 2, 0, 0, 0]);
        invalid(0x0080_1000, &[0x59, 0x01, 0, 0, 1, 0, 0, 0]); // another effect
        invalid(0x0080_8000, &[2; 23]);
        invalid(0x0080_8000, &[9; 24]); // no such light
        invalid(0x0080_b000, &[0; 8]); // key 0 alone is no parameter
        invalid(0x0080_b000, &[0x00, 0x02, 0x03, 0, 0, 0, 0xc0, 0x7f]); // not a number
        invalid(0x0080_b000, &[0x00, 0x01, 0x07, 0, 0, 0, 0, 0]); // bleep level: not handled
        invalid(0x0080_b000, &[0; 16]); // gain key 0 is not a gain
        invalid(
            0x0080_b000,
            &[1, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0],
        );
    }
}
