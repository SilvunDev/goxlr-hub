//! What the app knows about the device, and the picture of it the interface
//! draws. The interface never reaches the device: it only receives a
//! [`Snapshot`].

mod station;

use goxlr_hub_device::{Device, DeviceError, DeviceKind};
use goxlr_hub_protocol::{Button, Channel, Fader, mic_level_db};
use serde::Serialize;

pub use station::{Connection, ConnectionView, Port, Station};

/// Faders, volumes and mutes. The device cannot be asked for them, so the
/// app keeps them and sends them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MixerState {
    /// The channel under each fader.
    pub faders: [Channel; Fader::COUNT],
    pub volumes: [u8; Channel::COUNT],
    pub muted: [bool; Channel::COUNT],
}

impl Default for MixerState {
    fn default() -> Self {
        let mut volumes = [255; Channel::COUNT];
        volumes[usize::from(Channel::Mic.index())] = 214;
        volumes[usize::from(Channel::Chat.index())] = 178;
        volumes[usize::from(Channel::Music.index())] = 120;
        volumes[usize::from(Channel::System.index())] = 196;
        Self {
            faders: [Channel::Mic, Channel::Chat, Channel::Music, Channel::System],
            volumes,
            muted: [false; Channel::COUNT],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    /// Which device is shown, and why.
    pub connection: ConnectionView,
    pub device: DeviceView,
    pub faders: [FaderView; Fader::COUNT],
    pub channels: Vec<ChannelView>,
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
    /// 0 to 255.
    pub volume: u8,
    pub muted: bool,
}

/// The app's side of one connected device.
pub struct Hub {
    device: Box<dyn Device>,
    mixer: MixerState,
}

impl Hub {
    /// Takes a device over and brings it to the app's state.
    pub fn connect(device: Box<dyn Device>) -> Result<Self, DeviceError> {
        let mut hub = Self {
            device,
            mixer: MixerState::default(),
        };
        hub.apply()?;
        Ok(hub)
    }

    /// Takes a real device over without changing how it sounds.
    ///
    /// Only the fader assignment is sent, so that the names on screen are
    /// true; the volumes are then read from the faders. A device that
    /// dropped and came back also gets the fader volumes it had.
    pub fn adopt(
        device: Box<dyn Device>,
        remembered: Option<&MixerState>,
    ) -> Result<Self, DeviceError> {
        let mut hub = Self {
            device,
            mixer: remembered.cloned().unwrap_or_default(),
        };
        for (fader, channel) in Fader::ALL.into_iter().zip(hub.mixer.faders) {
            hub.device.set_fader(fader, channel)?;
        }
        if remembered.is_some() {
            for channel in hub.mixer.faders {
                let volume = hub.mixer.volumes[usize::from(channel.index())];
                hub.device.set_volume(channel, volume)?;
            }
        }
        Ok(hub)
    }

    fn apply(&mut self) -> Result<(), DeviceError> {
        for (fader, channel) in Fader::ALL.into_iter().zip(self.mixer.faders) {
            self.device.set_fader(fader, channel)?;
        }
        for channel in Channel::ALL {
            let index = usize::from(channel.index());
            self.device.set_volume(channel, self.mixer.volumes[index])?;
            self.device.set_muted(channel, self.mixer.muted[index])?;
        }
        Ok(())
    }

    pub fn mixer(&self) -> &MixerState {
        &self.mixer
    }

    /// Reads what changed on the device and returns the picture to draw.
    pub fn poll(&mut self) -> Result<Snapshot, DeviceError> {
        let status = self.device.status()?;
        let mic_level = self.device.mic_level()?;

        // A fader moved by hand sets the volume of the channel it carries.
        for (channel, position) in self.mixer.faders.into_iter().zip(status.faders) {
            self.mixer.volumes[usize::from(channel.index())] = position;
        }

        let volume = |channel: Channel| self.mixer.volumes[usize::from(channel.index())];
        let muted = |channel: Channel| self.mixer.muted[usize::from(channel.index())];
        let info = self.device.info();

        let mut faders = Fader::ALL.map(|fader| FaderView {
            fader,
            channel: Channel::Mic,
            volume: 0,
            muted: false,
        });
        for (view, channel) in faders.iter_mut().zip(self.mixer.faders) {
            view.channel = channel;
            view.volume = volume(channel);
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
        assert_eq!(device.volumes, hub.mixer().volumes);
        assert_eq!(device.muted, hub.mixer().muted);
        assert_eq!(device.volumes[usize::from(Channel::Music.index())], 120);
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
        hands.move_fader(Fader::C, 31);
        hands.press(Button::Fader3Mute);

        let snapshot = hub.poll().unwrap();
        assert_eq!(snapshot.faders[2].volume, 31);
        assert_eq!(snapshot.faders[0].volume, 214);
        let music = snapshot.channels[usize::from(Channel::Music.index())];
        assert_eq!((music.channel, music.volume), (Channel::Music, 31));
        assert_eq!(snapshot.pressed, [Button::Fader3Mute]);
        assert_eq!(hub.mixer().volumes[usize::from(Channel::Music.index())], 31);
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
            json!({ "channel": "micMonitor", "volume": 255, "muted": false })
        );
        assert_eq!(value["pressed"], json!(["micMute"]));
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
