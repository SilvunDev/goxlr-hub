//! The command language of the full-size GoXLR.
//!
//! A command travels as a [`Packet`]: a 16-byte header followed by a body.
//! [`Request`] builds and reads the commands the app sends; the `response`
//! types build and read what the device answers. Nothing here touches
//! hardware.

mod error;
mod mic;
mod packet;
mod request;
mod response;
mod types;

pub use error::ProtocolError;
pub use mic::{
    COMPRESSOR_ATTACK_MS, COMPRESSOR_RATIOS, COMPRESSOR_RELEASE_MS, EffectKey, EqBand,
    GATE_TIMES_MS, MicParamKey, eq_frequency_value, gate_attenuation_db,
};
pub use packet::{HEADER_LEN, Packet};
pub use request::Request;
pub use response::{
    FirmwareInfo, MIC_LEVEL_FLOOR_DB, SerialInfo, Status, Version, decode_mic_level,
    encode_mic_level, mic_level_db,
};
pub use types::{
    Button, ButtonLight, ButtonLights, ButtonSet, Channel, Fader, MicType, OutputSet, RoutingInput,
    RoutingOutput, Side,
};
