//! The single way the app reaches a GoXLR.
//!
//! [`Device`] is what the rest of the app sees. [`Session`] implements it by
//! speaking the protocol over a [`Link`]. The virtual device is a link that
//! answers like the firmware would; the USB link to a real GoXLR plugs in the
//! same way.

mod hardware;
mod session;
mod virtual_device;

use goxlr_hub_protocol::{
    ButtonLights, Channel, EffectKey, Fader, MicParamKey, MicType, OutputSet, ProtocolError,
    RoutingInput, Status, Wheel,
};
use thiserror::Error;

pub use hardware::{HardwareLink, OpenError, open_hardware, rival};
pub use session::{Link, Session};
pub use virtual_device::{PastTheEnd, VirtualGoXlr, VirtualHandle, VirtualState, open_virtual};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceKind {
    /// The built-in simulated device of the demo mode.
    Virtual,
    /// A GoXLR plugged into the computer.
    Hardware,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceInfo {
    pub kind: DeviceKind,
    pub firmware: String,
    pub serial: String,
    pub manufactured: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DeviceError {
    #[error("the link to the device failed: {0}")]
    Link(String),
    #[error(transparent)]
    Protocol(#[from] ProtocolError),
    #[error("the device answered command {received} instead of command {expected}")]
    OutOfSync { expected: u16, received: u16 },
}

/// A GoXLR, real or virtual.
///
/// The device cannot be asked for most of its settings: the app is expected
/// to remember what it sent.
pub trait Device: Send {
    fn info(&self) -> &DeviceInfo;

    /// Buttons held down, fader and dial positions.
    fn status(&mut self) -> Result<Status, DeviceError>;

    /// Raw microphone level; see `goxlr_hub_protocol::mic_level_db`.
    fn mic_level(&mut self) -> Result<u16, DeviceError>;

    fn set_fader(&mut self, fader: Fader, channel: Channel) -> Result<(), DeviceError>;

    fn set_volume(&mut self, channel: Channel, volume: u8) -> Result<(), DeviceError>;

    fn set_muted(&mut self, channel: Channel, muted: bool) -> Result<(), DeviceError>;

    /// Puts a dial at a position. The device keeps it within its travel, and
    /// `status` tells where it kept it.
    fn set_encoder(&mut self, wheel: Wheel, value: i8) -> Result<(), DeviceError>;

    /// Silences the microphone itself, whatever its channel is set to.
    fn set_mic_input_muted(&mut self, muted: bool) -> Result<(), DeviceError>;

    /// Lights every button at once.
    fn set_button_lights(&mut self, lights: ButtonLights) -> Result<(), DeviceError>;

    fn set_routing(&mut self, input: RoutingInput, outputs: OutputSet) -> Result<(), DeviceError>;

    fn set_mic_gain(&mut self, mic_type: MicType, gain: u16) -> Result<(), DeviceError>;

    /// Sets one setting of the microphone processing.
    fn set_effect(&mut self, key: EffectKey, value: i32) -> Result<(), DeviceError>;

    /// Sets one setting of the gate or of the compressor. The full-size
    /// GoXLR wants them as an effect too.
    fn set_mic_param(&mut self, key: MicParamKey, value: f32) -> Result<(), DeviceError>;
}
