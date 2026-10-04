//! Copies the profiles aside the first time another version of the app
//! starts, before that version reads or writes any of them.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// How many backups are kept. The oldest go first.
pub const KEPT: usize = 10;

/// Folder of the backups, inside the configuration folder of the app.
pub const FOLDER: &str = "backups";

/// Keeps the version that started last.
const MARKER: &str = "last-version.txt";

/// What stands for the version of an app too old to leave its mark.
const UNKNOWN: &str = "unknown";

/// Backs the profiles up when `version` is not the one that started last,
/// and returns where. The folder `config` is the one of the app.
///
/// Nothing is copied when the version is the same, or when there is no
/// profile yet. A copy that fails leaves the mark as it was: the next launch
/// tries again.
pub fn on_version_change(
    config: &Path,
    profiles: &Path,
    version: &str,
    now: SystemTime,
) -> io::Result<Option<PathBuf>> {
    let backups = config.join(FOLDER);
    let marker = backups.join(MARKER);
    let last = fs::read_to_string(&marker)
        .ok()
        .map(|text| text.trim().to_owned());
    if last.as_deref() == Some(version) {
        return Ok(None);
    }

    let made = if has_files(profiles) {
        let from = last.as_deref().map_or(UNKNOWN.into(), safe);
        let name = format!("{}-{from}-to-{}", stamp(now), safe(version));
        let target = backups.join(name);
        // Whatever is in the way is not removed: it may be a backup.
        if target.exists() {
            return Err(io::Error::from(io::ErrorKind::AlreadyExists));
        }
        if let Err(error) = copy_folder(profiles, &target) {
            // Half a backup would pass for a whole one.
            let _ = fs::remove_dir_all(&target);
            return Err(error);
        }
        prune(&backups);
        Some(target)
    } else {
        None
    };
    fs::create_dir_all(&backups)?;
    fs::write(marker, version)?;
    Ok(made)
}

/// A version as part of a folder name: what could leave the folder is
/// replaced.
fn safe(version: &str) -> String {
    version
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '.' | '+' | '-') {
                character
            } else {
                '_'
            }
        })
        .collect()
}

fn has_files(folder: &Path) -> bool {
    fs::read_dir(folder).is_ok_and(|mut entries| entries.next().is_some())
}

fn copy_folder(from: &Path, to: &Path) -> io::Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_folder(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

/// Removes the oldest backups beyond the ones kept. Their names start with
/// their date, so the alphabetical order is the order of time.
fn prune(backups: &Path) {
    let mut folders: Vec<PathBuf> = fs::read_dir(backups)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect();
    folders.sort();
    let excess = folders.len().saturating_sub(KEPT);
    for folder in folders.into_iter().take(excess) {
        let _ = fs::remove_dir_all(folder);
    }
}

/// The time as `2026-10-04-153000`, in universal time.
fn stamp(now: SystemTime) -> String {
    let seconds = now
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_secs());
    let (days, rest) = (seconds / 86_400, seconds % 86_400);
    let (year, month, day) = civil(days);
    format!(
        "{year:04}-{month:02}-{day:02}-{:02}{:02}{:02}",
        rest / 3600,
        rest % 3600 / 60,
        rest % 60
    )
}

/// The date of a day counted from 1970-01-01 (the `civil_from_days`
/// algorithm of Howard Hinnant).
fn civil(days: u64) -> (u64, u64, u64) {
    let shifted = days + 719_468;
    let era = shifted / 146_097;
    let day_of_era = shifted % 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted_month = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * shifted_month + 2) / 5 + 1;
    let month = if shifted_month < 10 {
        shifted_month + 3
    } else {
        shifted_month - 9
    };
    let year = year_of_era + era * 400 + u64::from(month <= 2);
    (year, month, day)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::library::tests::Folder;

    /// 2026-10-04 15:30:00, universal time.
    fn a_time() -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(1_791_127_800)
    }

    fn later(seconds: u64) -> SystemTime {
        a_time() + Duration::from_secs(seconds)
    }

    fn backup(folder: &Folder, version: &str, now: SystemTime) -> io::Result<Option<PathBuf>> {
        on_version_change(&folder.0, &folder.0.join("profiles"), version, now)
    }

    #[test]
    fn dates_are_written_as_people_read_them() {
        assert_eq!(stamp(UNIX_EPOCH), "1970-01-01-000000");
        assert_eq!(stamp(a_time()), "2026-10-04-153000");
        // The last second of a leap February.
        assert_eq!(
            stamp(UNIX_EPOCH + Duration::from_secs(1_709_251_199)),
            "2024-02-29-235959"
        );
    }

    #[test]
    fn a_first_launch_has_nothing_to_back_up_and_leaves_its_mark() {
        let folder = Folder::new();
        assert_eq!(backup(&folder, "0.1.0", a_time()).unwrap(), None);
        assert_eq!(folder.text("backups/last-version.txt"), "0.1.0");

        folder.put("profiles/mixes/Stream.toml", "format = 1\n");
        assert_eq!(backup(&folder, "0.1.0", later(60)).unwrap(), None);
    }

    #[test]
    fn another_version_copies_every_file_before_it_touches_one() {
        let folder = Folder::new();
        backup(&folder, "0.1.0", a_time()).unwrap();
        folder.put("profiles/state.toml", "last = \"Stream\"\n");
        folder.put("profiles/mixes/Stream.toml", "format = 1\n");
        folder.put("profiles/profiles/Stream.toml", "mix = \"Stream\"\n");

        let made = backup(&folder, "0.2.0", a_time()).unwrap();
        let name = "backups/2026-10-04-153000-0.1.0-to-0.2.0";
        assert_eq!(made, Some(folder.0.join(name)));
        assert_eq!(
            folder.text(&format!("{name}/state.toml")),
            "last = \"Stream\"\n"
        );
        assert_eq!(
            folder.text(&format!("{name}/mixes/Stream.toml")),
            "format = 1\n"
        );
        assert_eq!(
            folder.text(&format!("{name}/profiles/Stream.toml")),
            "mix = \"Stream\"\n"
        );
        assert_eq!(folder.text("backups/last-version.txt"), "0.2.0");

        // Going back is a change of version too.
        let back = backup(&folder, "0.1.0", later(1)).unwrap();
        assert_eq!(
            back,
            Some(folder.0.join("backups/2026-10-04-153001-0.2.0-to-0.1.0"))
        );
    }

    #[test]
    fn profiles_from_before_the_mark_are_backed_up_too() {
        let folder = Folder::new();
        folder.put("profiles/mixes/Stream.toml", "format = 1\n");
        let made = backup(&folder, "0.1.0", a_time()).unwrap();
        assert_eq!(
            made,
            Some(folder.0.join("backups/2026-10-04-153000-unknown-to-0.1.0"))
        );
    }

    #[test]
    fn a_mark_that_makes_no_sense_cannot_send_the_backup_elsewhere() {
        let folder = Folder::new();
        folder.put("profiles/mixes/Stream.toml", "format = 1\n");
        folder.put("backups/last-version.txt", "../../elsewhere\n");
        let made = backup(&folder, "0.1.0", a_time()).unwrap().unwrap();
        assert_eq!(made.parent(), Some(folder.0.join("backups").as_path()));
    }

    #[test]
    fn only_the_latest_backups_are_kept() {
        let folder = Folder::new();
        folder.put("profiles/mixes/Stream.toml", "format = 1\n");
        for launch in 0..=KEPT {
            let version = format!("0.{launch}.0");
            backup(&folder, &version, later(launch as u64)).unwrap();
        }
        let mut kept: Vec<String> = fs::read_dir(folder.0.join("backups"))
            .unwrap()
            .flatten()
            .filter(|entry| entry.path().is_dir())
            .map(|entry| entry.file_name().into_string().unwrap())
            .collect();
        kept.sort();
        assert_eq!(kept.len(), KEPT);
        assert_eq!(kept[0], "2026-10-04-153001-0.0.0-to-0.1.0");
    }

    #[test]
    fn a_copy_that_fails_is_tried_again_on_the_next_launch() {
        let folder = Folder::new();
        backup(&folder, "0.1.0", a_time()).unwrap();
        folder.put("profiles/mixes/Stream.toml", "format = 1\n");
        // Something already where the backup should go.
        folder.put("backups/2026-10-04-153000-0.1.0-to-0.2.0", "in the way");

        assert!(backup(&folder, "0.2.0", a_time()).is_err());
        assert_eq!(folder.text("backups/last-version.txt"), "0.1.0");
        assert_eq!(
            folder.text("backups/2026-10-04-153000-0.1.0-to-0.2.0"),
            "in the way"
        );
        assert!(backup(&folder, "0.2.0", later(1)).unwrap().is_some());
    }
}
