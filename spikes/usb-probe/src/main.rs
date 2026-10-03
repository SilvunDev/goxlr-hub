mod scribble;

use anyhow::{Context, Result, bail};
use goxlr_types::FaderName;
use goxlr_usb::PID_GOXLR_FULL;
use goxlr_usb::device::base::FullGoXLRDevice;
use goxlr_usb::device::{find_devices, from_device};
use std::thread::sleep;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

const CLOSE_HINT: &str =
    "Could not open the GoXLR. Quit GoXLR Utility (icon next to the clock, Quit) and try again.";

fn open() -> Result<Box<dyn FullGoXLRDevice>> {
    let device = find_devices()
        .into_iter()
        .next()
        .context("No GoXLR found. Check the USB cable and the driver.")?;

    // The receivers are leaked on purpose: the spike handles neither unplugging
    // nor events, it polls the device in a loop.
    let (disconnect_tx, disconnect_rx) = mpsc::channel(8);
    let (event_tx, event_rx) = mpsc::channel(8);
    std::mem::forget(disconnect_rx);
    std::mem::forget(event_rx);

    let goxlr = from_device(device, disconnect_tx, event_tx, false).context(CLOSE_HINT)?;

    let descriptor = goxlr.get_descriptor().context(CLOSE_HINT)?;
    if descriptor.product_id() != PID_GOXLR_FULL {
        bail!(
            "This spike targets the full-size GoXLR. Device found: {}.",
            descriptor.product_name()
        );
    }
    Ok(goxlr)
}

fn info() -> Result<()> {
    let mut goxlr = open()?;
    let descriptor = goxlr.get_descriptor()?;
    println!(
        "Device    : {} ({})",
        descriptor.product_name(),
        descriptor.device_manufacturer()
    );
    println!(
        "Firmware  : {:?}",
        goxlr.get_firmware_version().context(CLOSE_HINT)?
    );
    let (serial, date) = goxlr.get_serial_number().context(CLOSE_HINT)?;
    println!("Serial    : {serial} (manufactured {date})");
    Ok(())
}

fn watch() -> Result<()> {
    let mut goxlr = open()?;
    println!("Move a fader, turn an encoder or press a button. Ctrl+C to stop.");
    let mut last = String::new();
    loop {
        let state = goxlr.get_button_states().context(CLOSE_HINT)?;
        let line = format!(
            "faders {:?} | encoders {:?} | buttons {:?}",
            state.volumes, state.encoders, state.pressed
        );
        if line != last {
            println!("{line}");
            last = line;
        }
        sleep(Duration::from_millis(20));
    }
}

fn anim(requested_fps: u32) -> Result<()> {
    const FRAMES: u32 = 240;
    let fps = scribble::clamp_fps(requested_fps);
    let budget = Duration::from_secs_f64(1.0 / fps as f64);
    let mut goxlr = open()?;

    println!("Sending {FRAMES} frames to the fader A display, target {fps} frames per second.");
    let mut slowest = Duration::ZERO;
    let mut late = 0;
    let start = Instant::now();
    for step in 0..FRAMES {
        let sent_at = Instant::now();
        goxlr
            .set_fader_scribble(FaderName::A, scribble::animation_frame(step).encode())
            .context(CLOSE_HINT)?;
        let took = sent_at.elapsed();
        slowest = slowest.max(took);
        if took > budget {
            late += 1;
        } else {
            sleep(budget - took);
        }
    }
    let total = start.elapsed();

    println!(
        "Actual rate   : {:.1} frames per second",
        FRAMES as f64 / total.as_secs_f64()
    );
    println!("Slowest send  : {:.1} ms", slowest.as_secs_f64() * 1000.0);
    println!("Late frames   : {late} out of {FRAMES}");
    println!("Restart GoXLR Utility to get the usual display back.");
    Ok(())
}

fn run() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("info") => info(),
        Some("watch") => watch(),
        Some("anim") => {
            let fps = match args.get(1) {
                Some(raw) => raw
                    .parse()
                    .context("The frame rate must be a whole number.")?,
                None => 30,
            };
            anim(fps)
        }
        _ => bail!("Usage: usb-probe info | watch | anim [frames_per_second]"),
    }
}

fn main() {
    // goxlr-usb creates tokio channels: give it an active runtime.
    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
    let _guard = runtime.enter();

    if let Err(error) = run() {
        eprintln!("{error:#}");
        std::process::exit(1);
    }
}
