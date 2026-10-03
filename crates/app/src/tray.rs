//! The system tray icon: the only way back to the window once it is closed.

use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, Wry};

use crate::device_feed::Unsaved;
use crate::locale::{Locale, tray_labels};

/// Name of the event telling the interface that the user wants to quit with
/// unsaved changes.
const QUIT_REQUESTED: &str = "quit-requested";

const TRAY_ID: &str = "main";
const OPEN_ID: &str = "open";
const QUIT_ID: &str = "quit";

pub fn create(app: &AppHandle, locale: Locale) -> tauri::Result<()> {
    let mut builder = TrayIconBuilder::with_id(TRAY_ID)
        .tooltip("GoXLR Hub")
        .menu(&menu(app, locale)?)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            OPEN_ID => show_main_window(app),
            QUIT_ID => request_quit(app),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main_window(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    Ok(())
}

/// Redraws the tray menu in another language.
pub fn set_locale(app: &AppHandle, locale: Locale) -> tauri::Result<()> {
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        tray.set_menu(Some(menu(app, locale)?))?;
    }
    Ok(())
}

/// Quits, unless something was changed and not saved: the window then comes
/// up and asks what to do with it.
fn request_quit(app: &AppHandle) {
    let unsaved = app.state::<Unsaved>().get();
    if unsaved && app.emit(QUIT_REQUESTED, ()).is_ok() {
        show_main_window(app);
    } else {
        app.exit(0);
    }
}

pub fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        // Failures here leave the window as it was; there is nothing to recover.
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

fn menu(app: &AppHandle, locale: Locale) -> tauri::Result<Menu<Wry>> {
    let labels = tray_labels(locale);
    let open = MenuItem::with_id(app, OPEN_ID, labels.open, true, None::<&str>)?;
    let quit = MenuItem::with_id(app, QUIT_ID, labels.quit, true, None::<&str>)?;
    Menu::with_items(app, &[&open, &quit])
}
