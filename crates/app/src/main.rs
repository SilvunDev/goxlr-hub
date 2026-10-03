// Hides the console window on Windows release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod device_feed;
mod locale;
mod tray;

use goxlr_hub_core::Intent;
use tauri::{AppHandle, Manager, State, WindowEvent};

use device_feed::Intents;
use locale::Locale;

/// Whether the tray icon exists. Without it a hidden window could never be
/// brought back, so closing the window must quit instead.
struct TrayAvailable(bool);

#[derive(Debug, PartialEq, Eq)]
enum CloseAction {
    HideToTray,
    Quit,
}

#[tauri::command]
fn set_locale(app: AppHandle, tag: String) -> Result<(), String> {
    tray::set_locale(&app, Locale::from_tag(&tag)).map_err(|error| error.to_string())
}

/// Passes on what the interface asks of the mixer. The answer is the next
/// state of the device.
#[tauri::command]
fn mixer_intent(intents: State<'_, Option<Intents>>, intent: Intent) -> Result<(), String> {
    let Some(Intents(sender)) = intents.inner() else {
        return Err("no device to drive".into());
    };
    sender.send(intent).map_err(|error| error.to_string())
}

fn close_action(tray_available: bool) -> CloseAction {
    if tray_available {
        CloseAction::HideToTray
    } else {
        CloseAction::Quit
    }
}

fn main() {
    tauri::Builder::default()
        // Must be registered first: a second launch only brings the window back.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            tray::show_main_window(app);
        }))
        .setup(|app| {
            let tray = tray::create(app.handle(), Locale::system());
            if let Err(error) = &tray {
                eprintln!("no system tray icon, closing the window will quit: {error}");
            }
            app.manage(TrayAvailable(tray.is_ok()));
            let intents = device_feed::start(app.handle());
            if let Err(error) = &intents {
                eprintln!("no device to show: {error}");
            }
            app.manage(intents.ok());
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                let tray_available = window.state::<TrayAvailable>().0;
                if close_action(tray_available) == CloseAction::HideToTray {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![set_locale, mixer_intent])
        .run(tauri::generate_context!())
        .expect("failed to run GoXLR Hub");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closing_hides_only_when_the_tray_can_bring_the_window_back() {
        assert_eq!(close_action(true), CloseAction::HideToTray);
        assert_eq!(close_action(false), CloseAction::Quit);
    }
}
