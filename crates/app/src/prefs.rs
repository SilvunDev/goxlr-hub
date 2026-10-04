//! What the app remembers about itself, apart from the profiles.

use std::fs;
use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};

/// Given to the app when the computer starts it, to tell that launch from
/// one the user asked for.
pub const AUTOSTART_FLAG: &str = "--autostart";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Prefs {
    /// Started with the computer, the app stays in the system tray.
    pub start_hidden: bool,
    /// The app asks GitHub by itself whether a new version is out.
    pub check_updates: bool,
}

impl Default for Prefs {
    fn default() -> Self {
        Self {
            start_hidden: true,
            check_updates: true,
        }
    }
}

impl Prefs {
    /// A file that is missing or makes no sense gives the defaults.
    pub fn load(path: &Path) -> Self {
        fs::read_to_string(path)
            .ok()
            .and_then(|text| toml::from_str(&text).ok())
            .unwrap_or_default()
    }

    pub fn save(self, path: &Path) -> io::Result<()> {
        let text = toml::to_string(&self).map_err(io::Error::other)?;
        if let Some(folder) = path.parent() {
            fs::create_dir_all(folder)?;
        }
        fs::write(path, text)
    }
}

/// Whether this launch keeps the window closed. Without a tray icon a hidden
/// window could never be brought back, so it is shown.
pub fn starts_hidden(
    arguments: impl IntoIterator<Item = String>,
    prefs: Prefs,
    tray_available: bool,
) -> bool {
    let by_the_computer = arguments
        .into_iter()
        .any(|argument| argument == AUTOSTART_FLAG);
    by_the_computer && prefs.start_hidden && tray_available
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arguments(list: &[&str]) -> Vec<String> {
        list.iter().map(ToString::to_string).collect()
    }

    #[test]
    fn only_a_launch_by_the_computer_keeps_the_window_closed() {
        let hidden = Prefs::default();
        let shown = Prefs {
            start_hidden: false,
            ..hidden
        };
        let by_the_computer = arguments(&["goxlr-hub.exe", AUTOSTART_FLAG]);
        let by_hand = arguments(&["goxlr-hub.exe"]);

        assert!(starts_hidden(by_the_computer.clone(), hidden, true));
        assert!(!starts_hidden(by_hand.clone(), hidden, true));
        assert!(!starts_hidden(by_the_computer.clone(), shown, true));
        assert!(!starts_hidden(by_hand, shown, true));
        // No tray icon: nothing could bring the window back.
        assert!(!starts_hidden(by_the_computer, hidden, false));
    }

    #[test]
    fn preferences_are_kept_and_a_broken_file_gives_the_defaults() {
        let folder = std::env::temp_dir().join(format!("goxlr-hub-prefs-{}", std::process::id()));
        let path = folder.join("deep").join("settings.toml");
        assert_eq!(
            Prefs::load(&path),
            Prefs {
                start_hidden: true,
                check_updates: true
            }
        );

        let shown = Prefs {
            start_hidden: false,
            check_updates: false,
        };
        shown.save(&path).unwrap();
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            "startHidden = false\ncheckUpdates = false\n"
        );
        assert_eq!(Prefs::load(&path), shown);

        // A file from before a setting existed keeps what it says.
        fs::write(&path, "startHidden = false\n").unwrap();
        assert_eq!(
            Prefs::load(&path),
            Prefs {
                start_hidden: false,
                check_updates: true
            }
        );

        for broken in ["startHidden = \"maybe\"", "= = =", ""] {
            fs::write(&path, broken).unwrap();
            assert_eq!(Prefs::load(&path), Prefs::default(), "{broken:?}");
        }
        fs::remove_dir_all(folder).unwrap();
    }
}
