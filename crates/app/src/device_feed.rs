//! Stands between the interface and the device: looks for the real GoXLR
//! every second, does what the interface asks as soon as it asks, reads the
//! state of the device shown many times a second and sends it to the window.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread;
use std::time::{Duration, Instant};

use goxlr_hub_core::{Intent, Port, ProfileCommand, ProfileError, Studio};
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

/// What the interface asks for.
pub enum Ask {
    /// Something of the device. The answer is its next state.
    Intent(Intent),
    /// Something of the profiles. The answer says whether it was done.
    Profile(ProfileCommand, Sender<Result<(), ProfileError>>),
}

/// Where the interface drops what it asks for.
pub struct Asks(pub Sender<Ask>);

/// Whether something was changed and not saved, for whoever wants to quit.
#[derive(Clone, Default)]
pub struct Unsaved(Arc<AtomicBool>);

impl Unsaved {
    pub fn get(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }

    fn set(&self, unsaved: bool) {
        self.0.store(unsaved, Ordering::Relaxed);
    }
}

/// Does what the interface asks until the next reading is due. Returns
/// `false` once nobody can ask anything any more.
fn serve(
    studio: &mut Studio<UsbPort>,
    asks: &Receiver<Ask>,
    unsaved: &Unsaved,
    until: Instant,
) -> bool {
    loop {
        let left = until.saturating_duration_since(Instant::now());
        match asks.recv_timeout(left) {
            Ok(Ask::Intent(intent)) => {
                if let Err(error) = studio.apply(intent) {
                    eprintln!("could not apply {intent:?}: {error}");
                }
            }
            Ok(Ask::Profile(command, answer)) => {
                let done = studio.run(command);
                // Whoever saves to quit must find nothing left to save.
                unsaved.set(studio.unsaved());
                // Nobody waits for the answer any more: nothing to tell.
                let _ = answer.send(done);
            }
            Err(RecvTimeoutError::Timeout) => return true,
            Err(RecvTimeoutError::Disconnected) => return false,
        }
    }
}

/// Starts reporting the state of the device: the real GoXLR when there is
/// one to use, the virtual device otherwise, set as the last profile says.
/// What is sent through the returned handle is applied to the device shown.
/// The profiles are kept in `profiles`.
pub fn start(app: &AppHandle, profiles: PathBuf, unsaved: Unsaved) -> Result<Asks, DeviceError> {
    let mut studio = Studio::open(UsbPort, profiles)?;
    let (sender, asks) = mpsc::channel();

    let app = app.clone();
    thread::Builder::new()
        .name("device-feed".into())
        .spawn(move || {
            let mut scanned: Option<Instant> = None;
            let mut reported = studio.connection().clone();
            loop {
                let interval = if studio.awaits_return() {
                    RETURN_SCAN_INTERVAL
                } else {
                    SCAN_INTERVAL
                };
                if scanned.is_none_or(|at| at.elapsed() >= interval) {
                    studio.scan();
                    scanned = Some(Instant::now());
                }
                let polled = studio.poll();
                unsaved.set(studio.unsaved());
                if *studio.connection() != reported {
                    reported = studio.connection().clone();
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
                if !serve(&mut studio, &asks, &unsaved, Instant::now() + POLL_INTERVAL) {
                    return;
                }
            }
        })
        .map_err(|error| DeviceError::Link(format!("could not start the device feed: {error}")))?;
    Ok(Asks(sender))
}
