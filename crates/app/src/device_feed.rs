//! Keeps the interface up to date with the device: looks for the real GoXLR
//! every second, asks for the state of the device shown many times a second
//! and sends it to the window.

use std::thread;
use std::time::{Duration, Instant};

use goxlr_hub_core::{Port, Station};
use goxlr_hub_device::{Device, DeviceError, OpenError, open_hardware};
use tauri::{AppHandle, Emitter};

/// Name of the event carrying a `Snapshot` to the interface.
const EVENT: &str = "device-state";

/// Twenty times a second: smooth enough for the microphone meter.
const POLL_INTERVAL: Duration = Duration::from_millis(50);

/// How often the real GoXLR is looked for, and rival programs checked.
const SCAN_INTERVAL: Duration = Duration::from_secs(1);

/// The USB port of the computer.
struct UsbPort;

impl Port for UsbPort {
    fn open(&mut self) -> Result<Box<dyn Device>, OpenError> {
        Ok(Box::new(open_hardware()?))
    }

    fn rival(&mut self) -> Option<String> {
        goxlr_hub_device::rival()
    }
}

/// Starts reporting the state of the device: the real GoXLR when there is
/// one to use, the virtual device otherwise.
pub fn start(app: &AppHandle) -> Result<(), DeviceError> {
    let mut station = Station::new(UsbPort)?;

    let app = app.clone();
    thread::Builder::new()
        .name("device-feed".into())
        .spawn(move || {
            let mut scanned: Option<Instant> = None;
            loop {
                if scanned.is_none_or(|at| at.elapsed() >= SCAN_INTERVAL) {
                    let before = station.connection().clone();
                    station.scan();
                    scanned = Some(Instant::now());
                    if *station.connection() != before {
                        eprintln!("device connection: {:?}", station.connection());
                    }
                }
                match station.poll() {
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
