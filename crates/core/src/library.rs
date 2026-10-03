//! Profiles as files the user can read: one file per piece, one file per
//! profile saying which pieces it is made of. Nothing here knows the device.

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use goxlr_hub_protocol::{Channel, Fader, OutputSet, RoutingInput, RoutingOutput};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::mic::MicFile;
use crate::{FADER_TOLERANCE, MicState, MixerState, can_route, default_routing};

/// The piece every profile starts with.
const FIRST_NAME: &str = "Default";

const MAX_NAME_LENGTH: usize = 60;

/// Names Windows keeps for itself, whatever follows the first dot.
const RESERVED_NAMES: [&str; 22] = [
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// What can be saved under a name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Kind {
    /// Which pieces go together.
    Profile,
    /// Faders, volumes and routing.
    Mix,
    /// The microphone and its processing.
    Mic,
}

impl Kind {
    const ALL: [Self; 3] = [Self::Profile, Self::Mix, Self::Mic];

    fn folder(self) -> &'static str {
        match self {
            Self::Profile => "profiles",
            Self::Mix => "mixes",
            Self::Mic => "mics",
        }
    }
}

/// Why a profile could not be saved, loaded or changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ProfileError {
    /// The name cannot be the name of a file.
    InvalidName,
    /// Something of the same kind has that name already.
    NameTaken,
    /// It is the one in use, or a profile is made of it.
    InUse,
    NotFound,
    /// The file is there but makes no sense.
    Unreadable,
    /// The disk refused.
    Storage,
}

/// The pieces a profile is made of, by name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Assembly {
    pub mix: String,
    pub mic: String,
    /// Nothing to keep yet.
    #[serde(default = "first_name")]
    pub controls: String,
    /// Nothing to keep yet.
    #[serde(default = "first_name")]
    pub lighting: String,
}

fn first_name() -> String {
    FIRST_NAME.into()
}

impl Assembly {
    fn piece(&self, kind: Kind) -> Option<&str> {
        match kind {
            Kind::Profile => None,
            Kind::Mix => Some(&self.mix),
            Kind::Mic => Some(&self.mic),
        }
    }

    fn piece_mut(&mut self, kind: Kind) -> Option<&mut String> {
        match kind {
            Kind::Profile => None,
            Kind::Mix => Some(&mut self.mix),
            Kind::Mic => Some(&mut self.mic),
        }
    }
}

/// What a profile keeps of the mixer. The mutes are gestures of the moment:
/// they are not kept.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MixPiece {
    pub faders: [Channel; Fader::COUNT],
    /// `None` for a volume that was never set.
    pub volumes: [Option<u8>; Channel::COUNT],
    pub routing: [OutputSet; RoutingInput::COUNT],
}

impl MixPiece {
    pub fn of(mixer: &MixerState) -> Self {
        Self {
            faders: mixer.faders,
            volumes: mixer.volumes,
            routing: mixer.routing,
        }
    }

    /// Sets a mixer as the piece says, leaving its mutes as they are.
    pub fn put(&self, mixer: &mut MixerState) {
        mixer.faders = self.faders;
        mixer.volumes = self.volumes;
        mixer.routing = self.routing;
    }

    /// Whether a mixer is still as the piece says.
    ///
    /// A motorised fader never stops exactly where it was sent. A volume the
    /// mixer does not know says nothing. A volume the piece does not know
    /// only counts once it is set for a channel no fader shows: the others
    /// are read from the faders.
    pub fn matches(&self, mixer: &MixerState) -> bool {
        let volumes_match = Channel::ALL.into_iter().all(|channel| {
            let at = usize::from(channel.index());
            let on_fader = mixer.faders.contains(&channel);
            match (self.volumes[at], mixer.volumes[at]) {
                (_, None) => true,
                (None, Some(_)) => on_fader,
                (Some(kept), Some(now)) if on_fader => kept.abs_diff(now) <= FADER_TOLERANCE,
                (Some(kept), Some(now)) => kept == now,
            }
        });
        self.faders == mixer.faders && self.routing == mixer.routing && volumes_match
    }

    fn to_file(&self) -> MixFile {
        MixFile {
            faders: self.faders.to_vec(),
            volumes: Channel::ALL
                .into_iter()
                .zip(self.volumes)
                .filter_map(|(channel, volume)| Some((channel, volume?)))
                .collect(),
            routing: RoutingInput::ALL
                .into_iter()
                .zip(self.routing)
                .map(|(input, outputs)| (input, outputs.iter().collect()))
                .collect(),
        }
    }

    /// Nothing when the faders are not four different channels. An input the
    /// file does not mention keeps its starting routing; a route that makes
    /// no sense is left out.
    fn from_file(file: MixFile) -> Option<Self> {
        let faders: [Channel; Fader::COUNT] = file.faders.try_into().ok()?;
        let distinct = faders
            .iter()
            .enumerate()
            .all(|(at, channel)| !faders[..at].contains(channel));
        if !distinct {
            return None;
        }
        let mut piece = Self {
            faders,
            volumes: [None; Channel::COUNT],
            routing: default_routing(),
        };
        for (channel, volume) in file.volumes {
            piece.volumes[usize::from(channel.index())] = Some(volume);
        }
        for (input, outputs) in file.routing {
            let allowed: Vec<RoutingOutput> = outputs
                .into_iter()
                .filter(|output| can_route(input, *output))
                .collect();
            piece.routing[input as usize] = OutputSet::of(&allowed);
        }
        Some(piece)
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct MixFile {
    /// The channel under each fader, left to right.
    faders: Vec<Channel>,
    /// 0 to 255. A channel left out was never set.
    #[serde(default)]
    volumes: BTreeMap<Channel, u8>,
    /// The outputs each input is sent to.
    #[serde(default)]
    routing: BTreeMap<RoutingInput, Vec<RoutingOutput>>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct StateFile {
    /// The profile to apply on launch.
    last: Option<String>,
}

/// Checks that a name can be the name of a file, here and on the other
/// systems, and returns it without the spaces around it.
pub fn valid_name(name: &str) -> Result<String, ProfileError> {
    let name = name.trim();
    let stem = name.split('.').next().unwrap_or_default().trim_end();
    let allowed = |character: char| !character.is_control() && !r#"<>:"/\|?*"#.contains(character);
    let valid = !name.is_empty()
        && name.chars().count() <= MAX_NAME_LENGTH
        && name.chars().all(allowed)
        && !name.starts_with('.')
        && !name.ends_with('.')
        && !RESERVED_NAMES
            .iter()
            .any(|reserved| reserved.eq_ignore_ascii_case(stem));
    if valid {
        Ok(name.into())
    } else {
        Err(ProfileError::InvalidName)
    }
}

fn same_name(one: &str, other: &str) -> bool {
    one.to_lowercase() == other.to_lowercase()
}

pub struct Library {
    root: PathBuf,
    /// The names on disk, in `Kind::ALL` order. Read once: listing folders
    /// many times a second would be too much.
    names: [Vec<String>; 3],
}

impl Library {
    /// Opens the folder the profiles are kept in. A folder that is not there
    /// yet holds no profile.
    pub fn open(root: PathBuf) -> Self {
        let mut library = Self {
            root,
            names: Default::default(),
        };
        for kind in Kind::ALL {
            library.refresh(kind);
        }
        library
    }

    /// The names of what is saved, in alphabetical order.
    pub fn names(&self, kind: Kind) -> &[String] {
        &self.names[kind as usize]
    }

    fn refresh(&mut self, kind: Kind) {
        let mut names: Vec<String> = fs::read_dir(self.root.join(kind.folder()))
            .into_iter()
            .flatten()
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| {
                path.extension()
                    .is_some_and(|extension| extension == "toml")
            })
            .filter_map(|path| Some(path.file_stem()?.to_str()?.to_owned()))
            .collect();
        names.sort_by_key(|name| name.to_lowercase());
        self.names[kind as usize] = names;
    }

    fn path(&self, kind: Kind, name: &str) -> PathBuf {
        self.root.join(kind.folder()).join(format!("{name}.toml"))
    }

    /// Only names that were listed are ever turned into a path to read.
    fn known(&self, kind: Kind, name: &str) -> Result<(), ProfileError> {
        if self.names(kind).iter().any(|known| known == name) {
            Ok(())
        } else {
            Err(ProfileError::NotFound)
        }
    }

    /// Whether a name is taken, whatever its case: some systems tell `A`
    /// from `a`, others do not.
    pub fn has(&self, kind: Kind, name: &str) -> bool {
        self.names(kind).iter().any(|known| same_name(known, name))
    }

    fn free(&self, kind: Kind, name: &str) -> Result<String, ProfileError> {
        let name = valid_name(name)?;
        if self.has(kind, &name) {
            return Err(ProfileError::NameTaken);
        }
        Ok(name)
    }

    /// The name asked for when it is free, or the same with a number.
    pub fn free_name(&self, kind: Kind, wanted: &str) -> String {
        if !self.has(kind, wanted) {
            return wanted.into();
        }
        (2..)
            .map(|number| format!("{wanted} {number}"))
            .find(|name| !self.has(kind, name))
            .unwrap_or_else(|| unreachable!())
    }

    fn read<T: DeserializeOwned>(&self, kind: Kind, name: &str) -> Result<T, ProfileError> {
        self.known(kind, name)?;
        let text = fs::read_to_string(self.path(kind, name)).map_err(|_| ProfileError::Storage)?;
        toml::from_str(&text).map_err(|_| ProfileError::Unreadable)
    }

    /// Writes beside the file, then swaps: a crash never leaves half a file.
    fn write<T: Serialize>(
        &mut self,
        kind: Kind,
        name: &str,
        file: &T,
    ) -> Result<(), ProfileError> {
        let name = valid_name(name)?;
        let text = toml::to_string(file).map_err(|_| ProfileError::Storage)?;
        let path = self.path(kind, &name);
        let beside = path.with_extension("toml.new");
        let written = fs::create_dir_all(self.root.join(kind.folder()))
            .and_then(|()| fs::write(&beside, text))
            .and_then(|()| fs::rename(&beside, &path));
        self.refresh(kind);
        written.map_err(|_| ProfileError::Storage)
    }

    pub fn read_profile(&self, name: &str) -> Result<Assembly, ProfileError> {
        self.read(Kind::Profile, name)
    }

    pub fn write_profile(&mut self, name: &str, assembly: &Assembly) -> Result<(), ProfileError> {
        self.write(Kind::Profile, name, assembly)
    }

    pub fn read_mix(&self, name: &str) -> Result<MixPiece, ProfileError> {
        MixPiece::from_file(self.read(Kind::Mix, name)?).ok_or(ProfileError::Unreadable)
    }

    pub fn write_mix(&mut self, name: &str, piece: &MixPiece) -> Result<(), ProfileError> {
        self.write(Kind::Mix, name, &piece.to_file())
    }

    pub fn read_mic(&self, name: &str) -> Result<MicState, ProfileError> {
        MicState::from_file(self.read::<MicFile>(Kind::Mic, name)?).ok_or(ProfileError::Unreadable)
    }

    pub fn write_mic(&mut self, name: &str, mic: &MicState) -> Result<(), ProfileError> {
        self.write(Kind::Mic, name, &mic.to_file())
    }

    /// The profiles made of a piece.
    fn users(&self, kind: Kind, name: &str) -> Vec<(String, Assembly)> {
        self.names(Kind::Profile)
            .iter()
            .filter_map(|profile| Some((profile.clone(), self.read_profile(profile).ok()?)))
            .filter(|(_, assembly)| assembly.piece(kind) == Some(name))
            .collect()
    }

    /// Gives another name, and returns it. The profiles made of a renamed
    /// piece follow.
    pub fn rename(&mut self, kind: Kind, name: &str, to: &str) -> Result<String, ProfileError> {
        self.known(kind, name)?;
        let to = valid_name(to)?;
        if to == name {
            return Ok(to);
        }
        // Changing the case of a name is no clash with itself.
        if self.has(kind, &to) && !same_name(name, &to) {
            return Err(ProfileError::NameTaken);
        }
        let users = self.users(kind, name);
        fs::rename(self.path(kind, name), self.path(kind, &to))
            .map_err(|_| ProfileError::Storage)?;
        self.refresh(kind);
        for (profile, mut assembly) in users {
            if let Some(piece) = assembly.piece_mut(kind) {
                piece.clone_from(&to);
            }
            self.write_profile(&profile, &assembly)?;
        }
        if kind == Kind::Profile && self.last().as_deref() == Some(name) {
            self.set_last(&to)?;
        }
        Ok(to)
    }

    /// Copies what is saved under another name, and returns that name.
    pub fn duplicate(&mut self, kind: Kind, name: &str, to: &str) -> Result<String, ProfileError> {
        self.known(kind, name)?;
        let to = self.free(kind, to)?;
        let copied = fs::copy(self.path(kind, name), self.path(kind, &to));
        self.refresh(kind);
        copied.map_err(|_| ProfileError::Storage)?;
        Ok(to)
    }

    /// Deletes what is saved under a name. A piece a profile is made of
    /// stays.
    pub fn delete(&mut self, kind: Kind, name: &str) -> Result<(), ProfileError> {
        self.known(kind, name)?;
        if !self.users(kind, name).is_empty() {
            return Err(ProfileError::InUse);
        }
        let removed = fs::remove_file(self.path(kind, name));
        self.refresh(kind);
        removed.map_err(|_| ProfileError::Storage)
    }

    fn state_path(&self) -> PathBuf {
        self.root.join("state.toml")
    }

    /// The profile to apply on launch.
    pub fn last(&self) -> Option<String> {
        let text = fs::read_to_string(self.state_path()).ok()?;
        toml::from_str::<StateFile>(&text).ok()?.last
    }

    pub fn set_last(&self, name: &str) -> Result<(), ProfileError> {
        let state = StateFile {
            last: Some(name.into()),
        };
        let text = toml::to_string(&state).map_err(|_| ProfileError::Storage)?;
        fs::create_dir_all(&self.root)
            .and_then(|()| fs::write(self.state_path(), text))
            .map_err(|_| ProfileError::Storage)
    }

    /// Controls and lighting have nothing to keep yet. Their files are there
    /// so that the format does not change when they do.
    pub fn write_empty_pieces(&self) -> Result<(), ProfileError> {
        for folder in ["controls", "lighting"] {
            let path = self.root.join(folder).join(format!("{FIRST_NAME}.toml"));
            if !path.exists() {
                fs::create_dir_all(self.root.join(folder))
                    .and_then(|()| fs::write(path, "# Nothing to keep yet.\n"))
                    .map_err(|_| ProfileError::Storage)?;
            }
        }
        Ok(())
    }

    /// A profile of its own pieces, named after it, all free names.
    pub fn new_assembly(&self, profile: &str) -> Assembly {
        Assembly {
            mix: self.free_name(Kind::Mix, profile),
            mic: self.free_name(Kind::Mic, profile),
            controls: first_name(),
            lighting: first_name(),
        }
    }

    /// The name of the profile of a first launch.
    pub fn first_profile_name(&self) -> String {
        self.free_name(Kind::Profile, FIRST_NAME)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use std::sync::atomic::{AtomicU32, Ordering};

    use goxlr_hub_protocol::MicType;

    use super::*;

    /// A folder of its own for a test, emptied when the test ends.
    pub(crate) struct Folder(pub PathBuf);

    impl Folder {
        pub(crate) fn new() -> Self {
            static NEXT: AtomicU32 = AtomicU32::new(0);
            let path = std::env::temp_dir().join(format!(
                "goxlr-hub-test-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            let _ = fs::remove_dir_all(&path);
            Self(path)
        }

        pub(crate) fn put(&self, file: &str, text: &str) {
            let path = self.0.join(file);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, text).unwrap();
        }

        pub(crate) fn text(&self, file: &str) -> String {
            fs::read_to_string(self.0.join(file)).unwrap()
        }
    }

    impl Drop for Folder {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn library() -> (Library, Folder) {
        let folder = Folder::new();
        (Library::open(folder.0.clone()), folder)
    }

    fn a_mix() -> MixPiece {
        let mut mixer = MixerState::unknown();
        mixer.faders = [Channel::Mic, Channel::Game, Channel::Music, Channel::System];
        mixer.volumes[usize::from(Channel::Mic.index())] = Some(200);
        mixer.volumes[usize::from(Channel::Headphones.index())] = Some(90);
        mixer.routing[RoutingInput::Mic as usize] = OutputSet::of(&[RoutingOutput::Headphones]);
        mixer.routing[RoutingInput::Samples as usize] = OutputSet::default();
        MixPiece::of(&mixer)
    }

    fn a_mic() -> MicState {
        let mut mic = MicState::unknown();
        mic.mic_type = Some(MicType::Condenser);
        mic.gains = [30, 41, 12];
        mic.gate.threshold = -42;
        mic.compressor.makeup_gain = 5;
        mic.equalizer[2].frequency = 151.5;
        mic.equalizer[2].gain = -3;
        mic.de_esser = 25;
        mic
    }

    fn assembly(mix: &str, mic: &str) -> Assembly {
        Assembly {
            mix: mix.into(),
            mic: mic.into(),
            controls: first_name(),
            lighting: first_name(),
        }
    }

    #[test]
    fn what_is_written_is_read_back_the_same() {
        let (mut library, folder) = library();
        library.write_mix("Stream", &a_mix()).unwrap();
        library.write_mic("Stream", &a_mic()).unwrap();
        library.write_mic("Blank", &MicState::unknown()).unwrap();
        library
            .write_profile("Stream", &assembly("Stream", "Stream"))
            .unwrap();

        // Another launch.
        let library = Library::open(folder.0.clone());
        assert_eq!(library.read_mix("Stream").unwrap(), a_mix());
        assert_eq!(library.read_mic("Stream").unwrap(), a_mic());
        assert_eq!(library.read_mic("Blank").unwrap(), MicState::unknown());
        assert_eq!(
            library.read_profile("Stream").unwrap(),
            assembly("Stream", "Stream")
        );
        assert_eq!(library.names(Kind::Mic), ["Blank", "Stream"]);
        assert_eq!(library.names(Kind::Profile), ["Stream"]);
    }

    #[test]
    fn the_files_can_be_read_by_their_owner() {
        let (mut library, folder) = library();
        library.write_mix("Stream", &a_mix()).unwrap();
        library.write_mic("Stream", &a_mic()).unwrap();
        library
            .write_profile("Stream", &assembly("Stream", "Voice"))
            .unwrap();

        let mix = folder.text("mixes/Stream.toml");
        assert!(
            mix.contains(r#"faders = ["mic", "game", "music", "system"]"#),
            "{mix}"
        );
        assert!(mix.contains("headphones = 90"), "{mix}");
        let volumes = mix.split("[routing]").next().unwrap();
        assert!(
            !volumes.contains("lineIn"),
            "an unknown volume is left out: {mix}"
        );
        assert!(mix.contains(r#"mic = ["headphones"]"#), "{mix}");
        assert!(mix.contains("samples = []"), "{mix}");

        let mic = folder.text("mics/Stream.toml");
        for expected in [
            r#"type = "condenser""#,
            "deEsser = 25",
            "condenser = 41",
            "[gate]",
            "threshold = -42",
            "makeupGain = 5",
            "[[equalizer]]",
            "frequency = 151.5",
        ] {
            assert!(mic.contains(expected), "{expected} in {mic}");
        }
        assert!(!folder.text("mics/Stream.toml").contains(".new"));

        let profile = folder.text("profiles/Stream.toml");
        for expected in [
            r#"mix = "Stream""#,
            r#"mic = "Voice""#,
            r#"controls = "Default""#,
            r#"lighting = "Default""#,
        ] {
            assert!(profile.contains(expected), "{expected} in {profile}");
        }
    }

    #[test]
    fn a_file_that_makes_no_sense_is_unreadable_not_a_crash() {
        let folder = Folder::new();
        let mic = toml::to_string(&a_mic().to_file()).unwrap();
        folder.put("mixes/Torn.toml", "faders = [\"mic\", \"cha");
        folder.put("mixes/Empty.toml", "");
        folder.put(
            "mixes/Twice.toml",
            r#"faders = ["mic", "mic", "music", "system"]"#,
        );
        folder.put("mixes/Three.toml", r#"faders = ["mic", "chat", "music"]"#);
        folder.put(
            "mixes/Loud.toml",
            "faders = [\"mic\", \"chat\", \"music\", \"system\"]\n[volumes]\nmic = 300\n",
        );
        folder.put(
            "mixes/Nowhere.toml",
            "faders = [\"mic\", \"chat\", \"music\", \"system\"]\n[volumes]\nmoon = 3\n",
        );
        folder.put(
            "mics/Short.toml",
            &mic.replacen("[[equalizer]]", "[[other]]", 1),
        );
        folder.put(
            "mics/Words.toml",
            &mic.replace("deEsser = 25", "deEsser = \"a lot\""),
        );
        folder.put("profiles/Half.toml", "mix = \"Stream\"\n");
        folder.put("profiles/notes.txt", "not a profile");

        let library = Library::open(folder.0.clone());
        for name in ["Torn", "Empty", "Twice", "Three", "Loud", "Nowhere"] {
            assert_eq!(
                library.read_mix(name),
                Err(ProfileError::Unreadable),
                "{name}"
            );
        }
        for name in ["Short", "Words"] {
            assert_eq!(
                library.read_mic(name),
                Err(ProfileError::Unreadable),
                "{name}"
            );
        }
        assert_eq!(library.read_profile("Half"), Err(ProfileError::Unreadable));
        assert_eq!(library.read_profile("Gone"), Err(ProfileError::NotFound));
        assert_eq!(library.read_profile("notes"), Err(ProfileError::NotFound));
        assert_eq!(library.names(Kind::Profile), ["Half"]);
    }

    #[test]
    fn values_out_of_bounds_are_brought_back_within_them() {
        let folder = Folder::new();
        folder.put(
            "mixes/Odd.toml",
            r#"faders = ["mic", "chat", "music", "system"]
[routing]
chat = ["chatMic", "headphones"]
samples = ["sampler", "sampler", "lineOut"]
"#,
        );
        folder.put(
            "mics/Odd.toml",
            r#"deEsser = 250
[gain]
dynamic = 200
[gate]
threshold = -100
attenuation = 180
attack = 99
release = 3
[compressor]
threshold = 20
ratio = 99
attack = 99
release = 99
makeupGain = 100
[[equalizer]]
frequency = 5.0
gain = 100
[[equalizer]]
frequency = 280.0
gain = -100
[[equalizer]]
frequency = 100.0
gain = 0
[[equalizer]]
frequency = nan
gain = 0
[[equalizer]]
frequency = 500.0
gain = 0
[[equalizer]]
frequency = 1000.0
gain = 0
[[equalizer]]
frequency = 2000.0
gain = 0
[[equalizer]]
frequency = 4000.0
gain = 0
[[equalizer]]
frequency = 8000.0
gain = 0
[[equalizer]]
frequency = 90000.0
gain = 0
"#,
        );
        let library = Library::open(folder.0.clone());

        let mix = library.read_mix("Odd").unwrap();
        assert_eq!(mix.volumes, [None; Channel::COUNT]);
        assert_eq!(
            mix.routing[RoutingInput::Chat as usize],
            OutputSet::of(&[RoutingOutput::Headphones])
        );
        assert_eq!(
            mix.routing[RoutingInput::Samples as usize],
            OutputSet::of(&[RoutingOutput::LineOut])
        );
        assert_eq!(
            mix.routing[RoutingInput::Game as usize],
            default_routing()[RoutingInput::Game as usize]
        );

        let mic = library.read_mic("Odd").unwrap();
        assert_eq!(mic.mic_type, None);
        assert_eq!(mic.gains, [72, 30, 30]);
        assert_eq!(
            (mic.gate.threshold, mic.gate.attenuation, mic.gate.attack),
            (-59, 100, 45)
        );
        assert_eq!(
            (
                mic.compressor.threshold,
                mic.compressor.ratio,
                mic.compressor.attack,
                mic.compressor.release,
                mic.compressor.makeup_gain
            ),
            (0, 14, 19, 19, 24)
        );
        let frequencies = mic.equalizer.map(|point| point.frequency);
        assert_eq!(
            frequencies,
            [
                30.0, 280.0, 280.0, 280.0, 500.0, 1000.0, 2000.0, 4000.0, 8000.0, 18000.0
            ]
        );
        assert_eq!((mic.equalizer[0].gain, mic.equalizer[1].gain), (9, -9));
        assert_eq!(mic.de_esser, 100);
        // What was read can be sent: every band is where a band can be.
        for band in goxlr_hub_protocol::EqBand::ALL {
            let (min, max) = mic.frequency_bounds(band);
            let frequency = mic.equalizer[band as usize].frequency;
            assert!((min..=max).contains(&frequency), "{band:?}");
        }
    }

    #[test]
    fn only_names_that_can_be_files_everywhere_are_taken() {
        for refused in [
            "",
            "   ",
            ".",
            "..",
            ".hidden",
            "a/b",
            "a\\b",
            "..\\..\\evil",
            "what?",
            "a:b",
            "tab\there",
            "ends.",
            "CON",
            "nul",
            "nul.txt",
            "Com1",
            "lpt9.old",
            &"x".repeat(61),
        ] {
            assert_eq!(
                valid_name(refused),
                Err(ProfileError::InvalidName),
                "{refused:?}"
            );
        }
        for (taken, kept) in [
            ("Stream", "Stream"),
            ("  Late night  ", "Late night"),
            ("Été 2026", "Été 2026"),
            ("v1.2", "v1.2"),
            ("console", "console"),
            ("Com10", "Com10"),
        ] {
            assert_eq!(valid_name(taken).as_deref(), Ok(kept));
        }
        assert!(valid_name(&"é".repeat(60)).is_ok());

        let (mut library, _folder) = library();
        assert_eq!(
            library.write_mix("../outside", &a_mix()),
            Err(ProfileError::InvalidName)
        );
        assert_eq!(library.read_mix("../outside"), Err(ProfileError::NotFound));
    }

    #[test]
    fn a_name_is_taken_whatever_its_case() {
        let (mut library, _folder) = library();
        library.write_mic("Stream", &a_mic()).unwrap();
        assert!(library.has(Kind::Mic, "stream"));
        assert!(!library.has(Kind::Mix, "Stream"));
        assert_eq!(
            library.duplicate(Kind::Mic, "Stream", "STREAM"),
            Err(ProfileError::NameTaken)
        );
        assert_eq!(library.free_name(Kind::Mic, "Voice"), "Voice");
        assert_eq!(library.free_name(Kind::Mic, "stream"), "stream 2");
        library.write_mic("Stream 2", &a_mic()).unwrap();
        assert_eq!(library.free_name(Kind::Mic, "Stream"), "Stream 3");
    }

    #[test]
    fn renaming_a_piece_takes_the_profiles_made_of_it_along() {
        let (mut library, _folder) = library();
        library.write_mic("Voice", &a_mic()).unwrap();
        library.write_mic("Other", &MicState::unknown()).unwrap();
        library.write_mix("Desk", &a_mix()).unwrap();
        library
            .write_profile("Stream", &assembly("Desk", "Voice"))
            .unwrap();
        library
            .write_profile("Game", &assembly("Desk", "Other"))
            .unwrap();
        library.set_last("Stream").unwrap();

        assert_eq!(
            library.rename(Kind::Mic, "Voice", " Radio "),
            Ok("Radio".into())
        );
        assert_eq!(library.names(Kind::Mic), ["Other", "Radio"]);
        assert_eq!(library.read_mic("Radio").unwrap(), a_mic());
        assert_eq!(library.read_profile("Stream").unwrap().mic, "Radio");
        assert_eq!(library.read_profile("Game").unwrap().mic, "Other");

        assert_eq!(
            library.rename(Kind::Mic, "Radio", "other"),
            Err(ProfileError::NameTaken)
        );
        assert_eq!(
            library.rename(Kind::Mic, "Radio", "a/b"),
            Err(ProfileError::InvalidName)
        );
        assert_eq!(
            library.rename(Kind::Mic, "Gone", "Here"),
            Err(ProfileError::NotFound)
        );
        // Only the case changes: no clash with itself.
        assert_eq!(
            library.rename(Kind::Mic, "Radio", "RADIO"),
            Ok("RADIO".into())
        );
        assert_eq!(library.read_profile("Stream").unwrap().mic, "RADIO");

        // The profile to apply on launch follows its name.
        assert_eq!(
            library.rename(Kind::Profile, "Stream", "Live"),
            Ok("Live".into())
        );
        assert_eq!(library.last().as_deref(), Some("Live"));
        assert_eq!(library.names(Kind::Profile), ["Game", "Live"]);
    }

    #[test]
    fn a_piece_a_profile_is_made_of_cannot_be_deleted() {
        let (mut library, _folder) = library();
        library.write_mic("Voice", &a_mic()).unwrap();
        library.write_mix("Desk", &a_mix()).unwrap();
        library
            .write_profile("Stream", &assembly("Desk", "Voice"))
            .unwrap();
        assert_eq!(
            library.duplicate(Kind::Mic, "Voice", "Spare"),
            Ok("Spare".into())
        );
        assert_eq!(library.read_mic("Spare").unwrap(), a_mic());

        assert_eq!(library.delete(Kind::Mic, "Voice"), Err(ProfileError::InUse));
        assert_eq!(library.delete(Kind::Mix, "Desk"), Err(ProfileError::InUse));
        assert_eq!(library.delete(Kind::Mic, "Spare"), Ok(()));
        assert_eq!(
            library.delete(Kind::Mic, "Spare"),
            Err(ProfileError::NotFound)
        );
        assert_eq!(library.names(Kind::Mic), ["Voice"]);

        assert_eq!(library.delete(Kind::Profile, "Stream"), Ok(()));
        assert_eq!(library.delete(Kind::Mic, "Voice"), Ok(()));
    }

    #[test]
    fn a_mixer_is_compared_to_a_piece_the_way_a_person_would() {
        let piece = a_mix();
        let mut mixer = MixerState::unknown();
        piece.put(&mut mixer);
        assert!(piece.matches(&mixer));

        // Mutes are gestures of the moment.
        mixer.muted[0] = true;
        mixer.mic_off = true;
        assert!(piece.matches(&mixer));

        let mic = usize::from(Channel::Mic.index());
        let music = usize::from(Channel::Music.index());
        let headphones = usize::from(Channel::Headphones.index());
        let line_in = usize::from(Channel::LineIn.index());

        // A motor stops near where it was sent.
        mixer.volumes[mic] = Some(204);
        assert!(piece.matches(&mixer));
        mixer.volumes[mic] = Some(170);
        assert!(!piece.matches(&mixer));
        mixer.volumes[mic] = Some(200);

        // A volume read from a fader the piece knows nothing of is no change.
        mixer.volumes[music] = Some(77);
        assert!(piece.matches(&mixer));
        // A volume another program may have changed is no change.
        mixer.volumes[headphones] = None;
        assert!(piece.matches(&mixer));
        // No fader shows the headphones: one step is a change.
        mixer.volumes[headphones] = Some(91);
        assert!(!piece.matches(&mixer));
        mixer.volumes[headphones] = Some(90);
        // Neither Line In: it was set on screen.
        mixer.volumes[line_in] = Some(10);
        assert!(!piece.matches(&mixer));
        mixer.volumes[line_in] = None;

        mixer.routing[0] = OutputSet::default();
        assert!(!piece.matches(&mixer));
        piece.put(&mut mixer);
        mixer.faders.swap(0, 1);
        assert!(!piece.matches(&mixer));
    }

    #[test]
    fn a_disk_that_refuses_is_an_error_not_a_crash() {
        let folder = Folder::new();
        // A file where the folder should be.
        fs::write(&folder.0, "in the way").unwrap();
        let mut library = Library::open(folder.0.clone());
        assert_eq!(library.names(Kind::Profile), [] as [&str; 0]);
        assert_eq!(
            library.write_mix("Desk", &a_mix()),
            Err(ProfileError::Storage)
        );
        assert_eq!(library.set_last("Desk"), Err(ProfileError::Storage));
        assert_eq!(library.write_empty_pieces(), Err(ProfileError::Storage));
        assert_eq!(library.last(), None);
        fs::remove_file(&folder.0).unwrap();
    }
}
