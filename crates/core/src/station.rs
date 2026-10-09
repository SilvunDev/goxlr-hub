//! Chooses the device the app shows: the real GoXLR when it can be had, the
//! virtual one otherwise, and says which and why.

use goxlr_hub_device::{Device, DeviceError, OpenError, VirtualHandle, open_virtual};
use goxlr_hub_protocol::{Button, Channel};
use serde::Serialize;

use crate::{Hub, Intent, Load, MixerState, Settings, Snapshot};

/// Where real devices come from.
pub trait Port: Send {
    /// Tries to open the GoXLR plugged into the computer.
    fn open(&mut self) -> Result<Box<dyn Device>, OpenError>;

    /// The name of a running program that drives the GoXLR too, if any.
    fn rival(&mut self) -> Option<String>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Connection {
    /// No GoXLR: the app shows the virtual device.
    Demo,
    /// The real GoXLR is the device shown.
    Hardware,
    /// A GoXLR is plugged in, but another program drives it.
    Busy { program: String },
    /// A GoXLR Mini is plugged in.
    Unsupported,
    /// A GoXLR is plugged in but could not be opened.
    Unreachable { reason: String },
}

/// The connection as the interface receives it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConnectionView {
    /// `demo`, `hardware`, `busy`, `unsupported` or `unreachable`.
    pub state: &'static str,
    /// The program to close, when `busy`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub program: Option<String>,
}

impl Connection {
    pub(crate) fn view(&self) -> ConnectionView {
        let (state, program) = match self {
            Self::Demo => ("demo", None),
            Self::Hardware => ("hardware", None),
            Self::Busy { program } => ("busy", Some(program.clone())),
            Self::Unsupported => ("unsupported", None),
            Self::Unreachable { .. } => ("unreachable", None),
        };
        ConnectionView { state, program }
    }
}

pub struct Station<P: Port> {
    port: P,
    /// Always there, shown whenever the real device is not.
    demo: Hub,
    /// The fingers on the virtual device.
    hands: VirtualHandle,
    hardware: Option<Hub>,
    connection: Connection,
    /// What the device shown is set to, real or virtual: a real device that
    /// comes, or comes back, is brought to it.
    settings: Settings,
    /// A real device dropped and is expected back.
    lost: bool,
}

/// The virtual device is there to be looked at: where the settings know no
/// volume, it shows a lively one.
fn for_show(settings: &Settings) -> Settings {
    let mut shown = settings.clone();
    let lively = MixerState::default().volumes;
    for (volume, lively) in shown.mixer.volumes.iter_mut().zip(lively) {
        *volume = volume.or(lively);
    }
    shown
}

impl<P: Port> Station<P> {
    /// Starts on the virtual device, set as asked; `scan` looks for the real
    /// one and brings it to the same settings.
    pub fn new(port: P, settings: Settings) -> Result<Self, DeviceError> {
        let (device, hands) = open_virtual()?;
        let mut station = Self {
            port,
            demo: Hub::adopt(Box::new(device), Some(&for_show(&settings)))?,
            hands,
            hardware: None,
            connection: Connection::Demo,
            settings,
            lost: false,
        };
        station.demo_shows_what_is_unknown();
        Ok(station)
    }

    /// The volumes the settings do not know are shown lively on the virtual
    /// device, and are no more known for it: a button or a dial that raises
    /// or lowers one does nothing.
    fn demo_shows_what_is_unknown(&mut self) {
        self.demo
            .mark_invented(self.settings.mixer.volumes.map(|volume| volume.is_none()));
    }

    /// The settings at rest: what a cell held by a finger was before the
    /// finger came is what a profile is compared with and saved as.
    pub fn settings_at_rest(&self) -> Settings {
        let mut settings = self.settings.clone();
        let hub = self.hardware.as_ref().unwrap_or(&self.demo);
        for (input, output, before) in hub.route_holds_before() {
            let outputs = &mut settings.mixer.routing[input as usize];
            *outputs = outputs.with(output, before);
        }
        settings
    }

    /// The cells held by a finger are let be: the routing is about to be
    /// replaced, and what comes will not be put back against it.
    pub(crate) fn forget_route_holds(&mut self) {
        self.demo.forget_route_holds();
        if let Some(hub) = &mut self.hardware {
            hub.forget_route_holds();
        }
    }

    pub fn connection(&self) -> &Connection {
        &self.connection
    }

    /// What the device shown is set to.
    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    /// A real device dropped and is expected back. Until the app has it
    /// again it plays with its own settings, so it is worth looking for it
    /// more often.
    pub fn awaits_return(&self) -> bool {
        self.lost && self.connection == Connection::Demo
    }

    /// Looks for the real device, or checks that it is still ours. Meant to
    /// be called about once a second, more often while a device is awaited.
    pub fn scan(&mut self) {
        if self.hardware.is_some() {
            // Two programs driving the device steal each other's answers:
            // the newcomer gets it.
            if let Some(program) = self.port.rival() {
                self.release();
                self.settings.mixer.forget_volumes();
                self.connection = Connection::Busy { program };
            }
            return;
        }

        self.connection = match self.port.open() {
            Ok(device) => {
                // A button held on the virtual device has no finger left.
                self.demo.release_holds();
                self.let_go_of_virtual_buttons();
                let set = self.demo.take_volumes_set();
                self.keep_from_demo(&set);
                match Hub::adopt(device, Some(&self.settings)) {
                    Ok(hub) => {
                        self.hardware = Some(hub);
                        self.lost = false;
                        Connection::Hardware
                    }
                    Err(error) => Connection::Unreachable {
                        reason: error.to_string(),
                    },
                }
            }
            Err(OpenError::Absent) => Connection::Demo,
            // The other program sets the volumes as it pleases. The fader
            // assignment, the routing and the microphone are put back by the
            // app, the volumes are not.
            Err(OpenError::Busy { program }) => {
                self.settings.mixer.forget_volumes();
                Connection::Busy { program }
            }
            Err(OpenError::Unsupported) => Connection::Unsupported,
            Err(OpenError::Failed(reason)) => Connection::Unreachable { reason },
        };
    }

    /// Does what the interface asked, on the device shown.
    pub fn apply(&mut self, intent: Intent) -> Result<(), DeviceError> {
        // Only the virtual device is pressed or turned from the screen. A
        // button is always let go, whichever device is shown: a click that
        // began on the virtual device may end after the real one came.
        match intent {
            Intent::PressButton { button, down } => {
                if !down {
                    self.hands.release(button);
                } else if self.hardware.is_none() {
                    self.hands.press(button);
                }
                return Ok(());
            }
            Intent::TurnWheel { wheel, notches } => {
                if self.hardware.is_none() {
                    self.hands.turn(wheel, notches);
                }
                return Ok(());
            }
            _ => {}
        }

        if let Some(hub) = &mut self.hardware {
            match hub.apply(intent) {
                Ok(()) => self.settings = hub.settings(),
                Err(_) => self.drop_hardware(),
            }
            return Ok(());
        }
        self.demo.apply(intent)?;
        let set = self.demo.take_volumes_set();
        self.keep_from_demo(&set);
        Ok(())
    }

    /// Brings the device shown to other settings, all of them.
    pub fn load(&mut self, mut settings: Settings) -> Result<(), DeviceError> {
        if let Some(hub) = &mut self.hardware {
            if hub.load(settings.clone()).is_ok() {
                self.settings = settings;
                return Ok(());
            }
            // The finger that held a button cannot come up any more: what it
            // silenced is not loaded.
            hub.release_holds();
            let mixer = hub.settings().mixer;
            settings.mixer.muted = mixer.muted;
            settings.mixer.mic_off = mixer.mic_off;
            // The cell the finger cut is back, the way the hub put it.
            settings.mixer.routing = mixer.routing;
            self.hardware = None;
            self.lost = true;
            self.connection = Connection::Demo;
            self.let_go_of_virtual_buttons();
        }
        self.demo.load(for_show(&settings))?;
        self.settings = settings;
        self.demo_shows_what_is_unknown();
        Ok(())
    }

    /// The virtual device is out of sight, or about to be: its buttons are
    /// let go, and what it saw of them is forgotten.
    fn let_go_of_virtual_buttons(&mut self) {
        for button in Button::ALL {
            self.hands.release(button);
        }
        self.demo.forget_presses();
    }

    /// What was set on the virtual device is kept for the real one, but for
    /// the volumes its faders show: they say nothing of a real device. A
    /// volume is only known once it was set.
    fn keep_from_demo(&mut self, set_volumes: &[Channel]) {
        let mut settings = self.demo.settings();
        for channel in Channel::ALL {
            let at = usize::from(channel.index());
            let set = set_volumes.contains(&channel);
            if self.settings.mixer.volumes[at].is_none() && !set {
                settings.mixer.volumes[at] = None;
            }
        }
        self.settings = settings;
    }

    /// Lets the real device go and shows its settings on the virtual one.
    fn release(&mut self) {
        if let Some(mut hub) = self.hardware.take() {
            // The finger that held a button cannot come up any more.
            hub.release_holds();
            self.settings = hub.settings();
        }
        // The virtual device always answers.
        let _ = self.demo.load(for_show(&self.settings));
        self.demo_shows_what_is_unknown();
        // It was not read while the real one was shown.
        self.demo.forget_presses();
    }

    /// The real device stopped answering: back to the virtual one, keeping
    /// what the real one was set to.
    fn drop_hardware(&mut self) {
        self.release();
        self.lost = true;
        self.connection = Connection::Demo;
    }

    /// The picture to draw: the real device, or the virtual one when the
    /// real one is not there or stops answering.
    pub fn poll(&mut self) -> Result<Snapshot, DeviceError> {
        if let Some(hub) = &mut self.hardware {
            match hub.poll() {
                Ok(snapshot) => {
                    self.settings = hub.settings();
                    return Ok(self.stamp(snapshot));
                }
                Err(_) => self.drop_hardware(),
            }
        }
        let snapshot = self.demo.poll()?;
        // Buttons pressed and dials turned on the virtual device change what
        // is set.
        let set = self.demo.take_volumes_set();
        self.keep_from_demo(&set);
        Ok(self.stamp(snapshot))
    }

    /// The profiles the buttons asked for during the last reading, from
    /// whichever device read them.
    pub(crate) fn take_loads(&mut self) -> Vec<Load> {
        let mut loads = self.demo.take_loads();
        if let Some(hub) = &mut self.hardware {
            loads.extend(hub.take_loads());
        }
        loads
    }

    fn stamp(&self, mut snapshot: Snapshot) -> Snapshot {
        snapshot.connection = self.connection.view();
        snapshot
    }
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};

    use goxlr_hub_device::{DeviceKind, Link, Session, VirtualGoXlr, VirtualHandle};
    use goxlr_hub_protocol::{
        Button, ButtonLight, Channel, EffectKey, EqBand, Fader, MicParamKey, MicType, OutputSet,
        Packet, Request, RoutingInput, RoutingOutput, Side,
    };
    use serde_json::json;

    use super::*;
    use crate::{Action, AudioTarget, CompressorSetting, GateSetting, Gesture, MuteMode};

    /// What the test can do to a fake real device, and see of it.
    #[derive(Clone)]
    struct Bench {
        hands: VirtualHandle,
        unplugged: Arc<AtomicBool>,
        /// Every command the device received.
        received: Arc<Mutex<Vec<Request>>>,
    }

    impl Bench {
        fn settings_received(&self) -> Vec<Request> {
            self.received
                .lock()
                .unwrap()
                .iter()
                .copied()
                .filter(|request| {
                    matches!(
                        request,
                        Request::SetFader { .. }
                            | Request::SetVolume { .. }
                            | Request::SetMuted { .. }
                            | Request::SetMicInputMuted { .. }
                            | Request::SetButtonLights { .. }
                            | Request::SetRouting { .. }
                            | Request::SetMicGain { .. }
                            | Request::SetEffect { .. }
                            | Request::SetMicParam { .. }
                    )
                })
                .collect()
        }

        fn volumes_received(&self) -> Vec<(Channel, u8)> {
            self.settings_received()
                .into_iter()
                .filter_map(|request| match request {
                    Request::SetVolume { channel, volume } => Some((channel, volume)),
                    _ => None,
                })
                .collect()
        }

        fn mutes_received(&self) -> Vec<(Channel, bool)> {
            self.settings_received()
                .into_iter()
                .filter_map(|request| match request {
                    Request::SetMuted { channel, muted } => Some((channel, muted)),
                    _ => None,
                })
                .collect()
        }
    }

    /// The virtual device passing for a real one, with a cable to pull.
    struct Cable {
        device: VirtualGoXlr,
        bench: Bench,
    }

    impl Link for Cable {
        fn exchange(&mut self, request: &[u8]) -> Result<Vec<u8>, DeviceError> {
            if self.bench.unplugged.load(Ordering::Relaxed) {
                return Err(DeviceError::Link("unplugged".into()));
            }
            let packet = Packet::decode(request).unwrap();
            self.bench
                .received
                .lock()
                .unwrap()
                .push(Request::decode(packet.command_id, &packet.body).unwrap());
            self.device.exchange(request)
        }
    }

    fn real_device() -> (Box<dyn Device>, Bench) {
        let (device, hands) = VirtualGoXlr::new();
        let bench = Bench {
            hands,
            unplugged: Arc::default(),
            received: Arc::default(),
        };
        let cable = Cable {
            device,
            bench: bench.clone(),
        };
        let session = Session::open(cable, DeviceKind::Hardware).unwrap();
        (Box::new(session), bench)
    }

    /// A port whose answers are written in advance.
    #[derive(Default)]
    struct Script {
        opens: VecDeque<Result<Box<dyn Device>, OpenError>>,
        rival: Arc<Mutex<Option<String>>>,
    }

    impl Port for Script {
        fn open(&mut self) -> Result<Box<dyn Device>, OpenError> {
            self.opens.pop_front().unwrap_or(Err(OpenError::Absent))
        }

        fn rival(&mut self) -> Option<String> {
            self.rival.lock().unwrap().clone()
        }
    }

    fn station(
        opens: impl IntoIterator<Item = Result<Box<dyn Device>, OpenError>>,
    ) -> (Station<Script>, Arc<Mutex<Option<String>>>) {
        let script = Script {
            opens: opens.into_iter().collect(),
            ..Script::default()
        };
        let rival = script.rival.clone();
        (Station::new(script, Settings::unknown()).unwrap(), rival)
    }

    #[test]
    fn the_virtual_device_shows_lively_volumes_the_settings_do_not_know() {
        let (mut station, _) = station([]);
        let snapshot = station.poll().unwrap();
        assert_eq!(
            snapshot.faders.map(|view| view.volume),
            [214, 178, 120, 196]
        );
        assert_eq!(station.settings().mixer.volumes, [None; Channel::COUNT]);
        assert_eq!(station.settings(), &Settings::unknown());
    }

    #[test]
    fn what_was_set_on_the_virtual_device_goes_to_the_real_one_when_it_comes() {
        let (device, bench) = real_device();
        let (mut station, _) = station([Err(OpenError::Absent), Ok(device)]);
        station.scan();
        for intent in [
            Intent::SetVolume {
                channel: Channel::Headphones,
                volume: 60,
            },
            Intent::AssignFader {
                fader: Fader::B,
                channel: Channel::Game,
            },
            Intent::SetDeEsser { amount: 20 },
            Intent::SetRoute {
                input: RoutingInput::Mic,
                output: RoutingOutput::Headphones,
                on: true,
            },
        ] {
            station.apply(intent).unwrap();
        }
        station.poll().unwrap();
        assert!(!station.awaits_return(), "no real device was ever lost");

        station.scan();
        assert_eq!(station.connection(), &Connection::Hardware);
        // The faders of the virtual device decide of no volume of the real
        // one: only the volume that was set is sent.
        assert_eq!(bench.volumes_received(), [(Channel::Headphones, 60)]);
        let device = bench.hands.state();
        assert_eq!(device.faders[1], Channel::Game);
        assert_eq!(device.effects[&EffectKey::DeEsser], 20);
        assert!(routed(&bench, RoutingInput::Mic).contains(RoutingOutput::Headphones));
        assert_eq!(station.poll().unwrap().channels[8].volume, Some(60));
    }

    #[test]
    fn other_settings_can_be_loaded_on_the_device_shown() {
        let (first, before) = real_device();
        let (second, after) = real_device();
        let (mut station, _) = station([Ok(first), Ok(second)]);
        let mut settings = Settings::unknown();
        settings.mixer.faders = [Channel::Music, Channel::Chat, Channel::Mic, Channel::System];
        settings.mixer.volumes[usize::from(Channel::Music.index())] = Some(99);
        settings.mic.de_esser = 15;

        // On the virtual device first.
        station.load(settings.clone()).unwrap();
        assert_eq!(station.settings(), &settings);
        let snapshot = station.poll().unwrap();
        assert_eq!(snapshot.faders[0].channel, Channel::Music);
        assert_eq!(snapshot.faders[0].volume, 99);
        assert_eq!(snapshot.mic.de_esser, 15);

        station.scan();
        assert_eq!(before.hands.state().faders[0], Channel::Music);
        assert_eq!(before.volumes_received(), [(Channel::Music, 99)]);

        // Then on the real one, which drops right after.
        settings.mic.de_esser = 30;
        station.load(settings.clone()).unwrap();
        assert_eq!(before.hands.state().effects[&EffectKey::DeEsser], 30);
        before.unplugged.store(true, Ordering::Relaxed);
        settings.mic.de_esser = 45;
        station.load(settings.clone()).unwrap();
        assert_eq!(station.connection(), &Connection::Demo);
        assert!(station.awaits_return());
        assert_eq!(station.poll().unwrap().mic.de_esser, 45);

        station.scan();
        assert_eq!(after.hands.state().effects[&EffectKey::DeEsser], 45);
    }

    #[test]
    fn starts_in_demo_mode_and_stays_there_without_a_device() {
        let (mut station, _) = station([]);
        assert_eq!(station.poll().unwrap().connection.state, "demo");
        station.scan();
        let snapshot = station.poll().unwrap();
        assert_eq!(snapshot.connection.state, "demo");
        assert_eq!(snapshot.device.kind, "virtual");
    }

    #[test]
    fn a_plugged_device_replaces_the_virtual_one() {
        let (device, bench) = real_device();
        let (mut station, _) = station([Ok(device)]);
        station.scan();
        assert_eq!(station.connection(), &Connection::Hardware);

        bench.hands.move_fader(Fader::B, 42);
        let snapshot = station.poll().unwrap();
        assert_eq!(snapshot.connection.state, "hardware");
        assert_eq!(snapshot.device.kind, "hardware");
        assert_eq!(snapshot.faders[1].volume, 42);
    }

    #[test]
    fn taking_a_real_device_over_sends_what_it_cannot_tell_and_no_volume() {
        let (device, bench) = real_device();
        let (mut station, _) = station([Ok(device)]);
        station.scan();
        station.poll().unwrap();

        let settings = bench.settings_received();
        // What silences comes first: until then the device plays as it
        // pleases.
        assert!(
            settings[..16]
                .iter()
                .all(|request| matches!(request, Request::SetRouting { .. }))
        );
        assert!(settings[16..28].iter().all(|request| matches!(
            request,
            Request::SetMuted { .. } | Request::SetMicInputMuted { .. }
        )));
        assert_eq!(
            settings[28..32],
            [
                (Fader::A, Channel::Mic),
                (Fader::B, Channel::Chat),
                (Fader::C, Channel::Music),
                (Fader::D, Channel::System),
            ]
            .map(|(fader, channel)| Request::SetFader { fader, channel })
        );
        let mut mutes = bench.mutes_received();
        mutes.sort_by_key(|(channel, _)| channel.index());
        assert_eq!(mutes, Channel::ALL.map(|channel| (channel, false)));
        assert_eq!(
            settings.last(),
            Some(&Request::SetButtonLights {
                lights: crate::tests::resting_lights()
            })
        );
        assert_eq!(bench.volumes_received(), []);
        assert!(settings.contains(&Request::SetMicInputMuted { muted: false }));
        let routes: Vec<Request> = settings
            .iter()
            .copied()
            .filter(|request| matches!(request, Request::SetRouting { .. }))
            .collect();
        assert_eq!(routes.len(), 16, "eight inputs, two sides each");
        assert!(routes.contains(&Request::SetRouting {
            input: RoutingInput::Mic,
            side: Side::Right,
            outputs: OutputSet::of(&[RoutingOutput::BroadcastMix, RoutingOutput::ChatMic]),
        }));
        // The microphone processing is sent, by both ways for the gate and
        // the compressor. Nobody said how the microphone is plugged in: its
        // type and gain are left as they are.
        let effects = settings
            .iter()
            .filter(|request| matches!(request, Request::SetEffect { .. }))
            .count();
        let params = settings
            .iter()
            .filter(|request| matches!(request, Request::SetMicParam { .. }))
            .count();
        assert_eq!((effects, params), (33, 9));
        assert!(
            !settings
                .iter()
                .any(|request| matches!(request, Request::SetMicGain { .. }))
        );
        assert_eq!(settings.len(), 4 + 11 + 1 + 16 + 1 + 33 + 9);
    }

    #[test]
    fn the_microphone_comes_back_with_a_device_plugged_again() {
        let (first, before) = real_device();
        let (second, after) = real_device();
        let (mut station, _) = station([Ok(first), Err(OpenError::Absent), Ok(second)]);
        station.scan();
        for intent in [
            Intent::SetMicType {
                mic_type: MicType::Condenser,
            },
            Intent::SetMicGain { gain: 44 },
            Intent::SetGate {
                setting: GateSetting::Threshold,
                value: -50,
            },
            Intent::SetCompressor {
                setting: CompressorSetting::MakeupGain,
                value: 6,
            },
            Intent::SetEqBand {
                band: EqBand::Khz8,
                frequency: 9000.0,
                gain: -5,
            },
            Intent::SetDeEsser { amount: 70 },
        ] {
            station.apply(intent).unwrap();
        }
        let shown = station.poll().unwrap().mic;
        assert_eq!(shown.gain, Some(44));

        // Pulled while a setting is on its way: no crash, the demo mode.
        before.unplugged.store(true, Ordering::Relaxed);
        station.apply(Intent::SetDeEsser { amount: 10 }).unwrap();
        assert_eq!(station.connection(), &Connection::Demo);
        // The virtual device shows what the real one was set to; the setting
        // it never received was not kept.
        let demo = station.poll().unwrap();
        assert_eq!(demo.device.kind, "virtual");
        assert_eq!(demo.mic, shown);

        station.scan();
        station.scan();
        assert_eq!(station.connection(), &Connection::Hardware);
        let device = after.hands.state();
        assert_eq!((device.mic_type, device.mic_gain), (MicType::Condenser, 44));
        assert_eq!(device.effects[&EffectKey::GateThreshold], -50);
        assert_eq!(device.mic_params[&MicParamKey::CompressorMakeupGain], 6.0);
        assert_eq!(device.effects[&EffectKey::EqFrequency(EqBand::Khz8)], 212);
        assert_eq!(device.effects[&EffectKey::EqGain(EqBand::Khz8)], -5);
        assert_eq!(device.effects[&EffectKey::DeEsser], 70);
        assert_eq!(station.poll().unwrap().mic, shown);
        assert_eq!(
            after.settings_received().last(),
            Some(&Request::SetButtonLights {
                lights: crate::tests::resting_lights()
            })
        );
    }

    #[test]
    fn the_microphone_comes_back_after_a_rival_program() {
        let (first, _) = real_device();
        let (second, after) = real_device();
        let (mut station, rival) = station([Ok(first), Ok(second)]);
        station.scan();
        station
            .apply(Intent::SetMicType {
                mic_type: MicType::Dynamic,
            })
            .unwrap();
        station.apply(Intent::SetMicGain { gain: 55 }).unwrap();
        station.apply(Intent::SetDeEsser { amount: 33 }).unwrap();

        *rival.lock().unwrap() = Some("GoXLR Utility".into());
        station.scan();
        *rival.lock().unwrap() = None;
        station.scan();
        assert_eq!(station.connection(), &Connection::Hardware);
        let device = after.hands.state();
        assert_eq!((device.mic_type, device.mic_gain), (MicType::Dynamic, 55));
        assert_eq!(device.effects[&EffectKey::DeEsser], 33);
    }

    fn routed(bench: &Bench, input: RoutingInput) -> OutputSet {
        let device = bench.hands.state();
        assert_eq!(
            device.routed(input, Side::Left),
            device.routed(input, Side::Right)
        );
        device.routed(input, Side::Left)
    }

    #[test]
    fn the_routing_goes_to_the_real_device_and_comes_back_with_it() {
        let (first, before) = real_device();
        let (second, after) = real_device();
        let (mut station, _) = station([Ok(first), Ok(second)]);
        station.scan();
        station.poll().unwrap();
        for (input, output, on) in [
            (RoutingInput::Mic, RoutingOutput::Headphones, true),
            (RoutingInput::System, RoutingOutput::LineOut, false),
        ] {
            station
                .apply(Intent::SetRoute { input, output, on })
                .unwrap();
        }
        let mic = OutputSet::of(&[
            RoutingOutput::Headphones,
            RoutingOutput::BroadcastMix,
            RoutingOutput::ChatMic,
        ]);
        let system = OutputSet::of(&[RoutingOutput::Headphones, RoutingOutput::BroadcastMix]);
        assert_eq!(routed(&before, RoutingInput::Mic), mic);
        assert_eq!(routed(&before, RoutingInput::System), system);

        // Unplugged in the middle of a change: no crash, the demo mode.
        before.unplugged.store(true, Ordering::Relaxed);
        station
            .apply(Intent::SetRoute {
                input: RoutingInput::Game,
                output: RoutingOutput::Headphones,
                on: false,
            })
            .unwrap();
        assert_eq!(station.connection(), &Connection::Demo);
        assert_eq!(station.poll().unwrap().device.kind, "virtual");

        station.scan();
        assert_eq!(station.connection(), &Connection::Hardware);
        assert_eq!(routed(&after, RoutingInput::Mic), mic);
        assert_eq!(routed(&after, RoutingInput::System), system);
        // The change the device never received was not kept.
        assert_eq!(
            routed(&after, RoutingInput::Game),
            crate::default_routing()[RoutingInput::Game as usize]
        );
        assert_eq!(
            station.poll().unwrap().routing[RoutingInput::Mic as usize].outputs,
            mic.iter().collect::<Vec<_>>()
        );
    }

    #[test]
    fn what_the_screen_asks_goes_to_the_real_device_when_it_is_shown() {
        let (device, bench) = real_device();
        let (mut station, _) = station([Ok(device)]);
        let intent = Intent::SetVolume {
            channel: Channel::Headphones,
            volume: 60,
        };

        // Before the device is taken, the virtual one answers.
        station.apply(intent).unwrap();
        assert_eq!(bench.volumes_received(), []);
        assert_eq!(station.poll().unwrap().channels[8].volume, Some(60));

        // The real one is brought to what was set, then answers itself.
        station.scan();
        assert_eq!(bench.volumes_received(), [(Channel::Headphones, 60)]);
        assert_eq!(station.poll().unwrap().channels[9].volume, None);
        station
            .apply(Intent::SetVolume {
                channel: Channel::MicMonitor,
                volume: 80,
            })
            .unwrap();
        assert_eq!(
            bench.volumes_received(),
            [(Channel::Headphones, 60), (Channel::MicMonitor, 80)]
        );
        assert_eq!(station.poll().unwrap().channels[9].volume, Some(80));
    }

    #[test]
    fn an_intent_for_a_device_that_just_dropped_brings_the_demo_mode_back() {
        let (device, bench) = real_device();
        let (mut station, _) = station([Ok(device)]);
        station.scan();
        station.poll().unwrap();

        bench.unplugged.store(true, Ordering::Relaxed);
        station
            .apply(Intent::SetMuted {
                channel: Channel::Mic,
                muted: true,
            })
            .unwrap();
        assert_eq!(station.connection(), &Connection::Demo);
        assert_eq!(station.poll().unwrap().device.kind, "virtual");
    }

    #[test]
    fn the_volumes_shown_are_read_from_the_real_faders() {
        let (device, bench) = real_device();
        for (fader, position) in Fader::ALL.into_iter().zip([10, 20, 30, 40]) {
            bench.hands.move_fader(fader, position);
        }
        let (mut station, _) = station([Ok(device)]);
        station.scan();
        let snapshot = station.poll().unwrap();
        assert_eq!(snapshot.faders.map(|view| view.volume), [10, 20, 30, 40]);
    }

    #[test]
    fn an_unplugged_device_brings_the_demo_mode_back() {
        let (device, bench) = real_device();
        let (mut station, _) = station([Ok(device)]);
        station.scan();
        station.poll().unwrap();

        bench.unplugged.store(true, Ordering::Relaxed);
        let snapshot = station.poll().unwrap();
        assert_eq!(snapshot.connection.state, "demo");
        assert_eq!(snapshot.device.kind, "virtual");
        assert_eq!(station.connection(), &Connection::Demo);

        // Still unplugged: nothing to open.
        station.scan();
        assert_eq!(station.poll().unwrap().connection.state, "demo");
    }

    #[test]
    fn a_device_that_dropped_is_awaited_until_it_is_back() {
        let (first, before) = real_device();
        let (second, _) = real_device();
        let (mut station, rival) = station([Ok(first), Err(OpenError::Absent), Ok(second)]);
        assert!(!station.awaits_return(), "nothing was ever plugged in");
        station.scan();
        station.poll().unwrap();
        assert!(!station.awaits_return());

        before.unplugged.store(true, Ordering::Relaxed);
        station.poll().unwrap();
        assert!(station.awaits_return());
        station.scan();
        assert!(station.awaits_return(), "still unplugged");
        station.scan();
        assert!(!station.awaits_return(), "it is back");

        // A rival is not hurried: it leaves when the user closes it.
        *rival.lock().unwrap() = Some("GoXLR Utility".into());
        station.scan();
        assert!(!station.awaits_return());
    }

    #[test]
    fn a_device_plugged_again_gets_back_all_the_app_knew() {
        let (first, before) = real_device();
        let (second, after) = real_device();
        let (mut station, _) = station([Ok(first), Err(OpenError::Absent), Ok(second)]);
        station.scan();
        before.hands.move_fader(Fader::A, 111);
        before.hands.move_fader(Fader::D, 222);
        station.poll().unwrap();
        for intent in [
            Intent::SetMuted {
                channel: Channel::Chat,
                muted: true,
            },
            Intent::SetVolume {
                channel: Channel::Headphones,
                volume: 60,
            },
            Intent::AssignFader {
                fader: Fader::C,
                channel: Channel::Game,
            },
        ] {
            station.apply(intent).unwrap();
        }
        station.poll().unwrap();

        before.unplugged.store(true, Ordering::Relaxed);
        station.poll().unwrap();
        station.scan();
        station.scan();
        assert_eq!(station.connection(), &Connection::Hardware);

        let device = after.hands.state();
        assert_eq!(
            device.faders,
            [Channel::Mic, Channel::Chat, Channel::Game, Channel::System]
        );
        assert!(device.muted[usize::from(Channel::Chat.index())]);
        assert_eq!(device.lights.get(Button::Fader2Mute), ButtonLight::Lit);
        let volumes = after.volumes_received();
        for known in [
            (Channel::Mic, 111),
            (Channel::System, 222),
            (Channel::Headphones, 60),
        ] {
            assert!(volumes.contains(&known), "{known:?} in {volumes:?}");
        }
        // Line In was never set nor shown by a fader: left alone.
        assert!(
            !volumes
                .iter()
                .any(|(channel, _)| *channel == Channel::LineIn)
        );
    }

    #[test]
    fn a_rival_program_is_named_and_the_virtual_device_is_shown() {
        let (mut station, _) = station([Err(OpenError::Busy {
            program: "GoXLR Utility".into(),
        })]);
        station.scan();
        let snapshot = station.poll().unwrap();
        assert_eq!(snapshot.device.kind, "virtual");
        assert_eq!(
            serde_json::to_value(&snapshot.connection).unwrap(),
            json!({ "state": "busy", "program": "GoXLR Utility" })
        );
    }

    #[test]
    fn a_rival_started_later_gets_the_device() {
        let (first, _) = real_device();
        let (second, after) = real_device();
        let (mut station, rival) = station([Ok(first), Ok(second)]);
        station.scan();
        station.poll().unwrap();
        station
            .apply(Intent::SetMuted {
                channel: Channel::Music,
                muted: true,
            })
            .unwrap();
        station
            .apply(Intent::SetRoute {
                input: RoutingInput::Music,
                output: RoutingOutput::Headphones,
                on: false,
            })
            .unwrap();

        *rival.lock().unwrap() = Some("GoXLR Utility".into());
        station.scan();
        assert_eq!(
            station.connection(),
            &Connection::Busy {
                program: "GoXLR Utility".into()
            }
        );
        assert_eq!(station.poll().unwrap().device.kind, "virtual");

        // Once the rival is gone the device comes back with the mutes and the
        // routing of the app, and the volumes the rival left.
        *rival.lock().unwrap() = None;
        station.scan();
        assert_eq!(station.connection(), &Connection::Hardware);
        assert!(after.mutes_received().contains(&(Channel::Music, true)));
        assert_eq!(after.volumes_received(), []);
        assert_eq!(
            routed(&after, RoutingInput::Music),
            OutputSet::of(&[RoutingOutput::BroadcastMix, RoutingOutput::LineOut])
        );
    }

    #[test]
    fn a_mini_and_a_device_that_cannot_be_opened_are_told_apart() {
        let (mut station, _) = station([
            Err(OpenError::Unsupported),
            Err(OpenError::Failed("access denied".into())),
        ]);
        station.scan();
        let snapshot = station.poll().unwrap();
        assert_eq!(
            serde_json::to_value(&snapshot.connection).unwrap(),
            json!({ "state": "unsupported" })
        );

        station.scan();
        assert_eq!(station.poll().unwrap().connection.state, "unreachable");
        assert_eq!(
            station.connection(),
            &Connection::Unreachable {
                reason: "access denied".into()
            }
        );
    }

    #[test]
    fn a_device_that_dies_while_being_taken_over_is_unreachable() {
        let (device, bench) = real_device();
        bench.unplugged.store(true, Ordering::Relaxed);
        let (mut station, _) = station([Ok(device)]);
        station.scan();
        assert!(matches!(
            station.connection(),
            Connection::Unreachable { .. }
        ));
        assert_eq!(station.poll().unwrap().device.kind, "virtual");
    }

    fn hold_to_mute(button: Button, target: AudioTarget) -> Intent {
        Intent::SetGesture {
            button,
            gesture: Gesture::Hold,
            action: Some(Action::Mute {
                target,
                mode: MuteMode::Mute,
            }),
        }
    }

    fn press(button: Button, down: bool) -> Intent {
        Intent::PressButton { button, down }
    }

    #[test]
    fn a_button_pressed_from_the_screen_is_pressed_on_the_virtual_device() {
        let (mut station, _) = station([]);
        station.poll().unwrap();

        station.apply(press(Button::MicMute, true)).unwrap();
        let snapshot = station.poll().unwrap();
        assert_eq!(snapshot.pressed, [Button::MicMute]);
        assert!(snapshot.mic_off);
        station.apply(press(Button::MicMute, false)).unwrap();
        assert!(station.poll().unwrap().pressed.is_empty());

        // A click shorter than a reading is a press all the same.
        station.apply(press(Button::MicMute, true)).unwrap();
        station.apply(press(Button::MicMute, false)).unwrap();
        let snapshot = station.poll().unwrap();
        assert!(!snapshot.mic_off, "toggled back");
        assert_eq!(snapshot.last_press.unwrap().button, Button::MicMute);
        // What the press did is what the next real device is brought to.
        assert!(!station.settings().mixer.mic_off);
    }

    #[test]
    fn the_real_device_is_never_pressed_from_the_screen() {
        let (device, bench) = real_device();
        let (mut station, _) = station([Ok(device)]);
        station.scan();
        station.poll().unwrap();
        let before = bench.received.lock().unwrap().len();

        station.apply(press(Button::MicMute, true)).unwrap();
        let snapshot = station.poll().unwrap();
        assert!(snapshot.pressed.is_empty() && !snapshot.mic_off);
        assert_eq!(bench.hands.state().pressed.bits(), 0);
        // Only readings went to the device.
        let received = bench.received.lock().unwrap();
        assert!(
            received[before..]
                .iter()
                .all(|request| matches!(request, Request::GetStatus | Request::GetMicLevel)),
            "{:?}",
            &received[before..]
        );
    }

    #[test]
    fn a_cable_pulled_during_a_hold_leaves_nothing_silenced() {
        let (first, before) = real_device();
        let (second, after) = real_device();
        let (mut station, _) = station([Ok(first), Ok(second)]);
        station.scan();
        station.poll().unwrap();
        station
            .apply(hold_to_mute(Button::MicMute, AudioTarget::Mic))
            .unwrap();
        before.hands.press(Button::MicMute);
        assert!(station.poll().unwrap().mic_off);

        before.unplugged.store(true, Ordering::Relaxed);
        let snapshot = station.poll().unwrap();
        assert_eq!(snapshot.device.kind, "virtual");
        assert!(!snapshot.mic_off, "the finger that held it is gone");
        assert!(!station.settings().mixer.mic_off);

        // The device that comes back is not told to keep the microphone off,
        // and is not pressed.
        station.scan();
        assert_eq!(station.connection(), &Connection::Hardware);
        assert!(!after.hands.state().mic_input_muted);
        assert!(!station.poll().unwrap().mic_off);
    }

    #[test]
    fn settings_loaded_on_a_device_that_dropped_do_not_keep_what_a_hold_silenced() {
        let (device, bench) = real_device();
        let (mut station, _) = station([Ok(device)]);
        station.scan();
        station.poll().unwrap();
        station
            .apply(hold_to_mute(Button::MicMute, AudioTarget::Mic))
            .unwrap();
        bench.hands.press(Button::MicMute);
        assert!(station.poll().unwrap().mic_off);

        // What a profile change loads is made of the settings in use.
        let settings = station.settings().clone();
        assert!(settings.mixer.mic_off);
        bench.unplugged.store(true, Ordering::Relaxed);
        station.load(settings).unwrap();
        assert_eq!(station.connection(), &Connection::Demo);
        assert!(!station.settings().mixer.mic_off);
        assert!(!station.poll().unwrap().mic_off);
    }

    #[test]
    fn a_click_that_ends_after_the_real_device_came_lets_go_of_the_virtual_button() {
        let (device, bench) = real_device();
        let (mut station, _) = station([Err(OpenError::Absent), Ok(device)]);
        station.scan();
        station.poll().unwrap();
        // A pad that does something for a short press and for a long one.
        for (gesture, mode) in [
            (Gesture::Short, MuteMode::Toggle),
            (Gesture::Long, MuteMode::Mute),
        ] {
            station
                .apply(Intent::SetGesture {
                    button: Button::SamplerTopLeft,
                    gesture,
                    action: Some(Action::Mute {
                        target: AudioTarget::Channel {
                            channel: Channel::Music,
                        },
                        mode,
                    }),
                })
                .unwrap();
        }
        station.apply(press(Button::SamplerTopLeft, true)).unwrap();
        station.poll().unwrap();

        // The real device comes, and the click ends on a screen that now shows it.
        station.scan();
        assert_eq!(station.connection(), &Connection::Hardware);
        station.apply(press(Button::SamplerTopLeft, false)).unwrap();
        assert!(station.poll().unwrap().pressed.is_empty());

        // The cable is pulled: the virtual device comes back with nothing held
        // and no press left over.
        bench.unplugged.store(true, Ordering::Relaxed);
        let snapshot = station.poll().unwrap();
        assert_eq!(snapshot.device.kind, "virtual");
        assert!(snapshot.pressed.is_empty());
        let snapshot = station.poll().unwrap();
        assert!(!snapshot.channels[usize::from(Channel::Music.index())].muted);
        assert!(!station.settings().mixer.muted[usize::from(Channel::Music.index())]);
    }

    #[test]
    fn a_button_held_on_the_virtual_device_leaves_nothing_silenced_when_a_real_one_comes() {
        let (device, bench) = real_device();
        let (mut station, _) = station([Err(OpenError::Absent), Ok(device)]);
        station.scan();
        station.poll().unwrap();
        station
            .apply(hold_to_mute(
                Button::SamplerTopLeft,
                AudioTarget::Channel {
                    channel: Channel::Music,
                },
            ))
            .unwrap();
        station.apply(press(Button::SamplerTopLeft, true)).unwrap();
        let snapshot = station.poll().unwrap();
        assert!(snapshot.channels[usize::from(Channel::Music.index())].muted);

        station.scan();
        assert_eq!(station.connection(), &Connection::Hardware);
        assert!(!bench.hands.state().muted[usize::from(Channel::Music.index())]);
        assert!(!station.settings().mixer.muted[usize::from(Channel::Music.index())]);
        // The gesture came with it.
        assert!(
            station
                .settings()
                .controls
                .action(Button::SamplerTopLeft, Gesture::Hold)
                .is_some()
        );
    }

    #[test]
    fn a_cable_pulled_during_a_hold_on_a_route_leaves_nothing_cut() {
        let (first, before) = real_device();
        let (second, after) = real_device();
        let (mut station, _) = station([Ok(first), Ok(second)]);
        station.scan();
        station.poll().unwrap();
        station
            .apply(Intent::SetGesture {
                button: Button::SamplerTopLeft,
                gesture: Gesture::Hold,
                action: Some(Action::Route {
                    input: RoutingInput::Music,
                    output: RoutingOutput::BroadcastMix,
                    mode: crate::RouteMode::Off,
                }),
            })
            .unwrap();
        before.hands.press(Button::SamplerTopLeft);
        station.poll().unwrap();
        assert!(!routed(&before, RoutingInput::Music).contains(RoutingOutput::BroadcastMix));

        before.unplugged.store(true, Ordering::Relaxed);
        assert_eq!(station.poll().unwrap().device.kind, "virtual");
        assert_eq!(station.settings().mixer.routing, crate::default_routing());

        // The device that comes back is sent the routing as it was, and is
        // not pressed.
        station.scan();
        assert_eq!(station.connection(), &Connection::Hardware);
        assert!(routed(&after, RoutingInput::Music).contains(RoutingOutput::BroadcastMix));
        station.poll().unwrap();
        assert!(routed(&after, RoutingInput::Music).contains(RoutingOutput::BroadcastMix));
    }

    #[test]
    fn a_dial_is_turned_from_the_screen_on_the_virtual_device_only() {
        let channel = usize::from(Channel::Headphones.index());
        let (device, bench) = real_device();
        let (mut virtual_station, _) = station([Err(OpenError::Absent), Ok(device)]);
        virtual_station.scan();
        virtual_station.poll().unwrap();
        virtual_station
            .apply(Intent::SetWheel {
                wheel: goxlr_hub_protocol::Wheel::Pitch,
                action: Some(crate::WheelAction::Volume {
                    target: AudioTarget::Channel {
                        channel: Channel::Headphones,
                    },
                    step: 4,
                }),
            })
            .unwrap();
        let turn = |notches| Intent::TurnWheel {
            wheel: goxlr_hub_protocol::Wheel::Pitch,
            notches,
        };

        // The virtual device shows a lively volume the app does not know: a
        // dial does not start from it, nor keep what it would give.
        virtual_station.apply(turn(-3)).unwrap();
        let snapshot = virtual_station.poll().unwrap();
        assert_eq!(snapshot.channels[channel].volume, Some(255));
        assert_eq!(virtual_station.settings().mixer.volumes[channel], None);

        // Set once, it is known: the dial follows, and what it sets is kept.
        virtual_station
            .apply(Intent::SetVolume {
                channel: Channel::Headphones,
                volume: 100,
            })
            .unwrap();
        virtual_station.apply(turn(-3)).unwrap();
        let snapshot = virtual_station.poll().unwrap();
        assert_eq!(snapshot.channels[channel].volume, Some(69));
        assert_eq!(virtual_station.settings().mixer.volumes[channel], Some(69));

        // That is what the real device is brought to, and nothing else.
        virtual_station.scan();
        assert_eq!(bench.volumes_received(), [(Channel::Headphones, 69)]);
    }

    #[test]
    fn a_real_device_is_never_turned_from_the_screen() {
        let (device, bench) = real_device();
        let (mut station, _) = station([Ok(device)]);
        station.scan();
        station.poll().unwrap();
        let before = bench.received.lock().unwrap().len();
        station
            .apply(Intent::TurnWheel {
                wheel: goxlr_hub_protocol::Wheel::Pitch,
                notches: 5,
            })
            .unwrap();
        station.poll().unwrap();
        assert_eq!(bench.hands.state().encoders, [0; 4]);
        let received = bench.received.lock().unwrap();
        assert!(
            received[before..]
                .iter()
                .all(|request| matches!(request, Request::GetStatus | Request::GetMicLevel)),
            "{:?}",
            &received[before..]
        );
    }

    #[test]
    fn settings_loaded_on_a_device_that_dropped_do_not_keep_a_cell_a_hold_cut() {
        let (device, bench) = real_device();
        let (mut station, _) = station([Ok(device)]);
        station.scan();
        station.poll().unwrap();
        station
            .apply(Intent::SetGesture {
                button: Button::SamplerTopLeft,
                gesture: Gesture::Hold,
                action: Some(Action::Route {
                    input: RoutingInput::Music,
                    output: RoutingOutput::BroadcastMix,
                    mode: crate::RouteMode::Off,
                }),
            })
            .unwrap();
        bench.hands.press(Button::SamplerTopLeft);
        station.poll().unwrap();
        assert!(
            !station.settings().mixer.routing[RoutingInput::Music as usize]
                .contains(RoutingOutput::BroadcastMix)
        );
        // At rest, the cell is as it was before the finger came.
        assert_eq!(
            station.settings_at_rest().mixer.routing,
            crate::default_routing()
        );

        // What a change of piece loads is made of the settings in use.
        let settings = station.settings().clone();
        bench.unplugged.store(true, Ordering::Relaxed);
        station.load(settings).unwrap();
        assert_eq!(station.connection(), &Connection::Demo);
        assert_eq!(station.settings().mixer.routing, crate::default_routing());
    }
}
