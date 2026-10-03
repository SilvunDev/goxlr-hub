use goxlr_hub_transport::{Model, TransportError, UsbLink};
use thiserror::Error;

use crate::{DeviceError, DeviceKind, Link, Session};

/// Why no real GoXLR could be opened.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum OpenError {
    #[error("no GoXLR is plugged in")]
    Absent,
    #[error("{program} is running and drives the GoXLR")]
    Busy { program: String },
    #[error("only the full-size GoXLR is supported")]
    Unsupported,
    #[error("the GoXLR could not be opened: {0}")]
    Failed(String),
}

/// The USB link to a real GoXLR.
pub struct HardwareLink(UsbLink);

impl Link for HardwareLink {
    fn exchange(&mut self, request: &[u8]) -> Result<Vec<u8>, DeviceError> {
        self.0
            .exchange(request)
            .map_err(|error| DeviceError::Link(error.to_string()))
    }
}

/// Decides, before touching the device, whether it must be left alone.
fn refusal(plugged: Option<Model>, rival: Option<&str>) -> Option<OpenError> {
    match (plugged, rival) {
        (None, _) => Some(OpenError::Absent),
        (Some(Model::Mini), _) => Some(OpenError::Unsupported),
        (Some(Model::Full), Some(program)) => Some(OpenError::Busy {
            program: program.into(),
        }),
        (Some(Model::Full), None) => None,
    }
}

/// The name of a running program that drives the GoXLR too, if any.
pub fn rival() -> Option<String> {
    goxlr_hub_transport::rival().map(String::from)
}

/// Opens the GoXLR plugged into the computer.
///
/// The device is left untouched while another program drives it: two
/// programs would steal each other's answers.
pub fn open_hardware() -> Result<Session<HardwareLink>, OpenError> {
    let failed = |error: TransportError| OpenError::Failed(error.to_string());

    let plugged = goxlr_hub_transport::probe().map_err(failed)?;
    if let Some(refusal) = refusal(plugged, goxlr_hub_transport::rival()) {
        return Err(refusal);
    }
    let link = match goxlr_hub_transport::open() {
        Ok(link) => link,
        // Unplugged between the two calls.
        Err(TransportError::NoDevice) => return Err(OpenError::Absent),
        Err(error) => return Err(failed(error)),
    };
    Session::open(HardwareLink(link), DeviceKind::Hardware)
        .map_err(|error| OpenError::Failed(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_plugged_is_absent_whatever_else_runs() {
        assert_eq!(refusal(None, None), Some(OpenError::Absent));
        assert_eq!(
            refusal(None, Some("GoXLR Utility")),
            Some(OpenError::Absent)
        );
    }

    #[test]
    fn a_mini_is_refused_before_anything_else() {
        assert_eq!(
            refusal(Some(Model::Mini), None),
            Some(OpenError::Unsupported)
        );
        assert_eq!(
            refusal(Some(Model::Mini), Some("GoXLR Utility")),
            Some(OpenError::Unsupported)
        );
    }

    #[test]
    fn a_rival_keeps_the_device_and_is_named() {
        assert_eq!(
            refusal(Some(Model::Full), Some("GoXLR Utility")),
            Some(OpenError::Busy {
                program: "GoXLR Utility".into()
            })
        );
    }

    #[test]
    fn a_full_size_device_left_alone_can_be_opened() {
        assert_eq!(refusal(Some(Model::Full), None), None);
    }
}
