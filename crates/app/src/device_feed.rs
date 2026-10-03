//! Stands between the interface and the device: looks for the real GoXLR
//! every second, does what the interface asks as soon as it asks, reads the
//! state of the device shown many times a second and sends it to the window.

use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread;
use std::time::{Duration, Instant};

use goxlr_hub_core::{Intent, Port, Station};
use goxlr_hub_device::{Device, DeviceError, OpenError, open_hardware};
use tauri::{AppHandle, Emitter};

/// Name of the event carrying a `Snapshot` to the interface.
const EVENT: &str = "device-state";

/// Twenty times a second: smooth enough for the microphone meter.
const POLL_INTERVAL: Duration = Duration::from_millis(50);

/// How often the real GoXLR is looked for, and rival programs checked.
const SCAN_INTERVAL: Duration = Duration::from_secs(1);

/// How often a GoXLR that dropped is looked for: until the app has it
/// again, it plays with its own settings.
const RETURN_SCAN_INTERVAL: Duration = Duration::from_millis(50);

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

/// Where the interface drops what it asks of the device.
pub struct Intents(pub Sender<Intent>);

/// Does what the interface asks until the next reading is due. Returns
/// `false` once nobody can ask anything any more.
fn serve(station: &mut Station<UsbPort>, intents: &Receiver<Intent>, until: Instant) -> bool {
    loop {
        let left = until.saturating_duration_since(Instant::now());
        match intents.recv_timeout(left) {
            Ok(intent) => {
                if let Err(error) = station.apply(intent) {
                    eprintln!("could not apply {intent:?}: {error}");
                }
            }
            Err(RecvTimeoutError::Timeout) => return true,
            Err(RecvTimeoutError::Disconnected) => return false,
        }
    }
}

/// Starts reporting the state of the device: the real GoXLR when there is
/// one to use, the virtual device otherwise. What is sent through the
/// returned handle is applied to the device shown.
pub fn start(app: &AppHandle) -> Result<Intents, DeviceError> {
    let mut station = Station::new(UsbPort)?;
    let (sender, intents) = mpsc::channel();

    let app = app.clone();
    thread::Builder::new()
        .name("device-feed".into())
        .spawn(move || {
            let mut scanned: Option<Instant> = None;
            let mut reported = station.connection().clone();
            loop {
                let interval = if station.awaits_return() {
                    RETURN_SCAN_INTERVAL
                } else {
                    SCAN_INTERVAL
                };
                if scanned.is_none_or(|at| at.elapsed() >= interval) {
                    station.scan();
                    scanned = Some(Instant::now());
                }
                let polled = station.poll();
                if *station.connection() != reported {
                    reported = station.connection().clone();
                    eprintln!("device connection: {reported:?}");
                }
                match polled {
                    Ok(snapshot) => {
                        if let Err(error) = app.emit(EVENT, &snapshot) {
                            eprintln!("could not send the device state to the window: {error}");
                        }
                    }
                    // The interface keeps showing the last state it received.
                    Err(error) => eprintln!("could not read the device: {error}"),
                }
                if !serve(&mut station, &intents, Instant::now() + POLL_INTERVAL) {
                    return;
                }
            }
        })
        .map_err(|error| DeviceError::Link(format!("could not start the device feed: {error}")))?;
    Ok(Intents(sender))
}
