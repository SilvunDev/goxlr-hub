// Hides the console window on Windows release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod device_feed;
mod locale;
mod prefs;
mod tray;
mod update;
mod update_net;

use std::path::PathBuf;
use std::sync::{Mutex, mpsc};
use std::time::{Duration, SystemTime};

use goxlr_hub_core::{Intent, ProfileCommand, ProfileError, backup};
use serde::Serialize;
use tauri::{AppHandle, Manager, State, WindowEvent};
use tauri_plugin_autostart::{MacosLauncher, ManagerExt};
use tauri_plugin_opener::OpenerExt;

use device_feed::{Ask, Asks, Unsaved};
use locale::Locale;
use prefs::Prefs;
use update::{Platform, UpdateError, Version};

/// Whether the tray icon exists. Without it a hidden window could never be
/// brought back, so closing the window must quit instead.
struct TrayAvailable(bool);

/// Where the preferences of the app are kept.
struct PrefsFile(PathBuf);

/// Where the app keeps what is its own: preferences, profiles, backups.
struct ConfigFolder(PathBuf);

/// The published versions, as GitHub last listed them.
#[derive(Default)]
struct Published(Mutex<Vec<Version>>);

/// What the interface needs to know about updates before asking GitHub.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct UpdateSettings {
    /// The version running.
    version: String,
    /// The app asks GitHub by itself whether a new version is out.
    automatic: bool,
    /// The app can install a version by itself. Elsewhere it opens the
    /// page to download it from.
    installs: bool,
}

/// How long the interface waits to hear that a profile was saved or loaded.
const PROFILE_ANSWER_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, PartialEq, Eq)]
enum CloseAction {
    HideToTray,
    Quit,
}

/// How the app starts with the computer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
struct Startup {
    /// The computer starts the app.
    enabled: bool,
    /// Started by the computer, the app stays in the system tray.
    hidden: bool,
}

#[tauri::command]
fn set_locale(app: AppHandle, tag: String) -> Result<(), String> {
    tray::set_locale(&app, Locale::from_tag(&tag)).map_err(|error| error.to_string())
}

/// Passes on what the interface asks of the mixer. The answer is the next
/// state of the device.
#[tauri::command]
fn mixer_intent(asks: State<'_, Option<Asks>>, intent: Intent) -> Result<(), String> {
    let Some(Asks(sender)) = asks.inner() else {
        return Err("no device to drive".into());
    };
    sender
        .send(Ask::Intent(intent))
        .map_err(|error| error.to_string())
}

/// Passes on what the interface asks of the profiles, and waits to hear
/// whether it was done.
#[tauri::command(async)]
fn profile_command(
    asks: State<'_, Option<Asks>>,
    command: ProfileCommand,
) -> Result<(), ProfileError> {
    let Some(Asks(sender)) = asks.inner() else {
        return Err(ProfileError::Storage);
    };
    let (answer, done) = mpsc::channel();
    sender
        .send(Ask::Profile(command, answer))
        .map_err(|_| ProfileError::Storage)?;
    done.recv_timeout(PROFILE_ANSWER_TIMEOUT)
        .map_err(|_| ProfileError::Storage)?
}

fn read_startup(app: &AppHandle) -> Startup {
    Startup {
        // A system that cannot tell is a system that does not start the app.
        enabled: app.autolaunch().is_enabled().unwrap_or(false),
        hidden: Prefs::load(&app.state::<PrefsFile>().0).start_hidden,
    }
}

#[tauri::command]
fn startup(app: AppHandle) -> Startup {
    read_startup(&app)
}

/// Starts the app with the computer or stops doing so, and says how it is
/// now.
#[tauri::command]
fn set_startup(app: AppHandle, enabled: bool, hidden: bool) -> Result<Startup, String> {
    let launcher = app.autolaunch();
    if enabled {
        launcher.enable().map_err(|error| error.to_string())?;
    } else if launcher.is_enabled().unwrap_or(false) {
        launcher.disable().map_err(|error| error.to_string())?;
    }
    let file = &app.state::<PrefsFile>().0;
    Prefs {
        start_hidden: hidden,
        ..Prefs::load(file)
    }
    .save(file)
    .map_err(|error| error.to_string())?;
    Ok(read_startup(&app))
}

fn read_update_settings(app: &AppHandle) -> UpdateSettings {
    UpdateSettings {
        version: app.package_info().version.to_string(),
        automatic: Prefs::load(&app.state::<PrefsFile>().0).check_updates,
        installs: Platform::current().installs(),
    }
}

#[tauri::command]
fn update_settings(app: AppHandle) -> UpdateSettings {
    read_update_settings(&app)
}

/// Lets the app ask GitHub by itself whether a new version is out, or not,
/// and says how it is now.
#[tauri::command]
fn set_automatic_updates(app: AppHandle, automatic: bool) -> Result<UpdateSettings, String> {
    let file = &app.state::<PrefsFile>().0;
    Prefs {
        check_updates: automatic,
        ..Prefs::load(file)
    }
    .save(file)
    .map_err(|error| error.to_string())?;
    Ok(read_update_settings(&app))
}

/// Asks GitHub for the published versions, latest first.
#[tauri::command(async)]
fn published_versions(app: AppHandle) -> Result<Vec<Version>, UpdateError> {
    let current = app.package_info().version.to_string();
    let versions = update::versions(&update_net::releases()?, &current, Platform::current())?;
    if let Ok(mut published) = app.state::<Published>().0.lock() {
        published.clone_from(&versions);
    }
    Ok(versions)
}

/// Downloads the installer of a published version, checks it, starts it and
/// quits: the installer opens the app again. Newer or older, the user chose.
#[tauri::command(async)]
fn install_version(app: AppHandle, version: String) -> Result<(), UpdateError> {
    if !Platform::current().installs() {
        return Err(UpdateError::Unsupported);
    }
    // The installer closes the app: nothing may be left to save.
    if app.state::<Unsaved>().get() {
        return Err(UpdateError::Unsaved);
    }
    let installer = app
        .state::<Published>()
        .0
        .lock()
        .ok()
        .and_then(|published| {
            published
                .iter()
                .find(|published| published.version == version)
                .and_then(|published| published.installer.clone())
        })
        .ok_or(UpdateError::NotFound)?;
    let folder = std::env::temp_dir().join("goxlr-hub-update");
    let file = update_net::download(&installer, &folder)?;
    // `/P` shows the progress and asks nothing, `/R` opens the app again.
    std::process::Command::new(file)
        .args(["/P", "/R"])
        .spawn()
        .map_err(|_| UpdateError::Storage)?;
    app.exit(0);
    Ok(())
}

/// Opens the page of a published version in the browser.
#[tauri::command]
fn open_release(app: AppHandle, version: String) -> Result<(), UpdateError> {
    let page = update::release_page(&version).ok_or(UpdateError::NotFound)?;
    app.opener()
        .open_url(page, None::<&str>)
        .map_err(|_| UpdateError::Storage)
}

/// Opens the folder the profiles are backed up in.
#[tauri::command]
fn open_backups(app: AppHandle) -> Result<(), String> {
    let folder = app.state::<ConfigFolder>().0.join(backup::FOLDER);
    std::fs::create_dir_all(&folder).map_err(|error| error.to_string())?;
    app.opener()
        .open_path(folder.to_string_lossy(), None::<&str>)
        .map_err(|error| error.to_string())
}

/// Quits for good: the interface asked, or was asked and agreed.
#[tauri::command]
fn quit(app: AppHandle) {
    app.exit(0);
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
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            Some(vec![prefs::AUTOSTART_FLAG]),
        ))
        .plugin(tauri_plugin_opener::init())
        .manage(Published::default())
        .setup(|app| {
            let tray = tray::create(app.handle(), Locale::system());
            if let Err(error) = &tray {
                eprintln!("no system tray icon, closing the window will quit: {error}");
            }
            app.manage(TrayAvailable(tray.is_ok()));

            let folder = app.path().app_config_dir()?;
            let prefs_file = folder.join("settings.toml");
            let prefs = Prefs::load(&prefs_file);
            app.manage(PrefsFile(prefs_file));
            app.manage(ConfigFolder(folder.clone()));

            // Before anything reads the profiles: another version of the
            // app may be about to rewrite them.
            let profiles = folder.join("profiles");
            let version = app.package_info().version.to_string();
            match backup::on_version_change(&folder, &profiles, &version, SystemTime::now()) {
                Ok(Some(copy)) => eprintln!("profiles backed up to {}", copy.display()),
                Ok(None) => {}
                Err(error) => eprintln!("could not back the profiles up: {error}"),
            }

            let unsaved = Unsaved::default();
            app.manage(unsaved.clone());
            let asks = device_feed::start(app.handle(), profiles, unsaved);
            if let Err(error) = &asks {
                eprintln!("no device to show: {error}");
            }
            app.manage(asks.ok());

            if !prefs::starts_hidden(std::env::args(), prefs, tray.is_ok()) {
                tray::show_main_window(app.handle());
            }
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
        .invoke_handler(tauri::generate_handler![
            set_locale,
            mixer_intent,
            profile_command,
            startup,
            set_startup,
            update_settings,
            set_automatic_updates,
            published_versions,
            install_version,
            open_release,
            open_backups,
            quit
        ])
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
