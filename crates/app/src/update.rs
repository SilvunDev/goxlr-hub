//! The published versions of the app, read from what GitHub says of the
//! releases, and the checks a download must pass. Nothing here uses the
//! network.

use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

use semver::Version as Number;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// The repository the releases are published in.
pub const REPOSITORY: &str = "SilvunDev/goxlr-hub";

/// No installer is anywhere near this size: more is not an installer.
const MAX_SIZE: u64 = 200 * 1024 * 1024;

/// Why the versions could not be listed, or one could not be installed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum UpdateError {
    /// GitHub could not be reached.
    Offline,
    /// GitHub answered something that makes no sense.
    Unexpected,
    /// The version is not published, or has no installer for this system.
    NotFound,
    /// What was downloaded is not what was published.
    Corrupt,
    /// Something is not saved: installing would lose it.
    Unsaved,
    /// The disk refused.
    Storage,
    /// This system installs through its package manager.
    Unsupported,
}

/// The systems the app is published for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    Windows,
    Linux,
}

impl Platform {
    pub fn current() -> Self {
        if cfg!(windows) {
            Self::Windows
        } else {
            Self::Linux
        }
    }

    /// Whether the app can install a version by itself. Elsewhere the
    /// package manager does it.
    pub fn installs(self) -> bool {
        self == Self::Windows
    }

    /// The name of the installer of a version, as the release workflow
    /// names it.
    fn installer(self, version: &Number) -> Option<String> {
        match self {
            Self::Windows => Some(format!("goxlr-hub_{version}_windows_x64-setup.exe")),
            Self::Linux => None,
        }
    }
}

/// An installer to download.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Installer {
    pub name: String,
    pub url: String,
    pub size: u64,
    /// Lowercase hexadecimal.
    pub sha256: String,
}

/// A published version, as the interface receives it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Version {
    pub version: String,
    /// When it was published, as GitHub writes dates.
    pub published: String,
    /// It is the one running.
    pub current: bool,
    /// It came out after the one running.
    pub newer: bool,
    /// The app can install it by itself.
    pub installable: bool,
    #[serde(skip)]
    pub installer: Option<Installer>,
}

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    published_at: Option<String>,
    #[serde(default)]
    assets: Vec<Asset>,
}

#[derive(Deserialize)]
struct Asset {
    name: String,
    #[serde(default)]
    size: u64,
    #[serde(default)]
    digest: Option<String>,
}

/// The address of the page of a version. Nothing when the text is not a
/// version: only addresses of the repository are ever opened.
pub fn release_page(version: &str) -> Option<String> {
    let version = Number::parse(version).ok()?;
    Some(format!(
        "https://github.com/{REPOSITORY}/releases/tag/v{version}"
    ))
}

/// The installer of a version, when the release has one that can be
/// trusted: the expected name, a checksum, a size that makes sense. Its
/// address is built here, never taken from the answer.
fn installer(release: &Release, version: &Number, platform: Platform) -> Option<Installer> {
    let name = platform.installer(version)?;
    let asset = release.assets.iter().find(|asset| asset.name == name)?;
    let sha256 = asset.digest.as_deref()?.strip_prefix("sha256:")?;
    let is_checksum = sha256.len() == 64
        && sha256
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte));
    if !is_checksum || asset.size == 0 || asset.size > MAX_SIZE {
        return None;
    }
    Some(Installer {
        url: format!(
            "https://github.com/{REPOSITORY}/releases/download/{}/{name}",
            release.tag_name
        ),
        name,
        size: asset.size,
        sha256: sha256.into(),
    })
}

/// Reads the published versions out of the answer of GitHub, latest first.
/// Drafts, previews and tags that are not versions are left out.
pub fn versions(
    json: &str,
    current: &str,
    platform: Platform,
) -> Result<Vec<Version>, UpdateError> {
    let releases: Vec<Release> = serde_json::from_str(json).map_err(|_| UpdateError::Unexpected)?;
    let current = Number::parse(current).ok();
    let mut found: Vec<(Number, Version)> = releases
        .iter()
        .filter(|release| !release.draft && !release.prerelease)
        .filter_map(|release| {
            let number = Number::parse(release.tag_name.strip_prefix('v')?).ok()?;
            // A preview published as a full release is still a preview.
            if !number.pre.is_empty() {
                return None;
            }
            let installer = installer(release, &number, platform);
            let version = Version {
                version: number.to_string(),
                published: release.published_at.clone().unwrap_or_default(),
                current: current.as_ref() == Some(&number),
                newer: current.as_ref().is_some_and(|current| &number > current),
                installable: installer.is_some(),
                installer,
            };
            Some((number, version))
        })
        .collect();
    found.sort_by(|(one, _), (other, _)| other.cmp(one));
    found.dedup_by(|(one, _), (other, _)| one == other);
    Ok(found.into_iter().map(|(_, version)| version).collect())
}

/// Checks that a downloaded file is the one that was published: same size,
/// same checksum.
pub fn verify(file: &Path, installer: &Installer) -> Result<(), UpdateError> {
    let size = file.metadata().map_err(|_| UpdateError::Storage)?.len();
    if size != installer.size {
        return Err(UpdateError::Corrupt);
    }
    let sha256 = checksum(file).map_err(|_| UpdateError::Storage)?;
    if sha256 == installer.sha256 {
        Ok(())
    } else {
        Err(UpdateError::Corrupt)
    }
}

fn checksum(file: &Path) -> io::Result<String> {
    let mut file = File::open(file)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    /// The checksum of `abc`.
    const ABC: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

    fn asset(name: &str, digest: &str) -> String {
        format!(
            r#"{{"name": "{name}", "size": 3, "digest": "{digest}",
                "browser_download_url": "https://elsewhere.example/{name}"}}"#
        )
    }

    fn release(tag: &str, flags: &str, assets: &[String]) -> String {
        format!(
            r#"{{"tag_name": "{tag}", {flags} "published_at": "2026-10-04T10:00:00Z",
                "assets": [{}]}}"#,
            assets.join(",")
        )
    }

    fn windows_installer(version: &str) -> String {
        asset(
            &format!("goxlr-hub_{version}_windows_x64-setup.exe"),
            &format!("sha256:{ABC}"),
        )
    }

    fn answer() -> String {
        let releases = [
            release("v0.1.0", "", &[windows_installer("0.1.0")]),
            release(
                "v0.3.0",
                "",
                &[
                    asset("goxlr-hub_0.3.0_linux_amd64.deb", &format!("sha256:{ABC}")),
                    windows_installer("0.3.0"),
                ],
            ),
            release("v0.2.0", "", &[windows_installer("0.2.0")]),
            release("v0.4.0", r#""draft": true,"#, &[windows_installer("0.4.0")]),
            release(
                "v0.5.0",
                r#""prerelease": true,"#,
                &[windows_installer("0.5.0")],
            ),
            release("v0.6.0-beta.1", "", &[windows_installer("0.6.0-beta.1")]),
            release("nightly", "", &[]),
        ];
        format!("[{}]", releases.join(","))
    }

    fn numbers(versions: &[Version]) -> Vec<&str> {
        versions
            .iter()
            .map(|version| version.version.as_str())
            .collect()
    }

    #[test]
    fn published_versions_are_listed_latest_first_without_drafts_and_previews() {
        let versions = versions(&answer(), "0.2.0", Platform::Windows).unwrap();
        assert_eq!(numbers(&versions), ["0.3.0", "0.2.0", "0.1.0"]);

        let flags: Vec<(bool, bool)> = versions
            .iter()
            .map(|version| (version.current, version.newer))
            .collect();
        assert_eq!(flags, [(false, true), (true, false), (false, false)]);
        assert_eq!(versions[0].published, "2026-10-04T10:00:00Z");
    }

    #[test]
    fn the_installer_is_fetched_from_the_repository_whatever_the_answer_says() {
        let versions = versions(&answer(), "0.2.0", Platform::Windows).unwrap();
        assert!(versions[0].installable);
        assert_eq!(
            versions[0].installer,
            Some(Installer {
                name: "goxlr-hub_0.3.0_windows_x64-setup.exe".into(),
                url: "https://github.com/SilvunDev/goxlr-hub/releases/download/v0.3.0/goxlr-hub_0.3.0_windows_x64-setup.exe".into(),
                size: 3,
                sha256: ABC.into(),
            })
        );
    }

    #[test]
    fn a_version_without_a_trusted_installer_is_listed_but_not_installable() {
        let name = "goxlr-hub_0.3.0_windows_x64-setup.exe";
        let huge =
            format!(r#"{{"name": "{name}", "size": 999999999999, "digest": "sha256:{ABC}"}}"#);
        let unsure = [
            // Still being built.
            vec![],
            // Another name.
            vec![asset(
                "GoXLR.Hub_0.3.0_x64-setup.exe",
                &format!("sha256:{ABC}"),
            )],
            // No checksum, or not one.
            vec![format!(r#"{{"name": "{name}", "size": 3}}"#)],
            vec![asset(name, "md5:900150983cd24fb0d6963f7d28e17f72")],
            vec![asset(name, "sha256:../../elsewhere")],
            vec![huge],
        ];
        for assets in unsure {
            let json = format!("[{}]", release("v0.3.0", "", &assets));
            let versions = versions(&json, "0.2.0", Platform::Windows).unwrap();
            assert_eq!(numbers(&versions), ["0.3.0"], "{assets:?}");
            assert!(!versions[0].installable, "{assets:?}");
            assert_eq!(versions[0].installer, None, "{assets:?}");
        }
    }

    #[test]
    fn linux_lists_the_versions_and_leaves_installing_to_the_package_manager() {
        assert!(Platform::Windows.installs());
        assert!(!Platform::Linux.installs());
        let versions = versions(&answer(), "0.1.0", Platform::Linux).unwrap();
        assert_eq!(numbers(&versions), ["0.3.0", "0.2.0", "0.1.0"]);
        assert!(versions.iter().all(|version| !version.installable));
        assert!(versions[0].newer);
    }

    #[test]
    fn an_answer_that_makes_no_sense_is_an_error_not_a_crash() {
        for json in ["", "{}", r#"{"message": "API rate limit exceeded"}"#, "[1]"] {
            assert_eq!(
                versions(json, "0.1.0", Platform::Windows),
                Err(UpdateError::Unexpected),
                "{json:?}"
            );
        }
        assert_eq!(versions("[]", "0.1.0", Platform::Windows), Ok(vec![]));
        // A build that does not know its version calls nothing newer.
        let versions = versions(&answer(), "dev", Platform::Windows).unwrap();
        assert!(
            versions
                .iter()
                .all(|version| !version.newer && !version.current)
        );
    }

    #[test]
    fn only_pages_of_the_repository_are_opened() {
        assert_eq!(
            release_page("0.3.0").as_deref(),
            Some("https://github.com/SilvunDev/goxlr-hub/releases/tag/v0.3.0")
        );
        for text in [
            "",
            "latest",
            "0.3.0/../../../elsewhere",
            "0.3.0?x=https://elsewhere",
        ] {
            assert_eq!(release_page(text), None, "{text:?}");
        }
    }

    #[test]
    fn a_download_that_is_not_what_was_published_is_refused() {
        let folder = std::env::temp_dir().join(format!("goxlr-hub-update-{}", std::process::id()));
        fs::create_dir_all(&folder).unwrap();
        let file = folder.join("setup.exe");
        let installer = Installer {
            name: "setup.exe".into(),
            url: String::new(),
            size: 3,
            sha256: ABC.into(),
        };

        fs::write(&file, "abc").unwrap();
        assert_eq!(verify(&file, &installer), Ok(()));
        // Same size, other content.
        fs::write(&file, "abd").unwrap();
        assert_eq!(verify(&file, &installer), Err(UpdateError::Corrupt));
        // Cut short.
        fs::write(&file, "ab").unwrap();
        assert_eq!(verify(&file, &installer), Err(UpdateError::Corrupt));
        fs::remove_file(&file).unwrap();
        assert_eq!(verify(&file, &installer), Err(UpdateError::Storage));
        fs::remove_dir_all(folder).unwrap();
    }

    #[test]
    fn errors_are_sent_as_the_interface_names_them() {
        assert_eq!(
            serde_json::to_value(UpdateError::NotFound).unwrap(),
            "notFound"
        );
    }
}
