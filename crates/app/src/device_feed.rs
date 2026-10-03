//! Keeps the interface up to date with the device: asks the hub for its
//! state many times a second and sends it to the window.

use std::thread;
use std::time::Duration;

use goxlr_hub_core::Hub;
use goxlr_hub_device::{DeviceError, open_virtual};
use tauri::{AppHandle, Emitter};

/// Name of the event carrying a `Snapshot` to the interface.
const EVENT: &str = "device-state";

/// Twenty times a second: smooth enough for the microphone meter.
const POLL_INTERVAL: Duration = Duration::from_millis(50);

/// Connects a device and starts reporting its state.
///
/// Only the virtual device exists for now, so the app always starts in demo
/// mode.
pub fn start(app: &AppHandle) -> Result<(), DeviceError> {
    let (device, _hands) = open_virtual()?;
    let mut hub = Hub::connect(Box::new(device))?;

    let app = app.clone();
    thread::Builder::new()
        .name("device-feed".into())
        .spawn(move || {
            loop {
                match hub.poll() {
                    Ok(snapshot) => {
                        if let Err(error) = app.emit(EVENT, &snapshot) {
                            eprintln!("could not send the device state to the window: {error}");
                        }
                    }
                    // The interface keeps showing the last state it received.
                    Err(error) => eprintln!("could not read the device: {error}"),
                }
                thread::sleep(POLL_INTERVAL);
            }
        })
        .map_err(|error| DeviceError::Link(format!("could not start the device feed: {error}")))?;
    Ok(())
}
