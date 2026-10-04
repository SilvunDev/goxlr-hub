//! The part of the updates that uses the network: asking GitHub for the
//! releases, downloading an installer.

use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use ureq::Agent;

use crate::update::{Installer, REPOSITORY, UpdateError, verify};

/// GitHub refuses requests that do not say who asks.
const USER_AGENT: &str = "goxlr-hub (https://github.com/SilvunDev/goxlr-hub)";

/// Enough for the list of releases on a slow line.
const LIST_TIMEOUT: Duration = Duration::from_secs(20);

/// Enough for an installer on a slow line.
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(600);

fn agent(timeout: Duration) -> Agent {
    Agent::config_builder()
        .timeout_global(Some(timeout))
        .https_only(true)
        .user_agent(USER_AGENT)
        .build()
        .into()
}

/// What GitHub says of the releases, as it sends it. No account is used:
/// the request carries nothing about the user or the device.
pub fn releases() -> Result<String, UpdateError> {
    let url = format!("https://api.github.com/repos/{REPOSITORY}/releases?per_page=100");
    agent(LIST_TIMEOUT)
        .get(&url)
        .header("Accept", "application/vnd.github+json")
        .call()
        .map_err(|_| UpdateError::Offline)?
        .body_mut()
        .read_to_string()
        .map_err(|_| UpdateError::Offline)
}

/// Downloads an installer into `folder`, and returns where it is once it is
/// known to be the file that was published. Anything else is deleted.
pub fn download(installer: &Installer, folder: &Path) -> Result<PathBuf, UpdateError> {
    fs::create_dir_all(folder).map_err(|_| UpdateError::Storage)?;
    let path = folder.join(&installer.name);
    let fetched = fetch(installer, &path).and_then(|()| verify(&path, installer));
    if fetched.is_err() {
        let _ = fs::remove_file(&path);
    }
    fetched.map(|()| path)
}

fn fetch(installer: &Installer, path: &Path) -> Result<(), UpdateError> {
    let mut response = agent(DOWNLOAD_TIMEOUT)
        .get(&installer.url)
        .call()
        .map_err(|_| UpdateError::Offline)?;
    // Never more than what was published: a longer answer is cut, and the
    // check that follows refuses it.
    let mut body = response
        .body_mut()
        .with_config()
        .limit(installer.size + 1)
        .reader();
    let mut file = File::create(path).map_err(|_| UpdateError::Storage)?;
    io::copy(&mut body, &mut file).map_err(|error| {
        if error.kind() == io::ErrorKind::StorageFull {
            UpdateError::Storage
        } else {
            UpdateError::Offline
        }
    })?;
    file.sync_all().map_err(|_| UpdateError::Storage)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::update::{Platform, versions};

    /// Uses the network: run by hand with `cargo test -- --ignored`.
    #[test]
    #[ignore = "uses the network"]
    fn github_lists_the_releases_and_serves_a_file_that_passes_the_check() {
        let json = releases().unwrap();
        versions(&json, "0.0.0", Platform::current()).unwrap();

        // Any small file of a release will do: the redirect, the limit and
        // the check are the same.
        let name = "gitleaks_8.24.3_checksums.txt";
        let mut published = Installer {
            name: name.into(),
            url: format!("https://github.com/gitleaks/gitleaks/releases/download/v8.24.3/{name}"),
            size: 1099,
            sha256: "eefa9ab1ee69571248a6fa070a2a7de0837dd9eb72745d131f8766206aac8ac2".into(),
        };
        let folder = std::env::temp_dir().join(format!("goxlr-hub-net-{}", std::process::id()));
        let file = download(&published, &folder).unwrap();
        assert_eq!(file.metadata().unwrap().len(), 1099);

        // The same file under another checksum is refused and removed.
        published.sha256 = "0".repeat(64);
        assert_eq!(download(&published, &folder), Err(UpdateError::Corrupt));
        assert!(!file.exists());
        // A file longer than announced is cut and refused.
        published.size = 100;
        assert!(download(&published, &folder).is_err());
        assert!(!file.exists());
        fs::remove_dir_all(folder).unwrap();
    }
}
