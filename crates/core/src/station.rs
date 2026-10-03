//! Chooses the device the app shows: the real GoXLR when it can be had, the
//! virtual one otherwise, and says which and why.

use goxlr_hub_device::{Device, DeviceError, OpenError, open_virtual};
use serde::Serialize;

use crate::{Hub, MixerState, Snapshot};

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
    /// The mixer of a real device that dropped, to bring it back as it was.
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
                self.hardware = None;
                self.remembered = None;
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
                // The other program changed the device as it pleased.
                self.remembered = None;
                Connection::Busy { program }
            }
            Err(OpenError::Unsupported) => Connection::Unsupported,
            Err(OpenError::Failed(reason)) => Connection::Unreachable { reason },
        };
    }

    /// The picture to draw: the real device, or the virtual one when the
    /// real one is not there or stops answering.
    pub fn poll(&mut self) -> Result<Snapshot, DeviceError> {
        if let Some(hub) = &mut self.hardware {
            match hub.poll() {
                Ok(snapshot) => return Ok(self.stamp(snapshot)),
                Err(_) => {
                    self.remembered = Some(hub.mixer().clone());
                    self.hardware = None;
                    self.connection = Connection::Demo;
                }
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
    use goxlr_hub_protocol::{Channel, Fader, Packet, Request};
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
                            | Request::SetRouting { .. }
                            | Request::SetMicGain { .. }
                    )
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
    fn taking_a_real_device_over_only_assigns_its_faders() {
        let (device, bench) = real_device();
        let (mut station, _) = station([Ok(device)]);
        station.scan();
        station.poll().unwrap();
        assert_eq!(
            bench.settings_received(),
            [
                (Fader::A, Channel::Mic),
                (Fader::B, Channel::Chat),
                (Fader::C, Channel::Music),
                (Fader::D, Channel::System),
            ]
            .map(|(fader, channel)| Request::SetFader { fader, channel })
        );
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
    fn a_device_plugged_again_gets_its_fader_volumes_back_and_nothing_else() {
        let (first, before) = real_device();
        let (second, after) = real_device();
        let (mut station, _) = station([Ok(first), Err(OpenError::Absent), Ok(second)]);
        station.scan();
        before.hands.move_fader(Fader::A, 111);
        before.hands.move_fader(Fader::D, 222);
        station.poll().unwrap();

        before.unplugged.store(true, Ordering::Relaxed);
        station.poll().unwrap();
        station.scan();
        station.scan();
        assert_eq!(station.connection(), &Connection::Hardware);

        let settings = after.settings_received();
        assert_eq!(settings.len(), 8, "four assignments, four volumes");
        assert!(settings.contains(&Request::SetVolume {
            channel: Channel::Mic,
            volume: 111
        }));
        assert!(settings.contains(&Request::SetVolume {
            channel: Channel::System,
            volume: 222
        }));
        assert!(
            !settings
                .iter()
                .any(|request| matches!(request, Request::SetMuted { .. }))
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

        *rival.lock().unwrap() = Some("GoXLR Utility".into());
        station.scan();
        assert_eq!(
            station.connection(),
            &Connection::Busy {
                program: "GoXLR Utility".into()
            }
        );
        assert_eq!(station.poll().unwrap().device.kind, "virtual");

        // Once the rival is gone the device comes back, as the rival left it.
        *rival.lock().unwrap() = None;
        station.scan();
        assert_eq!(station.connection(), &Connection::Hardware);
        assert_eq!(after.settings_received().len(), 4, "assignments only");
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
