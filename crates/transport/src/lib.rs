//! Carries commands to a GoXLR and brings the answers back.
//!
//! A command is written with one control request and its answer is read with
//! another, a few milliseconds later. This crate does not look inside the
//! bytes it carries.

mod processes;
mod retry;
mod rival;

#[cfg(windows)]
#[path = "tusb.rs"]
mod platform;

#[cfg(not(windows))]
#[path = "libusb.rs"]
mod platform;

use std::time::Duration;

use thiserror::Error;

pub use platform::UsbLink;

#[cfg_attr(windows, allow(dead_code))]
const VENDOR_ID: u16 = 0x1220;
const PRODUCT_FULL: u16 = 0x8fe0;
const PRODUCT_MINI: u16 = 0x8fe4;

/// Control requests of the vendor interface.
const REQUEST_ACTIVATE: u8 = 0;
const REQUEST_RESET: u8 = 1;
const REQUEST_COMMAND: u8 = 2;
const REQUEST_ANSWER: u8 = 3;

const ACTIVATE_LEN: usize = 24;
/// The largest answer the device gives.
const ANSWER_LEN: usize = 1040;

/// How many times an answer is asked for before giving up, and how long to
/// wait in between. The device usually answers on the first or second try.
const ATTEMPTS: u32 = 40;
const PAUSE: Duration = Duration::from_millis(3);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Model {
    Full,
    Mini,
}

impl Model {
    fn from_product_id(product_id: u16) -> Option<Self> {
        match product_id {
            PRODUCT_FULL => Some(Self::Full),
            PRODUCT_MINI => Some(Self::Mini),
            _ => None,
        }
    }
}

/// The full-size GoXLR wins when several devices are plugged in.
fn best(models: impl IntoIterator<Item = Model>) -> Option<Model> {
    models.into_iter().fold(None, |best, model| match best {
        Some(Model::Full) => best,
        _ if model == Model::Full => Some(model),
        _ => best.or(Some(model)),
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum TransportError {
    #[error("no full-size GoXLR is plugged in")]
    NoDevice,
    #[error("the device refused: {0}")]
    Io(String),
    #[error("the device did not answer: {0}")]
    NoAnswer(String),
}

/// Tells which GoXLR is plugged in, without taking it.
pub fn probe() -> Result<Option<Model>, TransportError> {
    platform::probe()
}

/// Takes the full-size GoXLR and gets it ready for commands.
pub fn open() -> Result<UsbLink, TransportError> {
    platform::open()
}

/// The name of a running program that drives the GoXLR too, if any.
pub fn rival() -> Option<&'static str> {
    processes::names()
        .iter()
        .find_map(|name| rival::rival_named(name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_product_id_tells_the_model() {
        assert_eq!(Model::from_product_id(0x8fe0), Some(Model::Full));
        assert_eq!(Model::from_product_id(0x8fe4), Some(Model::Mini));
        assert_eq!(Model::from_product_id(0x0000), None);
    }

    #[test]
    fn the_full_size_device_wins_over_a_mini() {
        assert_eq!(best([]), None);
        assert_eq!(best([Model::Mini]), Some(Model::Mini));
        assert_eq!(best([Model::Mini, Model::Full]), Some(Model::Full));
        assert_eq!(best([Model::Full, Model::Mini]), Some(Model::Full));
    }
}
