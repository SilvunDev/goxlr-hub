//! What the app knows about the device, and the picture of it the interface
//! draws. The interface never reaches the device: it sends an [`Intent`] and
//! receives a [`Snapshot`].

pub mod backup;
mod controls;
mod gestures;
mod library;
mod mic;
mod station;
mod studio;

use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use goxlr_hub_device::{Device, DeviceError, DeviceKind};
use goxlr_hub_protocol::{
    Button, ButtonLight, ButtonLights, ButtonSet, Channel, EqBand, Fader, MicType, OutputSet,
    RoutingInput, RoutingOutput, mic_level_db,
};
use serde::{Deserialize, Serialize};

pub use controls::{
    Action, AudioTarget, Bank, ButtonActions, ButtonView, Controls, ControlsView, DOUBLE_PRESS_MS,
    LONG_PRESS_MS, MuteMode,
};
pub use gestures::{Clock, Gesture, SystemClock};
use gestures::{Event, Recognizer};
pub use library::{Assembly, FORMAT, Kind, Library, MixPiece, ProfileError, valid_name};
pub use mic::{
    Compressor, CompressorSetting, EqBandView, EqPoint, Gate, GateSetting, MAX_GAIN_DB, MicBlock,
    MicState, MicView,
};
pub use station::{Connection, ConnectionView, Port, Station};
pub use studio::{ActiveView, Dirty, ProfileCommand, ProfilesView, Studio};

/// A motorised fader is considered arrived this close to where it was sent.
const FADER_TOLERANCE: u8 = 5;

/// Readings a travelling fader is given before it is believed again: about
/// a second.
const FADER_TRAVEL_READINGS: u8 = 20;

/// A button stays lit on screen this long after it was let go, so that a
/// quick press can be seen.
const AFTERGLOW: Duration = Duration::from_millis(600);

/// Counts every press of a button, in the whole app, so that two presses of
/// the same button are two for whoever looks, whichever device saw them.
static PRESSES: AtomicU32 = AtomicU32::new(0);

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
    /// The bank the pads are on. Like the mutes, it is of the moment and no
    /// profile keeps it.
    pub bank: Bank,
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
            bank: Bank::default(),
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

/// Everything the app sends a device, since the device cannot tell it.
#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    pub mixer: MixerState,
    pub mic: MicState,
    pub controls: Controls,
}

impl Settings {
    /// The settings of a real device the app meets.
    pub fn unknown() -> Self {
        Self {
            mixer: MixerState::unknown(),
            mic: MicState::unknown(),
            controls: Controls::default(),
        }
    }
}

/// What the interface asks for.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
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
    /// Says how the microphone is plugged in. A condenser gets phantom
    /// power.
    SetMicType {
        mic_type: MicType,
    },
    /// Sets the gain of the microphone type in use, 0 to 72 dB.
    SetMicGain {
        gain: u8,
    },
    SetGate {
        setting: GateSetting,
        value: i32,
    },
    SetCompressor {
        setting: CompressorSetting,
        value: i32,
    },
    /// Moves a band of the equaliser: its frequency in hertz, its gain in
    /// decibels.
    SetEqBand {
        band: EqBand,
        frequency: f32,
        gain: i8,
    },
    /// 0 to 100.
    SetDeEsser {
        amount: u8,
    },
    /// Puts a part of the microphone processing back to neutral.
    ResetMic {
        block: MicBlock,
    },
    /// Gives a gesture of a button an action, or none. A hold takes the
    /// other gestures of the button away, and they take the hold away.
    SetGesture {
        button: Button,
        gesture: Gesture,
        action: Option<Action>,
    },
    /// Puts a button, or every button and the times of a press, back to what
    /// they are when nobody chose.
    ResetControls {
        button: Option<Button>,
    },
    /// How long a press lasts to be long, and how long a second press is
    /// waited for, in milliseconds.
    SetPressTimes {
        long_press_ms: u16,
        double_press_ms: u16,
    },
    /// Presses or releases a button of the virtual device, as a mouse click
    /// would. The real device has real fingers.
    PressButton {
        button: Button,
        down: bool,
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
    /// How the microphone is plugged in and processed.
    pub mic: MicView,
    /// What is saved, what is in use and what changed since. Filled in by
    /// the [`Studio`].
    pub profiles: ProfilesView,
    /// What each button does.
    pub controls: ControlsView,
    /// Buttons held down, or let go a moment ago.
    pub touched: Vec<Button>,
    /// The button pressed last, and how many presses there were.
    pub last_press: Option<LastPress>,
    /// The bank the pads are on.
    pub bank: Bank,
}

/// A button went down.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct LastPress {
    pub button: Button,
    /// Changes at every press, of any button.
    pub count: u32,
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

/// What a mute action acts on, once the fader it names is looked up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Silenced {
    Mic,
    Channel(Channel),
}

/// A hold that began: what it acts on. Kept as it was found, so that a
/// profile loaded meanwhile changes nothing of it.
struct Hold {
    button: Button,
    targets: Vec<Silenced>,
}

/// The app's side of one connected device.
pub struct Hub {
    device: Box<dyn Device>,
    mixer: MixerState,
    mic: MicState,
    controls: Controls,
    travels: [Option<Travel>; Fader::COUNT],
    /// The buttons held at the last reading, to tell a press from a hold.
    held: Option<ButtonSet>,
    clock: Box<dyn Clock>,
    recognizer: Recognizer,
    holds: Vec<Hold>,
    /// What each target of a hold was before the first hold on it, and
    /// whether it was silenced: put back when the last hold on it ends.
    before_holds: Vec<(Silenced, bool)>,
    /// When each button was last seen down.
    last_down: [Option<Duration>; Button::ALL.len()],
    last_press: Option<LastPress>,
}

impl Hub {
    /// Takes a device over and brings it to the app's state.
    pub fn connect(device: Box<dyn Device>) -> Result<Self, DeviceError> {
        Self::take(
            device,
            Settings {
                mixer: MixerState::default(),
                mic: MicState::default(),
                controls: Controls::default(),
            },
        )
    }

    /// Takes a real device over, changing how it sounds as little as the
    /// truth of the screen allows.
    ///
    /// The fader assignment, the mutes, the routing, the microphone
    /// processing and the mute lights are sent, because the device cannot
    /// tell them. Volumes, and the type and gain of the microphone, are only
    /// sent when the app knows them: those of a device that dropped and came
    /// back, or that were set on screen. The other volumes are read from the
    /// faders, or stay unknown.
    pub fn adopt(
        device: Box<dyn Device>,
        remembered: Option<&Settings>,
    ) -> Result<Self, DeviceError> {
        Self::take(
            device,
            remembered.cloned().unwrap_or_else(Settings::unknown),
        )
    }

    fn take(device: Box<dyn Device>, settings: Settings) -> Result<Self, DeviceError> {
        let mut hub = Self {
            device,
            mixer: settings.mixer,
            mic: settings.mic,
            controls: settings.controls,
            travels: [None; Fader::COUNT],
            held: None,
            clock: Box::new(SystemClock::default()),
            recognizer: Recognizer::default(),
            holds: Vec::new(),
            before_holds: Vec::new(),
            last_down: [None; Button::ALL.len()],
            last_press: None,
        };
        hub.send_all()?;
        Ok(hub)
    }

    /// Brings the device to other settings, all of them: those of a profile.
    pub fn load(&mut self, settings: Settings) -> Result<(), DeviceError> {
        self.mixer = settings.mixer;
        self.mic = settings.mic;
        self.controls = settings.controls;
        self.travels = [None; Fader::COUNT];
        self.send_all()
    }

    fn send_all(&mut self) -> Result<(), DeviceError> {
        // A device that was just plugged in plays with its own settings:
        // what silences is sent first.
        for (input, outputs) in RoutingInput::ALL.into_iter().zip(self.mixer.routing) {
            self.device.set_routing(input, outputs)?;
        }
        for channel in Channel::ALL {
            if channel != Channel::Mic {
                let muted = self.mixer.muted[usize::from(channel.index())];
                self.device.set_muted(channel, muted)?;
            }
        }
        self.send_mic(self.mixer.mic_silenced())?;
        for (fader, channel) in Fader::ALL.into_iter().zip(self.mixer.faders) {
            self.device.set_fader(fader, channel)?;
        }
        for channel in Channel::ALL {
            if let Some(volume) = self.mixer.volumes[usize::from(channel.index())] {
                self.send_volume(channel, volume)?;
            }
        }
        self.mic.send_all(self.device.as_mut())?;
        self.send_lights()
    }

    pub fn mixer(&self) -> &MixerState {
        &self.mixer
    }

    pub fn mic(&self) -> &MicState {
        &self.mic
    }

    pub fn controls(&self) -> &Controls {
        &self.controls
    }

    /// What to send the device again, should it drop and come back.
    pub fn settings(&self) -> Settings {
        Settings {
            mixer: self.mixer.clone(),
            mic: self.mic.clone(),
            controls: self.controls.clone(),
        }
    }

    /// Puts back what the buttons held down have changed, without a word to
    /// the device: for when it is let go, or gone, and no finger will ever
    /// come up.
    pub fn release_holds(&mut self) {
        self.holds.clear();
        for (target, silenced) in std::mem::take(&mut self.before_holds) {
            self.set_silenced(target, silenced);
        }
    }

    /// Forgets the presses the device showed before: for when it was out of
    /// sight, and a button seen down then may be up now.
    pub fn forget_presses(&mut self) {
        self.recognizer = Recognizer::default();
        self.held = None;
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
            Intent::SetMicType { mic_type } => self.mic.set_type(self.device.as_mut(), mic_type),
            Intent::SetMicGain { gain } => self.mic.set_gain(self.device.as_mut(), gain),
            Intent::SetGate { setting, value } => {
                self.mic.set_gate(self.device.as_mut(), setting, value)
            }
            Intent::SetCompressor { setting, value } => {
                self.mic
                    .set_compressor(self.device.as_mut(), setting, value)
            }
            Intent::SetEqBand {
                band,
                frequency,
                gain,
            } => self
                .mic
                .set_eq_band(self.device.as_mut(), band, frequency, gain),
            Intent::SetDeEsser { amount } => self.mic.set_de_esser(self.device.as_mut(), amount),
            Intent::ResetMic { block } => self.mic.reset(self.device.as_mut(), block),
            Intent::SetGesture {
                button,
                gesture,
                action,
            } => {
                self.controls.set(button, gesture, action);
                self.send_lights()
            }
            Intent::ResetControls { button } => {
                match button {
                    Some(button) => self.controls.reset_button(button),
                    None => self.controls.reset(),
                }
                self.send_lights()
            }
            Intent::SetPressTimes {
                long_press_ms,
                double_press_ms,
            } => {
                self.controls.set_times(long_press_ms, double_press_ms);
                Ok(())
            }
            // Only the virtual device can be pressed from the screen, and the
            // station does it.
            Intent::PressButton { .. } => Ok(()),
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

    /// What a mute action acts on, with the fader it names looked up.
    fn silenced_by(&self, target: AudioTarget) -> Silenced {
        match target {
            AudioTarget::Mic => Silenced::Mic,
            AudioTarget::Channel { channel } => Silenced::Channel(channel),
            AudioTarget::FaderTrack { fader } => {
                Silenced::Channel(self.mixer.faders[usize::from(fader.index())])
            }
        }
    }

    fn is_silenced(&self, target: Silenced) -> bool {
        match target {
            Silenced::Mic => self.mixer.mic_off,
            Silenced::Channel(channel) => self.mixer.muted[usize::from(channel.index())],
        }
    }

    /// Writes what is silenced in the mixer only, without a word to the device.
    fn set_silenced(&mut self, target: Silenced, silenced: bool) {
        match target {
            Silenced::Mic => self.mixer.mic_off = silenced,
            Silenced::Channel(channel) => {
                self.mixer.muted[usize::from(channel.index())] = silenced;
            }
        }
    }

    fn silence(&mut self, target: Silenced, silenced: bool) -> Result<(), DeviceError> {
        match target {
            Silenced::Mic => self.set_mic_off(silenced),
            Silenced::Channel(channel) => self.set_muted(channel, silenced),
        }
    }

    /// Whether what an action does is in force: a track silenced for an
    /// action that silences it, open for one that opens it, the bank the
    /// action switches to.
    fn in_force(&self, action: Action) -> bool {
        match action {
            Action::Mute { target, mode } => {
                let silenced = self.is_silenced(self.silenced_by(target));
                silenced != (mode == MuteMode::Unmute)
            }
            Action::Bank { bank } => self.mixer.bank == bank,
        }
    }

    /// A button is lit when what one of its actions does is in force.
    fn lights(&self) -> ButtonLights {
        let mut lights = ButtonLights::default();
        for button in Button::ALL {
            if self
                .controls
                .actions(button)
                .all()
                .any(|action| self.in_force(action))
            {
                lights.set(button, ButtonLight::Lit);
            }
        }
        lights
    }

    fn send_lights(&mut self) -> Result<(), DeviceError> {
        self.device.set_button_lights(self.lights())
    }

    /// Does what an action says.
    fn fire(&mut self, action: Action) -> Result<(), DeviceError> {
        match action {
            Action::Mute { target, mode } => {
                let target = self.silenced_by(target);
                let silenced = self.is_silenced(target);
                let wanted = match mode {
                    MuteMode::Mute => true,
                    MuteMode::Unmute => false,
                    MuteMode::Toggle => !silenced,
                };
                if wanted == silenced {
                    return Ok(());
                }
                self.silence(target, wanted)
            }
            Action::Bank { bank } => {
                self.mixer.bank = bank;
                self.send_lights()
            }
        }
    }

    /// Begins a hold: does what the action says, and notes what to put back.
    /// Another hold on the same target finds it already held: what it was
    /// before the first one is what comes back.
    fn begin_hold(&mut self, button: Button, action: Action) -> Result<(), DeviceError> {
        let mut targets = Vec::new();
        if let Action::Mute { target, .. } = action {
            let target = self.silenced_by(target);
            if !self.before_holds.iter().any(|(held, _)| *held == target) {
                self.before_holds.push((target, self.is_silenced(target)));
            }
            targets.push(target);
        }
        self.holds.push(Hold { button, targets });
        self.fire(action)
    }

    /// Ends a hold. What it acted on goes back to what it was before, once no
    /// other hold acts on it. The mixer says so even when the device does
    /// not answer, so that nothing stays silenced for a finger long gone.
    fn end_hold(&mut self, button: Button) -> Result<(), DeviceError> {
        let Some(at) = self.holds.iter().position(|hold| hold.button == button) else {
            return Ok(());
        };
        let ended = self.holds.remove(at);
        let mut sent = Ok(());
        for target in ended.targets {
            if self.holds.iter().any(|hold| hold.targets.contains(&target)) {
                continue;
            }
            let Some(at) = self
                .before_holds
                .iter()
                .position(|(held, _)| *held == target)
            else {
                continue;
            };
            let (_, silenced) = self.before_holds.remove(at);
            if self.is_silenced(target) != silenced {
                self.set_silenced(target, silenced);
                if sent.is_ok() {
                    sent = self.send_silenced(target);
                }
            }
        }
        sent
    }

    /// Tells the device what the mixer says of a target.
    fn send_silenced(&mut self, target: Silenced) -> Result<(), DeviceError> {
        match target {
            Silenced::Mic | Silenced::Channel(Channel::Mic) => {
                self.send_mic(self.mixer.mic_silenced())?;
            }
            Silenced::Channel(channel) => {
                let muted = self.mixer.muted[usize::from(channel.index())];
                self.device.set_muted(channel, muted)?;
            }
        }
        self.send_lights()
    }

    /// Notes which buttons went down, and does what the gestures that are
    /// over are for.
    fn follow_buttons(&mut self, pressed: ButtonSet, now: Duration) -> Result<(), DeviceError> {
        // Buttons already down when the device is taken over are no press.
        let held = self.held.replace(pressed).unwrap_or(pressed);
        for button in pressed.iter() {
            self.last_down[usize::from(button.bit())] = Some(now);
            if !held.contains(button) {
                self.last_press = Some(LastPress {
                    button,
                    count: PRESSES.fetch_add(1, Ordering::Relaxed) + 1,
                });
            }
        }

        let events = self.recognizer.update(
            now,
            pressed,
            |button| self.controls.wanted(button),
            self.controls.timing(),
        );
        for (button, event) in events {
            match event {
                Event::Fire(gesture) => {
                    if let Some(action) = self.controls.action(button, gesture) {
                        self.fire(action)?;
                    }
                }
                Event::HoldStart => {
                    if let Some(action) = self.controls.action(button, Gesture::Hold) {
                        self.begin_hold(button, action)?;
                    }
                }
                Event::HoldEnd => self.end_hold(button)?,
            }
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

        let now = self.clock.now();
        self.follow_buttons(status.pressed, now)?;

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
            mic: self.mic.view(),
            profiles: ProfilesView::default(),
            controls: self.controls.view(),
            touched: Button::ALL
                .into_iter()
                .filter(|button| {
                    status.pressed.contains(*button)
                        || self.last_down[usize::from(button.bit())]
                            .is_some_and(|seen| now.saturating_sub(seen) < AFTERGLOW)
                })
                .collect(),
            last_press: self.last_press,
            bank: self.mixer.bank,
        })
    }
}

#[cfg(test)]
mod controls_tests;

#[cfg(test)]
mod tests {
    use goxlr_hub_device::open_virtual;
    use goxlr_hub_protocol::Side;
    use serde_json::json;

    use super::*;

    /// What the buttons show when nothing is muted: the pads are on bank A.
    pub(crate) fn resting_lights() -> ButtonLights {
        let mut lights = ButtonLights::default();
        lights.set(Button::SamplerSelectA, ButtonLight::Lit);
        lights
    }

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
        let mut lights = resting_lights();
        lights.set(Button::Fader2Mute, ButtonLight::Lit);
        assert_eq!(device.lights, lights);
        assert!(hub.poll().unwrap().faders[1].muted);

        hub.apply(mute(Channel::Chat, false)).unwrap();
        assert!(!hands.state().muted[at(Channel::Chat)]);
        assert_eq!(hands.state().lights, resting_lights());
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
        assert_eq!(device.lights, resting_lights());
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
        fn set_effect(
            &mut self,
            key: goxlr_hub_protocol::EffectKey,
            value: i32,
        ) -> Result<(), DeviceError> {
            self.device.set_effect(key, value)
        }
        fn set_mic_param(
            &mut self,
            key: goxlr_hub_protocol::MicParamKey,
            value: f32,
        ) -> Result<(), DeviceError> {
            self.device.set_mic_param(key, value)
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
        assert_eq!(
            read(json!({ "type": "setMicType", "micType": "condenser" })),
            Intent::SetMicType {
                mic_type: MicType::Condenser
            }
        );
        assert_eq!(
            read(json!({ "type": "setMicGain", "gain": 40 })),
            Intent::SetMicGain { gain: 40 }
        );
        assert_eq!(
            read(json!({ "type": "setGate", "setting": "attenuation", "value": 80 })),
            Intent::SetGate {
                setting: GateSetting::Attenuation,
                value: 80
            }
        );
        assert_eq!(
            read(json!({ "type": "setCompressor", "setting": "makeupGain", "value": -3 })),
            Intent::SetCompressor {
                setting: CompressorSetting::MakeupGain,
                value: -3
            }
        );
        assert_eq!(
            read(json!({ "type": "setEqBand", "band": "khz4", "frequency": 3500, "gain": -4 })),
            Intent::SetEqBand {
                band: EqBand::Khz4,
                frequency: 3500.0,
                gain: -4
            }
        );
        assert_eq!(
            read(json!({ "type": "setEqBand", "band": "hz31", "frequency": 31.5, "gain": 0 })),
            Intent::SetEqBand {
                band: EqBand::Hz31,
                frequency: 31.5,
                gain: 0
            }
        );
        assert_eq!(
            read(json!({ "type": "setDeEsser", "amount": 100 })),
            Intent::SetDeEsser { amount: 100 }
        );
        assert_eq!(
            read(json!({ "type": "resetMic", "block": "deEsser" })),
            Intent::ResetMic {
                block: MicBlock::DeEsser
            }
        );
        assert_eq!(
            read(json!({ "type": "resetMic", "block": "all" })),
            Intent::ResetMic {
                block: MicBlock::All
            }
        );
        for refused in [
            json!({ "type": "resetMic", "block": "gain" }),
            json!({ "type": "resetMic" }),
            json!({ "type": "setMicType", "micType": "ribbon" }),
            json!({ "type": "setMicGain", "gain": -1 }),
            json!({ "type": "setGate", "setting": "ratio", "value": 1 }),
            json!({ "type": "setGate", "setting": "threshold", "value": 1.5 }),
            json!({ "type": "setCompressor", "setting": "threshold" }),
            json!({ "type": "setEqBand", "band": "hz90", "frequency": 90, "gain": 0 }),
            json!({ "type": "setEqBand", "band": "hz31", "frequency": "low", "gain": 0 }),
            json!({ "type": "setEqBand", "band": "hz31", "frequency": 40, "gain": 200 }),
            json!({ "type": "setDeEsser", "amount": 256 }),
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

        let mic = &value["mic"];
        assert_eq!(mic["micType"], json!("dynamic"));
        assert_eq!(mic["gain"], json!(30));
        assert_eq!(
            mic["gate"],
            json!({ "threshold": -30, "attenuation": 100, "attack": 0, "release": 19 })
        );
        assert_eq!(
            mic["compressor"],
            json!({ "threshold": 0, "ratio": 9, "attack": 1, "release": 9, "makeupGain": 0 })
        );
        assert_eq!(mic["equalizer"].as_array().unwrap().len(), 10);
        assert_eq!(
            mic["equalizer"][0],
            json!({
                "band": "hz31", "frequency": 31.5, "gain": 0,
                "minFrequency": 30.0, "maxFrequency": 63.0_f32 / 1.12
            })
        );
        assert_eq!(
            mic["equalizer"][5],
            json!({
                "band": "khz1", "frequency": 1000.0, "gain": 0,
                "minFrequency": 500.0_f32 * 1.12, "maxFrequency": 2000.0_f32 / 1.12
            })
        );
        assert_eq!(mic["deEsser"], json!(0));
    }

    #[test]
    fn a_microphone_nobody_described_is_shown_as_unknown() {
        let (device, hands) = open_virtual().unwrap();
        let mut hub = Hub::adopt(Box::new(device), None).unwrap();
        // The processing was sent, the type and the gain were not.
        assert!(!hands.state().mic_gain_set);
        assert_eq!(hands.state().effects.len(), 33);

        hub.apply(Intent::SetMicGain { gain: 50 }).unwrap();
        assert!(!hands.state().mic_gain_set);
        let value = serde_json::to_value(hub.poll().unwrap()).unwrap();
        assert_eq!(value["mic"]["micType"], json!(null));
        assert_eq!(value["mic"]["gain"], json!(null));

        hub.apply(Intent::SetMicType {
            mic_type: MicType::Jack,
        })
        .unwrap();
        hub.apply(Intent::SetMicGain { gain: 12 }).unwrap();
        let state = hands.state();
        assert_eq!((state.mic_type, state.mic_gain), (MicType::Jack, 12));
        let snapshot = hub.poll().unwrap();
        assert_eq!(snapshot.mic.mic_type, Some(MicType::Jack));
        assert_eq!(snapshot.mic.gain, Some(12));
    }

    #[test]
    fn what_the_screen_asks_of_the_microphone_reaches_the_device_and_the_picture() {
        use goxlr_hub_protocol::{EffectKey, MicParamKey};

        let (mut hub, hands) = hub();
        for intent in [
            Intent::SetGate {
                setting: GateSetting::Threshold,
                value: -40,
            },
            Intent::SetCompressor {
                setting: CompressorSetting::Ratio,
                value: 11,
            },
            Intent::SetEqBand {
                band: EqBand::Hz125,
                frequency: 150.0,
                gain: 4,
            },
            Intent::SetDeEsser { amount: 25 },
        ] {
            hub.apply(intent).unwrap();
        }

        let device = hands.state();
        assert_eq!(device.effects[&EffectKey::GateThreshold], -40);
        assert_eq!(device.mic_params[&MicParamKey::GateThreshold], -40.0);
        assert_eq!(device.effects[&EffectKey::CompressorRatio], 11);
        assert_eq!(device.mic_params[&MicParamKey::CompressorRatio], 8.0);
        assert_eq!(device.effects[&EffectKey::EqFrequency(EqBand::Hz125)], 70);
        assert_eq!(device.effects[&EffectKey::EqGain(EqBand::Hz125)], 4);
        assert_eq!(device.effects[&EffectKey::DeEsser], 25);

        let mic = hub.poll().unwrap().mic;
        assert_eq!(mic.gate.threshold, -40);
        assert_eq!(mic.compressor.ratio, 11);
        assert_eq!(
            (mic.equalizer[2].frequency, mic.equalizer[2].gain),
            (150.0, 4)
        );
        // The neighbours of the band that moved can now go nearly as far as
        // it: two bands never share a frequency.
        assert_eq!(mic.equalizer[1].max_frequency, 150.0 / 1.12);
        assert_eq!(mic.equalizer[3].min_frequency, 150.0 * 1.12);
        assert_eq!(mic.de_esser, 25);
    }

    #[test]
    fn each_part_of_the_microphone_processing_goes_back_to_neutral_alone() {
        use goxlr_hub_protocol::{EffectKey, MicParamKey};

        let reset = |block| Intent::ResetMic { block };
        let (mut hub, hands) = hub();
        let neutral = hub.poll().unwrap().mic;
        let change = |hub: &mut Hub| {
            for intent in [
                Intent::SetMicGain { gain: 44 },
                Intent::SetGate {
                    setting: GateSetting::Threshold,
                    value: -40,
                },
                Intent::SetGate {
                    setting: GateSetting::Release,
                    value: 5,
                },
                Intent::SetCompressor {
                    setting: CompressorSetting::Ratio,
                    value: 11,
                },
                Intent::SetCompressor {
                    setting: CompressorSetting::MakeupGain,
                    value: 4,
                },
                Intent::SetEqBand {
                    band: EqBand::Hz125,
                    frequency: 150.0,
                    gain: 4,
                },
                Intent::SetDeEsser { amount: 25 },
            ] {
                hub.apply(intent).unwrap();
            }
        };

        change(&mut hub);
        hub.apply(reset(MicBlock::Gate)).unwrap();
        let mic = hub.poll().unwrap().mic;
        assert_eq!(mic.gate, neutral.gate);
        assert_eq!(mic.compressor.ratio, 11);
        assert_eq!((mic.equalizer[2].gain, mic.de_esser), (4, 25));
        let device = hands.state();
        assert_eq!(device.effects[&EffectKey::GateThreshold], -30);
        assert_eq!(device.mic_params[&MicParamKey::GateRelease], 19.0);
        assert_eq!(device.effects[&EffectKey::CompressorRatio], 11);

        hub.apply(reset(MicBlock::Compressor)).unwrap();
        let mic = hub.poll().unwrap().mic;
        assert_eq!(mic.compressor, neutral.compressor);
        assert_eq!((mic.equalizer[2].gain, mic.de_esser), (4, 25));
        let device = hands.state();
        assert_eq!(device.effects[&EffectKey::CompressorRatio], 9);
        assert_eq!(device.mic_params[&MicParamKey::CompressorMakeupGain], 0.0);

        hub.apply(reset(MicBlock::Equalizer)).unwrap();
        let mic = hub.poll().unwrap().mic;
        assert_eq!(mic.equalizer, neutral.equalizer);
        assert_eq!(mic.de_esser, 25);
        let device = hands.state();
        assert_eq!(device.effects[&EffectKey::EqGain(EqBand::Hz125)], 0);
        assert_eq!(device.effects[&EffectKey::EqFrequency(EqBand::Hz125)], 63);

        hub.apply(reset(MicBlock::DeEsser)).unwrap();
        assert_eq!(hands.state().effects[&EffectKey::DeEsser], 0);
        // The type and the gain are no processing: they stay.
        let mic = hub.poll().unwrap().mic;
        assert_eq!(mic.gain, Some(44));
        assert_eq!(hands.state().mic_gain, 44);

        change(&mut hub);
        hub.apply(reset(MicBlock::All)).unwrap();
        let mic = hub.poll().unwrap().mic;
        assert_eq!(
            MicView {
                gain: neutral.gain,
                ..mic
            },
            neutral
        );
        let device = hands.state();
        assert_eq!(device.effects[&EffectKey::GateThreshold], -30);
        assert_eq!(device.effects[&EffectKey::CompressorMakeupGain], 0);
        assert_eq!(device.effects[&EffectKey::EqGain(EqBand::Hz125)], 0);
        assert_eq!(device.effects[&EffectKey::DeEsser], 0);
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
            fn set_effect(
                &mut self,
                _: goxlr_hub_protocol::EffectKey,
                _: i32,
            ) -> Result<(), DeviceError> {
                Ok(())
            }
            fn set_mic_param(
                &mut self,
                _: goxlr_hub_protocol::MicParamKey,
                _: f32,
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
