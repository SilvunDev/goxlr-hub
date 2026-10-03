//! A GoXLR that exists only in memory. It receives the same bytes as the
//! real one and answers the way the firmware does, so the whole app can run,
//! and be tested, without hardware.

use std::sync::{Arc, Mutex, MutexGuard};

use goxlr_hub_protocol::{
    Button, ButtonLights, ButtonSet, Channel, Fader, FirmwareInfo, MicType, OutputSet, Packet,
    Request, RoutingInput, SerialInfo, Side, Status, Version, encode_mic_level,
};

use crate::{DeviceError, DeviceKind, Link, Session};

/// The microphone simulation is paced for this many level readings a second.
const MIC_READS_PER_SECOND: f32 = 20.0;

/// Everything the virtual device remembers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VirtualState {
    /// The channel under each fader.
    pub faders: [Channel; Fader::COUNT],
    pub volumes: [u8; Channel::COUNT],
    pub muted: [bool; Channel::COUNT],
    pub mic_input_muted: bool,
    pub lights: ButtonLights,
    /// Outputs of each input, in `RoutingInput::ALL` order, left then right.
    pub routing: [[OutputSet; 2]; RoutingInput::COUNT],
    pub mic_type: MicType,
    pub mic_gain: u16,
    pub pressed: ButtonSet,
    pub encoders: [i8; 4],
    mic_reads: u64,
}

impl Default for VirtualState {
    /// A device fresh out of the box: nothing set yet.
    fn default() -> Self {
        Self {
            faders: [Channel::Mic, Channel::Chat, Channel::Music, Channel::System],
            volumes: [0; Channel::COUNT],
            muted: [false; Channel::COUNT],
            mic_input_muted: false,
            lights: ButtonLights::default(),
            routing: [[OutputSet::default(); 2]; RoutingInput::COUNT],
            mic_type: MicType::Dynamic,
            mic_gain: 0,
            pressed: ButtonSet::default(),
            encoders: [0; 4],
            mic_reads: 0,
        }
    }
}

impl VirtualState {
    fn answer(&mut self, request: Request) -> Vec<u8> {
        match request {
            Request::ResetCommandIndex => Vec::new(),
            Request::GetStatus => Status {
                pressed: self.pressed,
                encoders: self.encoders,
                // A motorised fader sits at the volume of its channel.
                faders: self
                    .faders
                    .map(|channel| self.volumes[usize::from(channel.index())]),
            }
            .encode(),
            Request::GetMicLevel => {
                self.mic_reads += 1;
                encode_mic_level(simulated_mic_level(self.mic_reads))
            }
            Request::GetFirmwareInfo => {
                let version = Version {
                    major: 1,
                    minor: 4,
                    patch: 3,
                    build: 110,
                };
                FirmwareInfo {
                    firmware: version,
                    fpga_count: 7,
                    dice: version,
                }
                .encode()
            }
            Request::GetSerialInfo => SerialInfo {
                serial: "VIRTUAL".into(),
                manufactured: "2026-10-03".into(),
            }
            .encode(),
            Request::SetFader { fader, channel } => {
                self.faders[usize::from(fader.index())] = channel;
                Vec::new()
            }
            Request::SetVolume { channel, volume } => {
                self.volumes[usize::from(channel.index())] = volume;
                Vec::new()
            }
            Request::SetMuted { channel, muted } => {
                self.muted[usize::from(channel.index())] = muted;
                Vec::new()
            }
            Request::SetMicInputMuted { muted } => {
                self.mic_input_muted = muted;
                Vec::new()
            }
            Request::SetButtonLights { lights } => {
                self.lights = lights;
                Vec::new()
            }
            Request::SetRouting {
                input,
                side,
                outputs,
            } => {
                self.routing[input as usize][side as usize] = outputs;
                Vec::new()
            }
            Request::SetMicGain { mic_type, gain } => {
                self.mic_type = mic_type;
                self.mic_gain = gain;
                Vec::new()
            }
        }
    }

    /// The outputs fed by one side of an input.
    pub fn routed(&self, input: RoutingInput, side: Side) -> OutputSet {
        self.routing[input as usize][side as usize]
    }
}

/// Somebody talking into the microphone: syllables, breaths, and a pause
/// between sentences. Depends only on how many readings were taken, so the
/// same reading always gives the same level.
fn simulated_mic_level(reading: u64) -> u16 {
    use std::f32::consts::TAU;

    // A cheap, repeatable shuffle of the reading number, between 0 and 1.
    let noise = (reading
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1_442_695_040_888_963_407)
        >> 40) as f32
        / (1u64 << 24) as f32;

    let seconds = reading as f32 / MIC_READS_PER_SECOND;
    let speaking = seconds % 6.0 < 4.4;
    let decibels = if speaking {
        let syllable = (seconds * TAU * 1.55).sin().powi(2);
        let breath = 0.6 + 0.4 * (seconds * TAU * 0.35).sin();
        -38.0 + 26.0 * syllable * breath + 3.0 * noise
    } else {
        // Room noise.
        -64.0 + 4.0 * noise
    };
    // The inverse of `goxlr_hub_protocol::mic_level_db`.
    10f32.powf((decibels + 72.2) / 20.0) as u16
}

/// The virtual device as the app reaches it: a link that takes bytes in and
/// gives bytes back.
pub struct VirtualGoXlr {
    state: Arc<Mutex<VirtualState>>,
}

/// The hands on the virtual device: moves its faders, presses its buttons,
/// and looks at what it was told.
#[derive(Clone)]
pub struct VirtualHandle {
    state: Arc<Mutex<VirtualState>>,
}

fn lock(state: &Mutex<VirtualState>) -> MutexGuard<'_, VirtualState> {
    // The state is plain data: it stays usable after a panic elsewhere.
    state
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

impl VirtualGoXlr {
    pub fn new() -> (Self, VirtualHandle) {
        let state = Arc::new(Mutex::new(VirtualState::default()));
        let handle = VirtualHandle {
            state: state.clone(),
        };
        (Self { state }, handle)
    }
}

impl Link for VirtualGoXlr {
    fn exchange(&mut self, request: &[u8]) -> Result<Vec<u8>, DeviceError> {
        let refused = |error| DeviceError::Link(format!("the virtual device refused: {error}"));
        let packet = Packet::decode(request).map_err(refused)?;
        let request = Request::decode(packet.command_id, &packet.body).map_err(refused)?;
        let body = lock(&self.state).answer(request);
        Ok(Packet {
            command_id: packet.command_id,
            index: packet.index,
            body,
        }
        .encode())
    }
}

impl VirtualHandle {
    /// A copy of what the device currently remembers.
    pub fn state(&self) -> VirtualState {
        lock(&self.state).clone()
    }

    /// Pushes a fader by hand: the volume of its channel follows.
    pub fn move_fader(&self, fader: Fader, position: u8) {
        let mut state = lock(&self.state);
        let channel = state.faders[usize::from(fader.index())];
        state.volumes[usize::from(channel.index())] = position;
    }

    pub fn press(&self, button: Button) {
        lock(&self.state).pressed.insert(button);
    }

    pub fn release(&self, button: Button) {
        lock(&self.state).pressed.remove(button);
    }
}

/// Opens a session on a new virtual device.
pub fn open_virtual() -> Result<(Session<VirtualGoXlr>, VirtualHandle), DeviceError> {
    let (device, handle) = VirtualGoXlr::new();
    Ok((Session::open(device, DeviceKind::Virtual)?, handle))
}

#[cfg(test)]
mod tests {
    use goxlr_hub_protocol::{ButtonLight, MIC_LEVEL_FLOOR_DB, RoutingOutput, mic_level_db};

    use super::*;
    use crate::Device;

    #[test]
    fn it_introduces_itself_as_virtual() {
        let (device, _) = open_virtual().unwrap();
        let info = device.info();
        assert_eq!(info.kind, DeviceKind::Virtual);
        assert_eq!(info.firmware, "1.4.3.110");
        assert_eq!(info.serial, "VIRTUAL");
        assert_eq!(info.manufactured, "2026-10-03");
    }

    #[test]
    fn it_remembers_every_setting_it_is_sent() {
        let (mut device, hands) = open_virtual().unwrap();
        device.set_fader(Fader::B, Channel::Game).unwrap();
        device.set_volume(Channel::Game, 99).unwrap();
        device.set_muted(Channel::Music, true).unwrap();
        device
            .set_routing(
                RoutingInput::Game,
                OutputSet::of(&[RoutingOutput::Headphones, RoutingOutput::BroadcastMix]),
            )
            .unwrap();
        device.set_mic_gain(MicType::Condenser, 30).unwrap();
        let mut lights = ButtonLights::default();
        lights.set(Button::Fader3Mute, ButtonLight::Lit);
        device.set_button_lights(lights).unwrap();
        device.set_mic_input_muted(true).unwrap();

        let state = hands.state();
        assert_eq!(state.lights, lights);
        assert!(state.mic_input_muted);
        assert_eq!(state.faders[1], Channel::Game);
        assert_eq!(state.volumes[usize::from(Channel::Game.index())], 99);
        assert!(state.muted[usize::from(Channel::Music.index())]);
        assert!(!state.muted[usize::from(Channel::Game.index())]);
        for side in Side::BOTH {
            let outputs = state.routed(RoutingInput::Game, side);
            assert_eq!(
                outputs.iter().collect::<Vec<_>>(),
                [RoutingOutput::Headphones, RoutingOutput::BroadcastMix]
            );
            assert_eq!(state.routed(RoutingInput::Chat, side), OutputSet::default());
        }
        assert_eq!((state.mic_type, state.mic_gain), (MicType::Condenser, 30));
    }

    #[test]
    fn its_faders_sit_at_the_volume_of_their_channel() {
        let (mut device, _) = open_virtual().unwrap();
        device.set_volume(Channel::Music, 77).unwrap();
        device.set_volume(Channel::LineIn, 200).unwrap();
        assert_eq!(device.status().unwrap().faders, [0, 0, 77, 0]);

        // Putting another channel under a fader moves the fader.
        device.set_fader(Fader::A, Channel::LineIn).unwrap();
        assert_eq!(device.status().unwrap().faders, [200, 0, 77, 0]);
    }

    #[test]
    fn a_fader_pushed_by_hand_changes_the_volume_of_its_channel() {
        let (mut device, hands) = open_virtual().unwrap();
        hands.move_fader(Fader::D, 180);
        assert_eq!(device.status().unwrap().faders, [0, 0, 0, 180]);
        assert_eq!(
            hands.state().volumes[usize::from(Channel::System.index())],
            180
        );
    }

    #[test]
    fn buttons_show_while_they_are_held() {
        let (mut device, hands) = open_virtual().unwrap();
        hands.press(Button::Fader2Mute);
        hands.press(Button::Bleep);
        assert_eq!(
            device.status().unwrap().pressed.iter().collect::<Vec<_>>(),
            [Button::Fader2Mute, Button::Bleep]
        );
        hands.release(Button::Fader2Mute);
        hands.release(Button::Bleep);
        assert_eq!(device.status().unwrap().pressed, ButtonSet::default());
    }

    #[test]
    fn its_microphone_is_alive_and_stays_in_range() {
        let (mut device, _) = open_virtual().unwrap();
        // Thirty seconds of readings.
        let levels: Vec<f32> = (0..600)
            .map(|_| mic_level_db(device.mic_level().unwrap()))
            .collect();

        let loudest = levels.iter().copied().fold(f32::MIN, f32::max);
        let quietest = levels.iter().copied().fold(f32::MAX, f32::min);
        assert!(loudest < -3.0, "never clips: {loudest}");
        assert!(loudest > -20.0, "speech is clearly visible: {loudest}");
        assert!(quietest > MIC_LEVEL_FLOOR_DB, "a room is never silent");
        assert!(quietest < -55.0, "there are pauses: {quietest}");

        let moves = levels.windows(2).filter(|pair| pair[0] != pair[1]).count();
        assert!(moves > 500, "the meter keeps moving: {moves}");
    }

    #[test]
    fn its_microphone_is_repeatable() {
        let (mut first, _) = open_virtual().unwrap();
        let (mut second, _) = open_virtual().unwrap();
        for _ in 0..200 {
            assert_eq!(first.mic_level(), second.mic_level());
        }
    }

    #[test]
    fn it_refuses_what_it_does_not_understand_and_keeps_its_state() {
        let (mut device, hands) = VirtualGoXlr::new();
        let before = hands.state();
        let packet = |command_id: u32, body: &[u8]| {
            Packet {
                command_id,
                index: 1,
                body: body.to_vec(),
            }
            .encode()
        };

        for request in [
            vec![],
            vec![0; 15],
            packet(0x0080_3000, &[]),   // lighting: not a command it knows
            packet(0x0080_600b, &[10]), // volume of a channel that does not exist
            packet(0x0080_5000, &[11, 0, 0, 0]), // fader given a channel that does not exist
            packet(0x0080_9000, &[7]),  // neither muted nor open
            packet(0x0080_4002, &[0; 21]), // routing row of the wrong size
        ] {
            assert!(
                matches!(device.exchange(&request), Err(DeviceError::Link(_))),
                "{request:?}"
            );
        }
        assert_eq!(hands.state(), before);
    }
}
