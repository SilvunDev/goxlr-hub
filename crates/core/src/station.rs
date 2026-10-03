//! Chooses the device the app shows: the real GoXLR when it can be had, the
//! virtual one otherwise, and says which and why.

use goxlr_hub_device::{Device, DeviceError, OpenError, open_virtual};
use serde::Serialize;

use crate::{Hub, Intent, MixerState, Snapshot};

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
    hardware: Option<Hub>,
    connection: Connection,
    /// The mixer of a real device the app lost, to bring it back as it was.
    remembered: Option<MixerState>,
}

impl<P: Port> Station<P> {
    /// Starts on the virtual device; `scan` looks for the real one.
    pub fn new(port: P) -> Result<Self, DeviceError> {
        let (device, _) = open_virtual()?;
        Ok(Self {
            port,
            demo: Hub::connect(Box::new(device))?,
            hardware: None,
            connection: Connection::Demo,
            remembered: None,
        })
    }

    pub fn connection(&self) -> &Connection {
        &self.connection
    }

    /// Looks for the real device, or checks that it is still ours. Meant to
    /// be called about once a second.
    pub fn scan(&mut self) {
        if self.hardware.is_some() {
            // Two programs driving the device steal each other's answers:
            // the newcomer gets it.
            if let Some(program) = self.port.rival() {
                if let Some(hub) = self.hardware.take() {
                    self.remembered = Some(hub.mixer().clone());
                }
                self.forget_volumes();
                self.connection = Connection::Busy { program };
            }
            return;
        }

        self.connection = match self.port.open() {
            Ok(device) => match Hub::adopt(device, self.remembered.as_ref()) {
                Ok(hub) => {
                    self.hardware = Some(hub);
                    self.remembered = None;
                    Connection::Hardware
                }
                Err(error) => Connection::Unreachable {
                    reason: error.to_string(),
                },
            },
            Err(OpenError::Absent) => Connection::Demo,
            Err(OpenError::Busy { program }) => {
                self.forget_volumes();
                Connection::Busy { program }
            }
            Err(OpenError::Unsupported) => Connection::Unsupported,
            Err(OpenError::Failed(reason)) => Connection::Unreachable { reason },
        };
    }

    /// The other program set the volumes as it pleased. The mutes and the
    /// fader assignment are put back by the app, the volumes are not.
    fn forget_volumes(&mut self) {
        if let Some(mixer) = &mut self.remembered {
            mixer.forget_volumes();
        }
    }

    /// Does what the interface asked, on the device shown.
    pub fn apply(&mut self, intent: Intent) -> Result<(), DeviceError> {
        if let Some(hub) = &mut self.hardware {
            if hub.apply(intent).is_err() {
                self.drop_hardware();
            }
            return Ok(());
        }
        self.demo.apply(intent)
    }

    /// The real device stopped answering: back to the virtual one, keeping
    /// what the real one was set to.
    fn drop_hardware(&mut self) {
        if let Some(hub) = self.hardware.take() {
            self.remembered = Some(hub.mixer().clone());
        }
        self.connection = Connection::Demo;
    }

    /// The picture to draw: the real device, or the virtual one when the
    /// real one is not there or stops answering.
    pub fn poll(&mut self) -> Result<Snapshot, DeviceError> {
        if let Some(hub) = &mut self.hardware {
            match hub.poll() {
                Ok(snapshot) => return Ok(self.stamp(snapshot)),
                Err(_) => self.drop_hardware(),
            }
        }
        let snapshot = self.demo.poll()?;
        Ok(self.stamp(snapshot))
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
        Button, ButtonLight, ButtonLights, Channel, Fader, OutputSet, Packet, Request,
        RoutingInput, RoutingOutput, Side,
    };
    use serde_json::json;

    use super::*;

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
        (Station::new(script).unwrap(), rival)
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
        assert_eq!(
            settings[..4],
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
                lights: ButtonLights::default()
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
        assert_eq!(settings.len(), 4 + 11 + 1 + 16 + 1);
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

        station.scan();
        assert_eq!(station.poll().unwrap().channels[8].volume, None);
        station.apply(intent).unwrap();
        assert_eq!(bench.volumes_received(), [(Channel::Headphones, 60)]);
        assert_eq!(station.poll().unwrap().channels[8].volume, Some(60));
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
}
