//! What the app knows about the device, and the picture of it the interface
//! draws. The interface never reaches the device: it sends an [`Intent`] and
//! receives a [`Snapshot`].

mod station;

use goxlr_hub_device::{Device, DeviceError, DeviceKind};
use goxlr_hub_protocol::{
    Button, ButtonLight, ButtonLights, ButtonSet, Channel, Fader, OutputSet, RoutingInput,
    RoutingOutput, mic_level_db,
};
use serde::{Deserialize, Serialize};

pub use station::{Connection, ConnectionView, Port, Station};

/// The mute button under each fader, left to right.
const MUTE_BUTTONS: [Button; Fader::COUNT] = [
    Button::Fader1Mute,
    Button::Fader2Mute,
    Button::Fader3Mute,
    Button::Fader4Mute,
];

/// A motorised fader is considered arrived this close to where it was sent.
const FADER_TOLERANCE: u8 = 5;

/// Readings a travelling fader is given before it is believed again: about
/// a second.
const FADER_TRAVEL_READINGS: u8 = 20;

/// The routing of a device the app meets: everything is heard in the
/// headphones, on the stream and on the line output, the microphone goes to
/// the stream and to the voice chat, and the samples are played to the voice
/// chat too.
pub fn default_routing() -> [OutputSet; RoutingInput::COUNT] {
    use RoutingOutput::{BroadcastMix, ChatMic, Headphones, LineOut};
    RoutingInput::ALL.map(|input| match input {
        RoutingInput::Mic => OutputSet::of(&[BroadcastMix, ChatMic]),
        RoutingInput::Samples => OutputSet::of(&[Headphones, BroadcastMix, ChatMic, LineOut]),
        _ => OutputSet::of(&[Headphones, BroadcastMix, LineOut]),
    })
}

/// Two routes only feed a sound back to where it comes from: the voice chat
/// into the voice chat, and the samples into the sampler.
pub fn can_route(input: RoutingInput, output: RoutingOutput) -> bool {
    !matches!(
        (input, output),
        (RoutingInput::Chat, RoutingOutput::ChatMic)
            | (RoutingInput::Samples, RoutingOutput::Sampler)
    )
}

/// Faders, volumes, mutes and routing. The device cannot be asked for them,
/// so the app keeps them and sends them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MixerState {
    /// The channel under each fader.
    pub faders: [Channel; Fader::COUNT],
    /// `None` when the app never set the volume and no fader shows it.
    pub volumes: [Option<u8>; Channel::COUNT],
    pub muted: [bool; Channel::COUNT],
    /// The microphone button: silences the microphone itself, whatever the
    /// mute of its channel says.
    pub mic_off: bool,
    /// The outputs each input is sent to, in `RoutingInput::ALL` order.
    pub routing: [OutputSet; RoutingInput::COUNT],
}

impl MixerState {
    /// Nothing of the microphone is heard: its channel is muted, or the
    /// microphone itself is off.
    pub fn mic_silenced(&self) -> bool {
        self.mic_off || self.muted[usize::from(Channel::Mic.index())]
    }
}

impl Default for MixerState {
    fn default() -> Self {
        let mut volumes = [Some(255); Channel::COUNT];
        volumes[usize::from(Channel::Mic.index())] = Some(214);
        volumes[usize::from(Channel::Chat.index())] = Some(178);
        volumes[usize::from(Channel::Music.index())] = Some(120);
        volumes[usize::from(Channel::System.index())] = Some(196);
        Self {
            volumes,
            ..Self::unknown()
        }
    }
}

impl MixerState {
    /// The mixer of a real device the app meets: no volume is known yet.
    pub fn unknown() -> Self {
        Self {
            faders: [Channel::Mic, Channel::Chat, Channel::Music, Channel::System],
            volumes: [None; Channel::COUNT],
            muted: [false; Channel::COUNT],
            mic_off: false,
            routing: default_routing(),
        }
    }

    /// For when another program had the device and set volumes as it pleased.
    pub fn forget_volumes(&mut self) {
        self.volumes = [None; Channel::COUNT];
    }

    fn fader_of(&self, channel: Channel) -> Option<usize> {
        self.faders.iter().position(|carried| *carried == channel)
    }
}

/// What the interface asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Intent {
    SetVolume {
        channel: Channel,
        volume: u8,
    },
    SetMuted {
        channel: Channel,
        muted: bool,
    },
    /// Turns the microphone itself off or on, like its button does.
    SetMicOff {
        off: bool,
    },
    /// Puts a channel under a fader. A channel that is already under
    /// another fader swaps places with the one it replaces.
    AssignFader {
        fader: Fader,
        channel: Channel,
    },
    /// Sends an input to an output, or stops sending it there.
    SetRoute {
        input: RoutingInput,
        output: RoutingOutput,
        on: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    /// Which device is shown, and why.
    pub connection: ConnectionView,
    pub device: DeviceView,
    pub faders: [FaderView; Fader::COUNT],
    pub channels: Vec<ChannelView>,
    /// The microphone itself is off, whatever its channel says.
    pub mic_off: bool,
    /// One row per input, in the order of the device.
    pub routing: Vec<RouteView>,
    /// Buttons held down right now.
    pub pressed: Vec<Button>,
    /// Between -72.2 (silence) and 0 (full scale).
    pub mic_level_db: f32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DeviceView {
    /// `virtual` or `hardware`.
    pub kind: &'static str,
    pub firmware: String,
    pub serial: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct FaderView {
    pub fader: Fader,
    pub channel: Channel,
    /// 0 to 255.
    pub volume: u8,
    pub muted: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ChannelView {
    pub channel: Channel,
    /// 0 to 255, or nothing when the app does not know it.
    pub volume: Option<u8>,
    pub muted: bool,
    /// The fader that carries the channel, if any.
    pub fader: Option<Fader>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RouteView {
    pub input: RoutingInput,
    /// The outputs the input is sent to.
    pub outputs: Vec<RoutingOutput>,
}

/// A fader sent somewhere by the app: its readings are not believed while
/// its motor travels.
#[derive(Debug, Clone, Copy)]
struct Travel {
    target: u8,
    readings_left: u8,
}

/// The app's side of one connected device.
pub struct Hub {
    device: Box<dyn Device>,
    mixer: MixerState,
    travels: [Option<Travel>; Fader::COUNT],
    /// The buttons held at the last reading, to tell a press from a hold.
    held: Option<ButtonSet>,
}

impl Hub {
    /// Takes a device over and brings it to the app's state.
    pub fn connect(device: Box<dyn Device>) -> Result<Self, DeviceError> {
        Self::take(device, MixerState::default())
    }

    /// Takes a real device over, changing how it sounds as little as the
    /// truth of the screen allows.
    ///
    /// The fader assignment, the mutes, the routing and the mute lights are
    /// sent, because the device cannot tell them. Volumes are only sent when the app knows
    /// them: those of a device that dropped and came back. The others are
    /// read from the faders, or stay unknown.
    pub fn adopt(
        device: Box<dyn Device>,
        remembered: Option<&MixerState>,
    ) -> Result<Self, DeviceError> {
        Self::take(
            device,
            remembered.cloned().unwrap_or_else(MixerState::unknown),
        )
    }

    fn take(device: Box<dyn Device>, mixer: MixerState) -> Result<Self, DeviceError> {
        let mut hub = Self {
            device,
            mixer,
            travels: [None; Fader::COUNT],
            held: None,
        };
        // A device that was just plugged in plays with its own settings:
        // what silences is sent first.
        for (input, outputs) in RoutingInput::ALL.into_iter().zip(hub.mixer.routing) {
            hub.device.set_routing(input, outputs)?;
        }
        for channel in Channel::ALL {
            if channel != Channel::Mic {
                let muted = hub.mixer.muted[usize::from(channel.index())];
                hub.device.set_muted(channel, muted)?;
            }
        }
        hub.send_mic(hub.mixer.mic_silenced())?;
        for (fader, channel) in Fader::ALL.into_iter().zip(hub.mixer.faders) {
            hub.device.set_fader(fader, channel)?;
        }
        for channel in Channel::ALL {
            if let Some(volume) = hub.mixer.volumes[usize::from(channel.index())] {
                hub.send_volume(channel, volume)?;
            }
        }
        hub.send_lights()?;
        Ok(hub)
    }

    pub fn mixer(&self) -> &MixerState {
        &self.mixer
    }

    /// Does what the interface asked, on the device first.
    pub fn apply(&mut self, intent: Intent) -> Result<(), DeviceError> {
        match intent {
            Intent::SetVolume { channel, volume } => {
                self.send_volume(channel, volume)?;
                self.mixer.volumes[usize::from(channel.index())] = Some(volume);
                Ok(())
            }
            Intent::SetMuted { channel, muted } => self.set_muted(channel, muted),
            Intent::SetMicOff { off } => self.set_mic_off(off),
            Intent::AssignFader { fader, channel } => self.assign(fader, channel),
            Intent::SetRoute { input, output, on } => self.set_route(input, output, on),
        }
    }

    /// The device takes the outputs of an input as a whole.
    fn set_route(
        &mut self,
        input: RoutingInput,
        output: RoutingOutput,
        on: bool,
    ) -> Result<(), DeviceError> {
        if !can_route(input, output) {
            return Ok(());
        }
        let outputs = self.mixer.routing[input as usize].with(output, on);
        self.device.set_routing(input, outputs)?;
        self.mixer.routing[input as usize] = outputs;
        Ok(())
    }

    /// Sends a volume; the fader that carries the channel will travel there.
    fn send_volume(&mut self, channel: Channel, volume: u8) -> Result<(), DeviceError> {
        self.device.set_volume(channel, volume)?;
        if let Some(fader) = self.mixer.fader_of(channel) {
            self.travels[fader] = Some(Travel {
                target: volume,
                readings_left: FADER_TRAVEL_READINGS,
            });
        }
        Ok(())
    }

    fn set_muted(&mut self, channel: Channel, muted: bool) -> Result<(), DeviceError> {
        if channel == Channel::Mic {
            self.send_mic(muted || self.mixer.mic_off)?;
        } else {
            self.device.set_muted(channel, muted)?;
        }
        self.mixer.muted[usize::from(channel.index())] = muted;
        self.send_lights()
    }

    fn set_mic_off(&mut self, off: bool) -> Result<(), DeviceError> {
        self.send_mic(off || self.mixer.muted[usize::from(Channel::Mic.index())])?;
        self.mixer.mic_off = off;
        self.send_lights()
    }

    /// Muting the channel of the microphone is not enough to silence it
    /// everywhere: its input is muted too.
    fn send_mic(&mut self, silenced: bool) -> Result<(), DeviceError> {
        self.device.set_muted(Channel::Mic, silenced)?;
        self.device.set_mic_input_muted(silenced)
    }

    fn assign(&mut self, fader: Fader, channel: Channel) -> Result<(), DeviceError> {
        let at = usize::from(fader.index());
        let replaced = self.mixer.faders[at];
        if replaced == channel {
            return Ok(());
        }

        let mut moved = vec![at];
        self.device.set_fader(fader, channel)?;
        if let Some(other) = self.mixer.fader_of(channel) {
            self.device.set_fader(Fader::ALL[other], replaced)?;
            self.mixer.faders.swap(at, other);
            moved.push(other);
        } else {
            self.mixer.faders[at] = channel;
            // The motor may have dragged the channel that left along.
            if let Some(volume) = self.mixer.volumes[usize::from(replaced.index())] {
                self.device.set_volume(replaced, volume)?;
            }
        }

        for at in moved {
            let carried = self.mixer.faders[at];
            self.travels[at] = None;
            // A volume the app does not know is read from the fader instead.
            if let Some(volume) = self.mixer.volumes[usize::from(carried.index())] {
                self.send_volume(carried, volume)?;
            }
        }
        self.send_lights()
    }

    /// A mute button is lit when the channel of its fader is muted.
    fn lights(&self) -> ButtonLights {
        let muted = |channel: Channel| self.mixer.muted[usize::from(channel.index())];
        let mut lights = ButtonLights::default();
        for (button, channel) in MUTE_BUTTONS.into_iter().zip(self.mixer.faders) {
            if muted(channel) {
                lights.set(button, ButtonLight::Lit);
            }
        }
        if self.mixer.mic_off {
            lights.set(Button::MicMute, ButtonLight::Lit);
        }
        lights
    }

    fn send_lights(&mut self) -> Result<(), DeviceError> {
        self.device.set_button_lights(self.lights())
    }

    /// Does what the mute buttons that went down since the last reading
    /// are for.
    fn follow_buttons(&mut self, pressed: ButtonSet) -> Result<(), DeviceError> {
        // Buttons already down when the device is taken over are no press.
        let held = self.held.replace(pressed).unwrap_or(pressed);
        let went_down = |button: Button| pressed.contains(button) && !held.contains(button);

        for (button, channel) in MUTE_BUTTONS.into_iter().zip(self.mixer.faders) {
            if went_down(button) {
                let muted = self.mixer.muted[usize::from(channel.index())];
                self.set_muted(channel, !muted)?;
            }
        }
        if went_down(Button::MicMute) {
            self.set_mic_off(!self.mixer.mic_off)?;
        }
        Ok(())
    }

    /// Reads what changed on the device and returns the picture to draw.
    pub fn poll(&mut self) -> Result<Snapshot, DeviceError> {
        let status = self.device.status()?;
        let mic_level = self.device.mic_level()?;

        // A fader moved by hand sets the volume of the channel it carries.
        for (at, position) in status.faders.into_iter().enumerate() {
            if let Some(travel) = &mut self.travels[at] {
                let arrived = position.abs_diff(travel.target) <= FADER_TOLERANCE;
                let given_up = travel.readings_left == 0;
                travel.readings_left = travel.readings_left.saturating_sub(1);
                if arrived || given_up {
                    self.travels[at] = None;
                }
                if !given_up {
                    continue;
                }
            }
            let channel = self.mixer.faders[at];
            self.mixer.volumes[usize::from(channel.index())] = Some(position);
        }

        self.follow_buttons(status.pressed)?;

        let volume = |channel: Channel| self.mixer.volumes[usize::from(channel.index())];
        let muted = |channel: Channel| self.mixer.muted[usize::from(channel.index())];
        let info = self.device.info();

        let mut faders = Fader::ALL.map(|fader| FaderView {
            fader,
            channel: Channel::Mic,
            volume: 0,
            muted: false,
        });
        for (at, view) in faders.iter_mut().enumerate() {
            let channel = self.mixer.faders[at];
            view.channel = channel;
            view.volume = volume(channel).unwrap_or(status.faders[at]);
            view.muted = muted(channel);
        }

        let connection = match info.kind {
            DeviceKind::Virtual => Connection::Demo,
            DeviceKind::Hardware => Connection::Hardware,
        };

        Ok(Snapshot {
            connection: connection.view(),
            device: DeviceView {
                kind: match info.kind {
                    DeviceKind::Virtual => "virtual",
                    DeviceKind::Hardware => "hardware",
                },
                firmware: info.firmware.clone(),
                serial: info.serial.clone(),
            },
            faders,
            channels: Channel::ALL
                .into_iter()
                .map(|channel| ChannelView {
                    channel,
                    volume: volume(channel),
                    muted: muted(channel),
                    fader: self.mixer.fader_of(channel).map(|at| Fader::ALL[at]),
                })
                .collect(),
            mic_off: self.mixer.mic_off,
            routing: RoutingInput::ALL
                .into_iter()
                .zip(self.mixer.routing)
                .map(|(input, outputs)| RouteView {
                    input,
                    outputs: outputs.iter().collect(),
                })
                .collect(),
            pressed: status.pressed.iter().collect(),
            mic_level_db: mic_level_db(mic_level),
        })
    }
}

#[cfg(test)]
mod tests {
    use goxlr_hub_device::open_virtual;
    use goxlr_hub_protocol::Side;
    use serde_json::json;

    use super::*;

    fn hub() -> (Hub, goxlr_hub_device::VirtualHandle) {
        let (device, hands) = open_virtual().unwrap();
        (Hub::connect(Box::new(device)).unwrap(), hands)
    }

    #[test]
    fn connecting_brings_the_device_to_the_app_state() {
        let (hub, hands) = hub();
        let device = hands.state();
        assert_eq!(device.faders, hub.mixer().faders);
        assert_eq!(device.volumes.map(Some), hub.mixer().volumes);
        assert_eq!(device.muted, hub.mixer().muted);
        assert_eq!(device.volumes[usize::from(Channel::Music.index())], 120);
        for input in RoutingInput::ALL {
            assert_eq!(routed(&hands, input), hub.mixer().routing[input as usize]);
        }
        assert_eq!(
            routed(&hands, RoutingInput::Mic),
            OutputSet::of(&[RoutingOutput::BroadcastMix, RoutingOutput::ChatMic])
        );
        assert_eq!(
            routed(&hands, RoutingInput::Music),
            OutputSet::of(&[
                RoutingOutput::Headphones,
                RoutingOutput::BroadcastMix,
                RoutingOutput::LineOut
            ])
        );
    }

    /// The outputs an input feeds on the device, the same on both sides.
    fn routed(hands: &goxlr_hub_device::VirtualHandle, input: RoutingInput) -> OutputSet {
        let device = hands.state();
        let left = device.routed(input, Side::Left);
        assert_eq!(left, device.routed(input, Side::Right), "{input:?}");
        left
    }

    fn route(input: RoutingInput, output: RoutingOutput, on: bool) -> Intent {
        Intent::SetRoute { input, output, on }
    }

    #[test]
    fn a_route_set_from_the_screen_reaches_the_device_and_leaves_the_others() {
        let (mut hub, hands) = hub();
        hub.apply(route(RoutingInput::Mic, RoutingOutput::Headphones, true))
            .unwrap();
        hub.apply(route(RoutingInput::Music, RoutingOutput::Headphones, false))
            .unwrap();

        assert_eq!(
            routed(&hands, RoutingInput::Mic),
            OutputSet::of(&[
                RoutingOutput::Headphones,
                RoutingOutput::BroadcastMix,
                RoutingOutput::ChatMic
            ])
        );
        assert_eq!(
            routed(&hands, RoutingInput::Music),
            OutputSet::of(&[RoutingOutput::BroadcastMix, RoutingOutput::LineOut])
        );
        assert_eq!(
            routed(&hands, RoutingInput::Game),
            default_routing()[RoutingInput::Game as usize]
        );

        let snapshot = hub.poll().unwrap();
        let music = &snapshot.routing[RoutingInput::Music as usize];
        assert_eq!(music.input, RoutingInput::Music);
        assert_eq!(
            music.outputs,
            [RoutingOutput::BroadcastMix, RoutingOutput::LineOut]
        );
    }

    #[test]
    fn an_input_can_be_sent_nowhere() {
        let (mut hub, hands) = hub();
        for output in RoutingOutput::ALL {
            hub.apply(route(RoutingInput::Mic, output, false)).unwrap();
        }
        assert_eq!(routed(&hands, RoutingInput::Mic), OutputSet::default());
        let snapshot = hub.poll().unwrap();
        assert_eq!(snapshot.routing[RoutingInput::Mic as usize].outputs, []);
        assert_eq!(snapshot.routing.len(), 8);
    }

    #[test]
    fn a_route_that_makes_no_sense_is_not_sent() {
        assert!(!can_route(RoutingInput::Chat, RoutingOutput::ChatMic));
        assert!(!can_route(RoutingInput::Samples, RoutingOutput::Sampler));
        assert!(can_route(RoutingInput::Samples, RoutingOutput::ChatMic));
        assert!(can_route(RoutingInput::Mic, RoutingOutput::Sampler));
        for (input, outputs) in RoutingInput::ALL.into_iter().zip(default_routing()) {
            for output in outputs.iter() {
                assert!(can_route(input, output), "{input:?} to {output:?}");
            }
        }

        let (mut hub, hands) = hub();
        let (device, mixer) = (hands.state(), hub.mixer().clone());
        hub.apply(route(RoutingInput::Chat, RoutingOutput::ChatMic, true))
            .unwrap();
        hub.apply(route(RoutingInput::Samples, RoutingOutput::Sampler, true))
            .unwrap();
        assert_eq!(hands.state(), device);
        assert_eq!(hub.mixer(), &mixer);
    }

    #[test]
    fn the_snapshot_shows_each_fader_with_its_channel() {
        let (mut hub, _) = hub();
        let snapshot = hub.poll().unwrap();
        assert_eq!(snapshot.device.kind, "virtual");
        assert_eq!(
            snapshot.faders.map(|view| (view.channel, view.volume)),
            [
                (Channel::Mic, 214),
                (Channel::Chat, 178),
                (Channel::Music, 120),
                (Channel::System, 196),
            ]
        );
        assert_eq!(snapshot.channels.len(), 11);
        assert_eq!(snapshot.pressed, []);
    }

    #[test]
    fn a_fader_moved_by_hand_shows_up_on_its_channel() {
        let (mut hub, hands) = hub();
        // The faders have reached the volumes sent on connection.
        hub.poll().unwrap();
        hands.move_fader(Fader::C, 31);
        hands.press(Button::MicMute);

        let snapshot = hub.poll().unwrap();
        assert_eq!(snapshot.faders[2].volume, 31);
        assert_eq!(snapshot.faders[0].volume, 214);
        let music = snapshot.channels[usize::from(Channel::Music.index())];
        assert_eq!((music.channel, music.volume), (Channel::Music, Some(31)));
        assert_eq!(snapshot.pressed, [Button::MicMute]);
        assert_eq!(
            hub.mixer().volumes[usize::from(Channel::Music.index())],
            Some(31)
        );
    }

    fn at(channel: Channel) -> usize {
        usize::from(channel.index())
    }

    #[test]
    fn a_volume_set_from_the_screen_reaches_the_device() {
        let (mut hub, hands) = hub();
        let set = |channel, volume| Intent::SetVolume { channel, volume };
        hub.apply(set(Channel::Music, 200)).unwrap();
        hub.apply(set(Channel::Headphones, 90)).unwrap();

        assert_eq!(hands.state().volumes[at(Channel::Music)], 200);
        assert_eq!(hands.state().volumes[at(Channel::Headphones)], 90);
        let snapshot = hub.poll().unwrap();
        assert_eq!(snapshot.faders[2].volume, 200);
        assert_eq!(snapshot.channels[at(Channel::Headphones)].volume, Some(90));
    }

    #[test]
    fn a_mute_set_from_the_screen_reaches_the_device_and_lights_its_button() {
        let (mut hub, hands) = hub();
        let mute = |channel, muted| Intent::SetMuted { channel, muted };
        hub.apply(mute(Channel::Chat, true)).unwrap();
        hub.apply(mute(Channel::Game, true)).unwrap();

        let device = hands.state();
        assert!(device.muted[at(Channel::Chat)] && device.muted[at(Channel::Game)]);
        // Chat is under fader B; Game is under no fader, so has no light.
        let mut lights = ButtonLights::default();
        lights.set(Button::Fader2Mute, ButtonLight::Lit);
        assert_eq!(device.lights, lights);
        assert!(hub.poll().unwrap().faders[1].muted);

        hub.apply(mute(Channel::Chat, false)).unwrap();
        assert!(!hands.state().muted[at(Channel::Chat)]);
        assert_eq!(hands.state().lights, ButtonLights::default());
    }

    #[test]
    fn a_mute_button_toggles_its_channel_once_however_long_it_is_held() {
        let (mut hub, hands) = hub();
        hub.poll().unwrap();

        hands.press(Button::Fader3Mute);
        for _ in 0..5 {
            assert!(hub.poll().unwrap().faders[2].muted);
        }
        assert!(hands.state().muted[at(Channel::Music)]);
        assert_eq!(
            hands.state().lights.get(Button::Fader3Mute),
            ButtonLight::Lit
        );

        hands.release(Button::Fader3Mute);
        assert!(hub.poll().unwrap().faders[2].muted);
        hands.press(Button::Fader3Mute);
        assert!(!hub.poll().unwrap().faders[2].muted);
        assert!(!hands.state().muted[at(Channel::Music)]);
    }

    #[test]
    fn a_button_already_held_when_the_device_is_taken_is_not_a_press() {
        let (mut hub, hands) = hub();
        hands.press(Button::Fader1Mute);
        assert!(!hub.poll().unwrap().faders[0].muted);
        assert!(!hub.poll().unwrap().faders[0].muted);
    }

    #[test]
    fn the_microphone_button_turns_the_microphone_off_not_its_channel() {
        let (mut hub, hands) = hub();
        hub.poll().unwrap();

        hands.press(Button::MicMute);
        let snapshot = hub.poll().unwrap();
        assert!(snapshot.mic_off);
        assert!(
            !snapshot.faders[0].muted,
            "the channel mute is another thing"
        );
        let device = hands.state();
        assert!(device.mic_input_muted && device.muted[at(Channel::Mic)]);
        assert_eq!(device.lights.get(Button::MicMute), ButtonLight::Lit);
        assert_eq!(device.lights.get(Button::Fader1Mute), ButtonLight::Dimmed);

        hands.release(Button::MicMute);
        hub.poll().unwrap();
        hands.press(Button::MicMute);
        assert!(!hub.poll().unwrap().mic_off);
        let device = hands.state();
        assert!(!device.mic_input_muted && !device.muted[at(Channel::Mic)]);
        assert_eq!(device.lights, ButtonLights::default());
    }

    #[test]
    fn the_microphone_is_heard_only_when_neither_mute_holds_it() {
        let (mut hub, hands) = hub();
        let silenced = || {
            let device = hands.state();
            assert_eq!(device.mic_input_muted, device.muted[at(Channel::Mic)]);
            device.mic_input_muted
        };
        let channel = |muted| Intent::SetMuted {
            channel: Channel::Mic,
            muted,
        };

        hub.apply(channel(true)).unwrap();
        assert!(silenced());
        hub.apply(Intent::SetMicOff { off: true }).unwrap();
        hub.apply(channel(false)).unwrap();
        assert!(silenced(), "the microphone is still off");
        hub.apply(Intent::SetMicOff { off: false }).unwrap();
        assert!(!silenced());

        hub.apply(channel(true)).unwrap();
        hub.apply(Intent::SetMicOff { off: true }).unwrap();
        hub.apply(Intent::SetMicOff { off: false }).unwrap();
        assert!(silenced(), "the channel is still muted");
    }

    #[test]
    fn a_fader_can_be_given_a_channel_that_is_under_no_fader() {
        let (mut hub, hands) = hub();
        hub.apply(Intent::SetMuted {
            channel: Channel::Game,
            muted: true,
        })
        .unwrap();
        hub.apply(Intent::AssignFader {
            fader: Fader::B,
            channel: Channel::Game,
        })
        .unwrap();

        let device = hands.state();
        assert_eq!(
            device.faders,
            [Channel::Mic, Channel::Game, Channel::Music, Channel::System]
        );
        // The channel that left keeps its volume; the mute light follows.
        assert_eq!(device.volumes[at(Channel::Chat)], 178);
        assert_eq!(device.lights.get(Button::Fader2Mute), ButtonLight::Lit);

        let snapshot = hub.poll().unwrap();
        assert_eq!(snapshot.faders[1].channel, Channel::Game);
        assert_eq!(snapshot.faders[1].volume, 255);
        assert_eq!(snapshot.channels[at(Channel::Game)].fader, Some(Fader::B));
        assert_eq!(snapshot.channels[at(Channel::Chat)].fader, None);
    }

    #[test]
    fn a_channel_already_under_a_fader_swaps_places() {
        let (mut hub, hands) = hub();
        hub.apply(Intent::AssignFader {
            fader: Fader::A,
            channel: Channel::System,
        })
        .unwrap();

        assert_eq!(
            hands.state().faders,
            [Channel::System, Channel::Chat, Channel::Music, Channel::Mic]
        );
        let snapshot = hub.poll().unwrap();
        assert_eq!(
            snapshot.faders.map(|view| (view.channel, view.volume)),
            [
                (Channel::System, 196),
                (Channel::Chat, 178),
                (Channel::Music, 120),
                (Channel::Mic, 214),
            ]
        );
    }

    #[test]
    fn giving_a_fader_the_channel_it_has_changes_nothing() {
        let (mut hub, hands) = hub();
        let before = hands.state();
        hub.apply(Intent::AssignFader {
            fader: Fader::C,
            channel: Channel::Music,
        })
        .unwrap();
        assert_eq!(hands.state(), before);
    }

    type Positions = std::sync::Arc<std::sync::Mutex<[u8; 4]>>;

    /// A device whose faders lag behind: they report what the test says,
    /// whatever volume they were sent.
    struct Lagging {
        device: Box<dyn Device>,
        positions: Positions,
    }

    impl Device for Lagging {
        fn info(&self) -> &goxlr_hub_device::DeviceInfo {
            self.device.info()
        }
        fn status(&mut self) -> Result<goxlr_hub_protocol::Status, DeviceError> {
            let mut status = self.device.status()?;
            status.faders = *self.positions.lock().unwrap();
            Ok(status)
        }
        fn mic_level(&mut self) -> Result<u16, DeviceError> {
            self.device.mic_level()
        }
        fn set_fader(&mut self, fader: Fader, channel: Channel) -> Result<(), DeviceError> {
            self.device.set_fader(fader, channel)
        }
        fn set_volume(&mut self, channel: Channel, volume: u8) -> Result<(), DeviceError> {
            self.device.set_volume(channel, volume)
        }
        fn set_muted(&mut self, channel: Channel, muted: bool) -> Result<(), DeviceError> {
            self.device.set_muted(channel, muted)
        }
        fn set_mic_input_muted(&mut self, muted: bool) -> Result<(), DeviceError> {
            self.device.set_mic_input_muted(muted)
        }
        fn set_button_lights(&mut self, lights: ButtonLights) -> Result<(), DeviceError> {
            self.device.set_button_lights(lights)
        }
        fn set_routing(
            &mut self,
            input: goxlr_hub_protocol::RoutingInput,
            outputs: goxlr_hub_protocol::OutputSet,
        ) -> Result<(), DeviceError> {
            self.device.set_routing(input, outputs)
        }
        fn set_mic_gain(
            &mut self,
            mic_type: goxlr_hub_protocol::MicType,
            gain: u16,
        ) -> Result<(), DeviceError> {
            self.device.set_mic_gain(mic_type, gain)
        }
    }

    fn lagging() -> (Hub, Positions) {
        let (device, _) = open_virtual().unwrap();
        let positions = Positions::new(std::sync::Mutex::new([214, 178, 120, 196]));
        let device = Lagging {
            device: Box::new(device),
            positions: positions.clone(),
        };
        let mut hub = Hub::connect(Box::new(device)).unwrap();
        hub.poll().unwrap();
        (hub, positions)
    }

    #[test]
    fn a_fader_sent_somewhere_is_not_believed_while_its_motor_travels() {
        let (mut hub, positions) = lagging();
        hub.apply(Intent::SetVolume {
            channel: Channel::Music,
            volume: 20,
        })
        .unwrap();

        for position in [120, 90, 60, 30, 22] {
            positions.lock().unwrap()[2] = position;
            assert_eq!(hub.poll().unwrap().faders[2].volume, 20, "at {position}");
        }
        // Arrived: the fader is believed again.
        assert_eq!(hub.poll().unwrap().faders[2].volume, 22);
        positions.lock().unwrap()[2] = 80;
        assert_eq!(hub.poll().unwrap().faders[2].volume, 80);
    }

    #[test]
    fn a_fader_that_never_arrives_is_believed_after_a_while() {
        let (mut hub, _) = lagging();
        hub.apply(Intent::SetVolume {
            channel: Channel::Music,
            volume: 20,
        })
        .unwrap();
        // A hand holds the fader at 120.
        let seen: Vec<u8> = (0..30)
            .map(|_| hub.poll().unwrap().faders[2].volume)
            .collect();
        assert_eq!(seen[0], 20);
        assert_eq!(seen[29], 120);
    }

    #[test]
    fn a_volume_the_app_does_not_know_is_sent_as_nothing() {
        let (device, hands) = open_virtual().unwrap();
        hands.move_fader(Fader::A, 77);
        let mut hub = Hub::adopt(Box::new(device), None).unwrap();
        let snapshot = hub.poll().unwrap();
        assert_eq!(snapshot.channels[at(Channel::Mic)].volume, Some(77));
        assert_eq!(snapshot.channels[at(Channel::Headphones)].volume, None);

        let value = serde_json::to_value(&snapshot).unwrap();
        assert_eq!(
            value["channels"][8],
            json!({ "channel": "headphones", "volume": null, "muted": false, "fader": null })
        );
    }

    #[test]
    fn an_unknown_channel_put_under_a_fader_takes_the_volume_the_fader_shows() {
        let (device, hands) = open_virtual().unwrap();
        hands.move_fader(Fader::D, 50);
        let mut hub = Hub::adopt(Box::new(device), None).unwrap();
        hub.poll().unwrap();

        hub.apply(Intent::AssignFader {
            fader: Fader::D,
            channel: Channel::Game,
        })
        .unwrap();
        // The virtual fader now sits at the volume of Game: zero.
        let snapshot = hub.poll().unwrap();
        assert_eq!(snapshot.faders[3].volume, 0);
        assert_eq!(snapshot.channels[at(Channel::Game)].volume, Some(0));
        assert_eq!(snapshot.channels[at(Channel::System)].volume, Some(50));
        assert_eq!(hands.state().volumes[at(Channel::System)], 50);
    }

    #[test]
    fn intents_are_read_from_what_the_interface_sends() {
        let read = |value| serde_json::from_value::<Intent>(value).unwrap();
        assert_eq!(
            read(json!({ "type": "setVolume", "channel": "micMonitor", "volume": 12 })),
            Intent::SetVolume {
                channel: Channel::MicMonitor,
                volume: 12
            }
        );
        assert_eq!(
            read(json!({ "type": "setMuted", "channel": "chat", "muted": true })),
            Intent::SetMuted {
                channel: Channel::Chat,
                muted: true
            }
        );
        assert_eq!(
            read(json!({ "type": "setMicOff", "off": true })),
            Intent::SetMicOff { off: true }
        );
        assert_eq!(
            read(json!({ "type": "assignFader", "fader": "d", "channel": "lineIn" })),
            Intent::AssignFader {
                fader: Fader::D,
                channel: Channel::LineIn
            }
        );
        assert_eq!(
            read(
                json!({ "type": "setRoute", "input": "lineIn", "output": "broadcastMix", "on": false })
            ),
            route(RoutingInput::LineIn, RoutingOutput::BroadcastMix, false)
        );
        for refused in [
            json!({ "type": "setRoute", "input": "headphones", "output": "chatMic", "on": true }),
            json!({ "type": "setRoute", "input": "mic", "output": "mic", "on": true }),
            json!({ "type": "setRoute", "input": "mic", "output": "sampler" }),
            json!({ "type": "setVolume", "channel": "music", "volume": 256 }),
            json!({ "type": "setVolume", "channel": "nowhere", "volume": 1 }),
            json!({ "type": "assignFader", "fader": "e", "channel": "mic" }),
            json!({ "type": "selfDestruct" }),
        ] {
            assert!(
                serde_json::from_value::<Intent>(refused.clone()).is_err(),
                "{refused}"
            );
        }
    }

    #[test]
    fn the_microphone_level_is_always_a_number_the_interface_can_draw() {
        let (mut hub, _) = hub();
        let mut levels = Vec::new();
        for _ in 0..300 {
            let level = hub.poll().unwrap().mic_level_db;
            assert!(level.is_finite() && (-72.2..=0.0).contains(&level));
            levels.push(level);
        }
        levels.dedup();
        assert!(levels.len() > 200, "the meter moves");
    }

    #[test]
    fn the_snapshot_is_sent_in_the_shape_the_interface_expects() {
        let (mut hub, hands) = hub();
        hands.press(Button::MicMute);
        let value = serde_json::to_value(hub.poll().unwrap()).unwrap();

        assert_eq!(value["connection"], json!({ "state": "demo" }));
        assert_eq!(
            value["device"],
            json!({ "kind": "virtual", "firmware": "1.4.3.110", "serial": "VIRTUAL" })
        );
        assert_eq!(
            value["faders"][1],
            json!({ "fader": "b", "channel": "chat", "volume": 178, "muted": false })
        );
        assert_eq!(
            value["channels"][9],
            json!({ "channel": "micMonitor", "volume": 255, "muted": false, "fader": null })
        );
        assert_eq!(
            value["routing"][0],
            json!({ "input": "mic", "outputs": ["broadcastMix", "chatMic"] })
        );
        assert_eq!(
            value["routing"][7],
            json!({
                "input": "samples",
                "outputs": ["headphones", "broadcastMix", "chatMic", "lineOut"]
            })
        );
        assert_eq!(value["pressed"], json!(["micMute"]));
        assert_eq!(value["micOff"], json!(false));
        assert!(value["micLevelDb"].is_number());
    }

    #[test]
    fn a_device_that_stops_answering_is_an_error_not_a_crash() {
        struct Dead(goxlr_hub_device::DeviceInfo);
        impl Device for Dead {
            fn info(&self) -> &goxlr_hub_device::DeviceInfo {
                &self.0
            }
            fn status(&mut self) -> Result<goxlr_hub_protocol::Status, DeviceError> {
                Err(DeviceError::Link("unplugged".into()))
            }
            fn mic_level(&mut self) -> Result<u16, DeviceError> {
                Err(DeviceError::Link("unplugged".into()))
            }
            fn set_fader(&mut self, _: Fader, _: Channel) -> Result<(), DeviceError> {
                Ok(())
            }
            fn set_volume(&mut self, _: Channel, _: u8) -> Result<(), DeviceError> {
                Ok(())
            }
            fn set_muted(&mut self, _: Channel, _: bool) -> Result<(), DeviceError> {
                Ok(())
            }
            fn set_mic_input_muted(&mut self, _: bool) -> Result<(), DeviceError> {
                Ok(())
            }
            fn set_button_lights(&mut self, _: ButtonLights) -> Result<(), DeviceError> {
                Ok(())
            }
            fn set_routing(
                &mut self,
                _: goxlr_hub_protocol::RoutingInput,
                _: goxlr_hub_protocol::OutputSet,
            ) -> Result<(), DeviceError> {
                Ok(())
            }
            fn set_mic_gain(
                &mut self,
                _: goxlr_hub_protocol::MicType,
                _: u16,
            ) -> Result<(), DeviceError> {
                Ok(())
            }
        }

        let info = goxlr_hub_device::DeviceInfo {
            kind: DeviceKind::Hardware,
            firmware: String::new(),
            serial: String::new(),
            manufactured: String::new(),
        };
        let mut hub = Hub::connect(Box::new(Dead(info))).unwrap();
        let before = hub.mixer().clone();
        assert!(matches!(hub.poll(), Err(DeviceError::Link(_))));
        assert_eq!(hub.mixer(), &before);
    }
}
