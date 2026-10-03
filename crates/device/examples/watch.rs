//! Opens the real GoXLR and prints what the hands do on it.
//!
//!     cargo run -p goxlr-hub-device --example watch
//!
//! Quit GoXLR Utility and the official app first.

use std::thread::sleep;
use std::time::Duration;

use goxlr_hub_device::{Device, open_hardware};

fn main() {
    let mut device = match open_hardware() {
        Ok(device) => device,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    };
    let info = device.info();
    println!("Firmware : {}", info.firmware);
    println!(
        "Serial   : {} (manufactured {})",
        info.serial, info.manufactured
    );
    println!("Move a fader or press a button. Ctrl+C to stop.");

    let mut last = String::new();
    loop {
        let line = match (device.status(), device.mic_level()) {
            (Ok(status), Ok(_)) => format!(
                "faders {:?} | encoders {:?} | buttons {:?}",
                status.faders,
                status.encoders,
                status.pressed.iter().collect::<Vec<_>>()
            ),
            (Err(error), _) | (_, Err(error)) => {
                eprintln!("{error}");
                std::process::exit(1);
            }
        };
        if line != last {
            println!("{line}");
            last = line;
        }
        sleep(Duration::from_millis(50));
    }
}
