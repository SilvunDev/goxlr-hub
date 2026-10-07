//! The device and its profiles together: which profile is in use, what
//! changed since it was saved, and what the interface asks of the profiles.

use std::path::PathBuf;

use goxlr_hub_device::DeviceError;
use serde::{Deserialize, Serialize};

use crate::library::{Assembly, Kind, Library, MixPiece, ProfileError};
use crate::{Connection, Controls, Intent, MicState, Port, Settings, Snapshot, Station};

/// What the interface asks of the profiles.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ProfileCommand {
    /// Brings the device to a profile, or to one piece leaving the others.
    Select {
        kind: Kind,
        name: String,
    },
    /// Saves the settings in use. A profile is saved with its pieces.
    Save {
        kind: Kind,
    },
    /// Saves the settings in use under a new name, and goes on with it.
    SaveAs {
        kind: Kind,
        name: String,
    },
    Rename {
        kind: Kind,
        name: String,
        to: String,
    },
    Duplicate {
        kind: Kind,
        name: String,
        to: String,
    },
    Delete {
        kind: Kind,
        name: String,
    },
}

/// The profiles as the interface receives them.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ProfilesView {
    pub active: ActiveView,
    pub profiles: Vec<String>,
    pub mixes: Vec<String>,
    pub mics: Vec<String>,
    pub controls: Vec<String>,
    pub dirty: Dirty,
    /// Something was changed and not saved.
    pub unsaved: bool,
}

/// The names of what is in use.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ActiveView {
    pub profile: String,
    pub mix: String,
    pub mic: String,
    pub controls: String,
}

/// What differs from what is saved.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct Dirty {
    pub mix: bool,
    pub mic: bool,
    pub controls: bool,
    /// The profile is made of other pieces than the ones in use.
    pub profile: bool,
}

/// What the files say of the profile in use.
struct Saved {
    assembly: Assembly,
    mix: MixPiece,
    mic: MicState,
    controls: Controls,
}

pub struct Studio<P: Port> {
    station: Station<P>,
    library: Library,
    /// The name of the profile in use.
    profile: String,
    /// The pieces in use.
    active: Assembly,
    saved: Saved,
}

impl<P: Port> Studio<P> {
    /// Starts on the profile of the last launch. A first launch makes one,
    /// with the settings of a device the app meets.
    pub fn open(port: P, root: PathBuf) -> Result<Self, DeviceError> {
        let mut library = Library::open(root);
        let (profile, saved) =
            Self::last_profile(&library).unwrap_or_else(|| Self::first_profile(&mut library));
        let mut settings = Settings::unknown();
        saved.mix.put(&mut settings.mixer);
        settings.mic = saved.mic.clone();
        settings.controls = saved.controls.clone();
        Ok(Self {
            station: Station::new(port, settings)?,
            library,
            profile,
            active: saved.assembly.clone(),
            saved,
        })
    }

    /// The profile of the last launch, or the first one that can be read.
    fn last_profile(library: &Library) -> Option<(String, Saved)> {
        let found = library
            .last()
            .into_iter()
            .chain(library.names(Kind::Profile).iter().cloned())
            .find_map(|name| Some((Self::read(library, &name).ok()?, name)));
        let (saved, name) = found?;
        // A disk that refuses only means the same search on the next launch.
        let _ = library.set_last(&name);
        Some((name, saved))
    }

    /// A disk that refuses leaves the profile in memory: the app runs, and
    /// saving will say what is wrong.
    fn first_profile(library: &mut Library) -> (String, Saved) {
        let profile = library.first_profile_name();
        let saved = Saved {
            assembly: library.new_assembly(&profile),
            mix: MixPiece::of(&Settings::unknown().mixer),
            mic: MicState::unknown(),
            controls: Controls::default(),
        };
        let _ = library
            .write_mix(&saved.assembly.mix, &saved.mix)
            .and_then(|()| library.write_mic(&saved.assembly.mic, &saved.mic))
            .and_then(|()| library.write_controls(&saved.assembly.controls, &saved.controls))
            .and_then(|()| library.write_empty_pieces())
            .and_then(|()| library.write_profile(&profile, &saved.assembly))
            .and_then(|()| library.set_last(&profile));
        (profile, saved)
    }

    fn read(library: &Library, profile: &str) -> Result<Saved, ProfileError> {
        let assembly = library.read_profile(profile)?;
        Ok(Saved {
            mix: library.read_mix(&assembly.mix)?,
            mic: library.read_mic(&assembly.mic)?,
            // A profile from before the controls were kept names a piece
            // that may never have been written.
            controls: match library.read_controls(&assembly.controls) {
                Err(ProfileError::NotFound) => Controls::default(),
                controls => controls?,
            },
            assembly,
        })
    }

    pub fn connection(&self) -> &Connection {
        self.station.connection()
    }

    /// See [`Station::awaits_return`].
    pub fn awaits_return(&self) -> bool {
        self.station.awaits_return()
    }

    /// See [`Station::scan`].
    pub fn scan(&mut self) {
        self.station.scan();
    }

    /// Does what the interface asked, on the device shown.
    pub fn apply(&mut self, intent: Intent) -> Result<(), DeviceError> {
        self.station.apply(intent)
    }

    /// The picture to draw, with the profiles.
    pub fn poll(&mut self) -> Result<Snapshot, DeviceError> {
        let mut snapshot = self.station.poll()?;
        snapshot.profiles = self.view();
        Ok(snapshot)
    }

    fn dirty(&self) -> Dirty {
        let settings = self.station.settings();
        Dirty {
            mix: !self.saved.mix.matches(&settings.mixer),
            mic: self.saved.mic != settings.mic,
            controls: self.saved.controls != settings.controls,
            profile: self.active != self.saved.assembly,
        }
    }

    /// Something was changed and not saved.
    pub fn unsaved(&self) -> bool {
        let dirty = self.dirty();
        dirty.mix || dirty.mic || dirty.controls || dirty.profile
    }

    fn view(&self) -> ProfilesView {
        ProfilesView {
            active: ActiveView {
                profile: self.profile.clone(),
                mix: self.active.mix.clone(),
                mic: self.active.mic.clone(),
                controls: self.active.controls.clone(),
            },
            profiles: self.library.names(Kind::Profile).to_vec(),
            mixes: self.library.names(Kind::Mix).to_vec(),
            mics: self.library.names(Kind::Mic).to_vec(),
            controls: self.library.names(Kind::Controls).to_vec(),
            dirty: self.dirty(),
            unsaved: self.unsaved(),
        }
    }

    /// Does what the interface asked of the profiles.
    pub fn run(&mut self, command: ProfileCommand) -> Result<(), ProfileError> {
        match command {
            ProfileCommand::Select { kind, name } => self.select(kind, &name),
            ProfileCommand::Save { kind } => self.save(kind),
            ProfileCommand::SaveAs { kind, name } => self.save_as(kind, &name),
            ProfileCommand::Rename { kind, name, to } => self.rename(kind, &name, &to),
            ProfileCommand::Duplicate { kind, name, to } => self.duplicate(kind, &name, &to),
            ProfileCommand::Delete { kind, name } => self.delete(kind, &name),
        }
    }

    /// Brings the device shown to a mix, a microphone and controls. The
    /// mutes stay as they are.
    fn load(
        &mut self,
        mix: &MixPiece,
        mic: &MicState,
        controls: &Controls,
    ) -> Result<(), ProfileError> {
        let mut settings = self.station.settings().clone();
        mix.put(&mut settings.mixer);
        settings.mic = mic.clone();
        settings.controls = controls.clone();
        self.station
            .load(settings)
            .map_err(|_| ProfileError::Storage)
    }

    fn select(&mut self, kind: Kind, name: &str) -> Result<(), ProfileError> {
        match kind {
            Kind::Profile => {
                let saved = Self::read(&self.library, name)?;
                self.load(&saved.mix, &saved.mic, &saved.controls)?;
                self.profile = name.into();
                self.active = saved.assembly.clone();
                self.saved = saved;
                // A disk that refuses only means another profile on the
                // next launch.
                let _ = self.library.set_last(name);
            }
            Kind::Mix => {
                let mix = self.library.read_mix(name)?;
                let settings = self.station.settings();
                let (mic, controls) = (settings.mic.clone(), settings.controls.clone());
                self.load(&mix, &mic, &controls)?;
                self.active.mix = name.into();
                self.saved.mix = mix;
            }
            Kind::Mic => {
                let mic = self.library.read_mic(name)?;
                let settings = self.station.settings();
                let mix = MixPiece::of(&settings.mixer);
                let controls = settings.controls.clone();
                self.load(&mix, &mic, &controls)?;
                self.active.mic = name.into();
                self.saved.mic = mic;
            }
            Kind::Controls => {
                let controls = self.library.read_controls(name)?;
                let settings = self.station.settings();
                let mix = MixPiece::of(&settings.mixer);
                let mic = settings.mic.clone();
                self.load(&mix, &mic, &controls)?;
                self.active.controls = name.into();
                self.saved.controls = controls;
            }
        }
        Ok(())
    }

    fn save_mix(&mut self, name: &str) -> Result<(), ProfileError> {
        let mix = MixPiece::of(&self.station.settings().mixer);
        self.library.write_mix(name, &mix)?;
        self.active.mix = name.into();
        self.saved.mix = mix;
        Ok(())
    }

    fn save_mic(&mut self, name: &str) -> Result<(), ProfileError> {
        let mic = self.station.settings().mic.clone();
        self.library.write_mic(name, &mic)?;
        self.active.mic = name.into();
        self.saved.mic = mic;
        Ok(())
    }

    fn save_controls(&mut self, name: &str) -> Result<(), ProfileError> {
        let controls = self.station.settings().controls.clone();
        self.library.write_controls(name, &controls)?;
        self.active.controls = name.into();
        self.saved.controls = controls;
        Ok(())
    }

    fn save_profile(&mut self, name: &str) -> Result<(), ProfileError> {
        self.library.write_profile(name, &self.active)?;
        self.profile = name.into();
        self.saved.assembly = self.active.clone();
        let _ = self.library.set_last(name);
        Ok(())
    }

    fn save(&mut self, kind: Kind) -> Result<(), ProfileError> {
        let active = self.active.clone();
        match kind {
            Kind::Mix => self.save_mix(&active.mix),
            Kind::Mic => self.save_mic(&active.mic),
            Kind::Controls => self.save_controls(&active.controls),
            Kind::Profile => {
                self.save_mix(&active.mix)?;
                self.save_mic(&active.mic)?;
                self.save_controls(&active.controls)?;
                self.save_profile(&self.profile.clone())
            }
        }
    }

    fn free(&self, kind: Kind, name: &str) -> Result<String, ProfileError> {
        let name = crate::library::valid_name(name)?;
        if self.library.has(kind, &name) {
            return Err(ProfileError::NameTaken);
        }
        Ok(name)
    }

    /// A profile saved under a new name gets pieces of its own, so that two
    /// profiles never change each other by accident.
    fn save_as(&mut self, kind: Kind, name: &str) -> Result<(), ProfileError> {
        let name = self.free(kind, name)?;
        match kind {
            Kind::Mix => self.save_mix(&name),
            Kind::Mic => self.save_mic(&name),
            Kind::Controls => self.save_controls(&name),
            Kind::Profile => {
                let pieces = self.library.new_assembly(&name);
                self.save_mix(&pieces.mix)?;
                self.save_mic(&pieces.mic)?;
                self.save_controls(&pieces.controls)?;
                self.save_profile(&name)
            }
        }
    }

    fn rename(&mut self, kind: Kind, name: &str, to: &str) -> Result<(), ProfileError> {
        let to = self.library.rename(kind, name, to)?;
        match kind {
            Kind::Profile => {
                if self.profile == name {
                    self.profile = to;
                }
            }
            Kind::Mix => {
                for mix in [&mut self.active.mix, &mut self.saved.assembly.mix] {
                    if mix == name {
                        mix.clone_from(&to);
                    }
                }
            }
            Kind::Mic => {
                for mic in [&mut self.active.mic, &mut self.saved.assembly.mic] {
                    if mic == name {
                        mic.clone_from(&to);
                    }
                }
            }
            Kind::Controls => {
                for controls in [&mut self.active.controls, &mut self.saved.assembly.controls] {
                    if controls == name {
                        controls.clone_from(&to);
                    }
                }
            }
        }
        Ok(())
    }

    /// The copy of a profile gets copies of its pieces.
    fn duplicate(&mut self, kind: Kind, name: &str, to: &str) -> Result<(), ProfileError> {
        if kind != Kind::Profile {
            return self.library.duplicate(kind, name, to).map(drop);
        }
        let to = self.free(Kind::Profile, to)?;
        let mut assembly = self.library.read_profile(name)?;
        let pieces = self.library.new_assembly(&to);
        // The controls first: a piece that was never written is made from the
        // starting buttons, and a refusal leaves no orphan piece behind.
        let controls = match self.library.read_controls(&assembly.controls) {
            Err(ProfileError::NotFound) => Controls::default(),
            controls => controls?,
        };
        self.library.write_controls(&pieces.controls, &controls)?;
        self.library
            .duplicate(Kind::Mix, &assembly.mix, &pieces.mix)?;
        self.library
            .duplicate(Kind::Mic, &assembly.mic, &pieces.mic)?;
        assembly.mix = pieces.mix;
        assembly.mic = pieces.mic;
        assembly.controls = pieces.controls;
        self.library.write_profile(&to, &assembly)
    }

    fn delete(&mut self, kind: Kind, name: &str) -> Result<(), ProfileError> {
        let in_use = match kind {
            Kind::Profile => self.profile == name,
            Kind::Mix => self.active.mix == name,
            Kind::Mic => self.active.mic == name,
            Kind::Controls => self.active.controls == name,
        };
        if in_use {
            return Err(ProfileError::InUse);
        }
        self.library.delete(kind, name)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use goxlr_hub_device::{Device, OpenError, VirtualHandle, open_virtual};
    use goxlr_hub_protocol::{
        Button, Channel, EffectKey, Fader, MicType, OutputSet, RoutingInput, RoutingOutput, Side,
    };
    use serde_json::json;

    use super::*;
    use crate::library::tests::Folder;
    use crate::{Action, AudioTarget, CompressorSetting, Gesture, MuteMode};

    /// A port with at most one device to open, and none by default.
    #[derive(Default)]
    struct Socket(Arc<Mutex<Option<Box<dyn Device>>>>);

    impl Port for Socket {
        fn open(&mut self) -> Result<Box<dyn Device>, OpenError> {
            self.0.lock().unwrap().take().ok_or(OpenError::Absent)
        }

        fn rival(&mut self) -> Option<String> {
            None
        }
    }

    /// A launch with no GoXLR.
    fn launch(folder: &Folder) -> Studio<Socket> {
        Studio::open(Socket::default(), folder.0.clone()).unwrap()
    }

    /// A launch with a GoXLR plugged in. The virtual device stands for it.
    fn launch_plugged(folder: &Folder) -> (Studio<Socket>, VirtualHandle) {
        let (device, hands) = open_virtual().unwrap();
        let socket = Socket(Arc::new(Mutex::new(Some(Box::new(device)))));
        let mut studio = Studio::open(socket, folder.0.clone()).unwrap();
        studio.scan();
        (studio, hands)
    }

    fn at(channel: Channel) -> usize {
        usize::from(channel.index())
    }

    fn select(kind: Kind, name: &str) -> ProfileCommand {
        ProfileCommand::Select {
            kind,
            name: name.into(),
        }
    }

    fn save_as(kind: Kind, name: &str) -> ProfileCommand {
        ProfileCommand::SaveAs {
            kind,
            name: name.into(),
        }
    }

    const SAVE: ProfileCommand = ProfileCommand::Save {
        kind: Kind::Profile,
    };

    fn headphones(volume: u8) -> Intent {
        Intent::SetVolume {
            channel: Channel::Headphones,
            volume,
        }
    }

    #[test]
    fn a_first_launch_makes_a_profile_with_nothing_to_save() {
        let folder = Folder::new();
        let mut studio = launch(&folder);
        let view = studio.poll().unwrap().profiles;
        assert_eq!(
            serde_json::to_value(&view).unwrap(),
            json!({
                "active": {
                    "profile": "Default", "mix": "Default", "mic": "Default", "controls": "Default"
                },
                "profiles": ["Default"],
                "mixes": ["Default"],
                "mics": ["Default"],
                "controls": ["Default"],
                "dirty": { "mix": false, "mic": false, "controls": false, "profile": false },
                "unsaved": false,
            })
        );
        assert!(!studio.unsaved());
        assert!(folder.text("state.toml").contains(r#"last = "Default""#));
        assert!(folder.0.join("controls/Default.toml").exists());
        assert!(folder.0.join("lighting/Default.toml").exists());

        // The same on the next launch.
        let mut studio = launch(&folder);
        assert_eq!(studio.poll().unwrap().profiles, view);
    }

    #[test]
    fn a_change_is_unsaved_until_it_is_saved() {
        let folder = Folder::new();
        let mut studio = launch(&folder);
        studio.apply(headphones(70)).unwrap();
        let dirty = studio.poll().unwrap().profiles.dirty;
        assert_eq!((dirty.mix, dirty.mic, dirty.profile), (true, false, false));
        assert!(studio.unsaved());

        studio.apply(Intent::SetDeEsser { amount: 20 }).unwrap();
        let view = studio.poll().unwrap().profiles;
        assert_eq!((view.dirty.mix, view.dirty.mic), (true, true));
        assert!(view.unsaved);

        // Put back by hand: nothing to save any more.
        studio.apply(Intent::SetDeEsser { amount: 0 }).unwrap();
        assert!(!studio.poll().unwrap().profiles.dirty.mic);

        studio.apply(Intent::SetDeEsser { amount: 20 }).unwrap();
        studio.run(SAVE).unwrap();
        let view = studio.poll().unwrap().profiles;
        assert_eq!(view.dirty, Dirty::default());
        assert!(!view.unsaved && !studio.unsaved());
        assert!(
            folder
                .text("mixes/Default.toml")
                .contains("headphones = 70")
        );
        assert!(folder.text("mics/Default.toml").contains("deEsser = 20"));
    }

    #[test]
    fn one_piece_can_be_saved_without_the_other() {
        let folder = Folder::new();
        let mut studio = launch(&folder);
        studio.apply(headphones(70)).unwrap();
        studio.apply(Intent::SetDeEsser { amount: 20 }).unwrap();
        studio
            .run(ProfileCommand::Save { kind: Kind::Mic })
            .unwrap();
        let dirty = studio.poll().unwrap().profiles.dirty;
        assert_eq!((dirty.mix, dirty.mic), (true, false));
        assert!(
            !folder
                .text("mixes/Default.toml")
                .contains("headphones = 70")
        );
        assert!(folder.text("mics/Default.toml").contains("deEsser = 20"));
    }

    #[test]
    fn the_next_launch_brings_the_device_to_the_last_profile() {
        let folder = Folder::new();
        let mut studio = launch(&folder);
        for intent in [
            headphones(70),
            Intent::SetVolume {
                channel: Channel::Music,
                volume: 150,
            },
            Intent::AssignFader {
                fader: Fader::D,
                channel: Channel::Game,
            },
            Intent::SetRoute {
                input: RoutingInput::Mic,
                output: RoutingOutput::Headphones,
                on: true,
            },
            Intent::SetMicType {
                mic_type: MicType::Dynamic,
            },
            Intent::SetMicGain { gain: 42 },
            Intent::SetCompressor {
                setting: CompressorSetting::MakeupGain,
                value: 3,
            },
            Intent::SetDeEsser { amount: 20 },
        ] {
            studio.apply(intent).unwrap();
        }
        studio.run(save_as(Kind::Profile, "Stream")).unwrap();
        // Changed after saving: lost on purpose.
        studio.apply(headphones(200)).unwrap();
        drop(studio);

        let (mut studio, hands) = launch_plugged(&folder);
        assert_eq!(studio.connection(), &Connection::Hardware);
        let device = hands.state();
        assert_eq!(device.volumes[at(Channel::Headphones)], 70);
        assert_eq!(device.volumes[at(Channel::Music)], 150);
        assert_eq!(device.faders[3], Channel::Game);
        assert!(
            device
                .routed(RoutingInput::Mic, Side::Left)
                .contains(RoutingOutput::Headphones)
        );
        assert_eq!((device.mic_type, device.mic_gain), (MicType::Dynamic, 42));
        assert_eq!(device.effects[&EffectKey::CompressorMakeupGain], 3);
        assert_eq!(device.effects[&EffectKey::DeEsser], 20);

        let view = studio.poll().unwrap().profiles;
        assert_eq!(view.active.profile, "Stream");
        assert!(!view.unsaved, "{:?}", view.dirty);
    }

    #[test]
    fn reading_the_faders_of_a_device_met_for_the_first_time_is_no_change() {
        let folder = Folder::new();
        let (device, hands) = open_virtual().unwrap();
        for (fader, position) in Fader::ALL.into_iter().zip([10, 20, 30, 40]) {
            hands.move_fader(fader, position);
        }
        let socket = Socket(Arc::new(Mutex::new(Some(Box::new(device)))));
        let mut studio = Studio::open(socket, folder.0.clone()).unwrap();
        studio.scan();
        for _ in 0..3 {
            let snapshot = studio.poll().unwrap();
            assert_eq!(snapshot.faders.map(|view| view.volume), [10, 20, 30, 40]);
            assert!(!snapshot.profiles.unsaved);
        }
        // Saving keeps what the faders show.
        studio.run(SAVE).unwrap();
        assert!(folder.text("mixes/Default.toml").contains("music = 30"));
    }

    #[test]
    fn a_mute_is_no_change_and_a_fader_moved_by_hand_is_one() {
        let folder = Folder::new();
        let (mut studio, hands) = launch_plugged(&folder);
        studio
            .apply(Intent::SetVolume {
                channel: Channel::Music,
                volume: 100,
            })
            .unwrap();
        studio.run(SAVE).unwrap();
        studio.poll().unwrap();

        studio
            .apply(Intent::SetMuted {
                channel: Channel::Music,
                muted: true,
            })
            .unwrap();
        studio.apply(Intent::SetMicOff { off: true }).unwrap();
        assert!(!studio.poll().unwrap().profiles.unsaved);

        // The motor stops a little off: no change.
        hands.move_fader(Fader::C, 103);
        assert!(!studio.poll().unwrap().profiles.unsaved);
        hands.move_fader(Fader::C, 130);
        let view = studio.poll().unwrap().profiles;
        assert!(view.unsaved && view.dirty.mix);
    }

    #[test]
    fn a_piece_can_be_changed_leaving_the_others() {
        let folder = Folder::new();
        let (mut studio, hands) = launch_plugged(&folder);
        studio.apply(headphones(70)).unwrap();
        studio.apply(Intent::SetDeEsser { amount: 20 }).unwrap();
        studio.run(save_as(Kind::Mic, "Radio")).unwrap();
        // The profile is now made of another microphone: to save.
        let view = studio.poll().unwrap().profiles;
        assert_eq!(view.active.mic, "Radio");
        assert_eq!(view.mics, ["Default", "Radio"]);
        assert_eq!(
            (view.dirty.mix, view.dirty.mic, view.dirty.profile),
            (true, false, true)
        );

        // Back to the first microphone: the device gets it, the mix and the
        // mutes stay.
        studio
            .apply(Intent::SetMuted {
                channel: Channel::Chat,
                muted: true,
            })
            .unwrap();
        studio.run(select(Kind::Mic, "Default")).unwrap();
        let device = hands.state();
        assert_eq!(device.effects[&EffectKey::DeEsser], 0);
        assert_eq!(device.volumes[at(Channel::Headphones)], 70);
        assert!(device.muted[at(Channel::Chat)]);
        let view = studio.poll().unwrap().profiles;
        assert_eq!(view.active.mic, "Default");
        assert_eq!(
            (view.dirty.mix, view.dirty.mic, view.dirty.profile),
            (true, false, false)
        );

        studio.run(select(Kind::Mic, "Radio")).unwrap();
        assert_eq!(hands.state().effects[&EffectKey::DeEsser], 20);
        studio.run(SAVE).unwrap();
        assert!(
            folder
                .text("profiles/Default.toml")
                .contains(r#"mic = "Radio""#)
        );
        assert!(!studio.unsaved());

        // The unsaved mix is what a chosen mix replaces.
        studio.apply(headphones(90)).unwrap();
        studio.run(select(Kind::Mix, "Default")).unwrap();
        assert_eq!(hands.state().volumes[at(Channel::Headphones)], 70);
        assert!(!studio.unsaved());
    }

    #[test]
    fn a_profile_saved_under_a_new_name_has_pieces_of_its_own() {
        let folder = Folder::new();
        let (mut studio, hands) = launch_plugged(&folder);
        studio.apply(headphones(70)).unwrap();
        studio.run(SAVE).unwrap();
        studio.apply(headphones(120)).unwrap();
        studio.apply(Intent::SetDeEsser { amount: 20 }).unwrap();
        studio.run(save_as(Kind::Profile, " Stream ")).unwrap();

        let view = studio.poll().unwrap().profiles;
        assert_eq!(view.profiles, ["Default", "Stream"]);
        assert_eq!(
            (
                view.active.profile.as_str(),
                view.active.mix.as_str(),
                view.active.mic.as_str()
            ),
            ("Stream", "Stream", "Stream")
        );
        assert!(!view.unsaved);
        assert!(
            folder
                .text("mixes/Default.toml")
                .contains("headphones = 70")
        );
        assert!(
            folder
                .text("mixes/Stream.toml")
                .contains("headphones = 120")
        );

        // Going back gives the first profile as it was saved.
        studio.run(select(Kind::Profile, "Default")).unwrap();
        let device = hands.state();
        assert_eq!(device.volumes[at(Channel::Headphones)], 70);
        assert_eq!(device.effects[&EffectKey::DeEsser], 0);
        assert!(!studio.unsaved());
        assert!(folder.text("state.toml").contains(r#"last = "Default""#));

        for (name, refused) in [
            ("stream", ProfileError::NameTaken),
            ("a/b", ProfileError::InvalidName),
            ("", ProfileError::InvalidName),
        ] {
            assert_eq!(studio.run(save_as(Kind::Profile, name)), Err(refused));
        }
        // A piece of that name is there already: the new profile gets another.
        studio.run(save_as(Kind::Mic, "Night")).unwrap();
        studio.run(save_as(Kind::Profile, "Night")).unwrap();
        let view = studio.poll().unwrap().profiles;
        assert_eq!(view.active.mic, "Night 2");
        assert_eq!(view.active.mix, "Night");
    }

    #[test]
    fn profiles_and_pieces_are_renamed_copied_and_deleted() {
        let folder = Folder::new();
        let mut studio = launch(&folder);
        studio.apply(headphones(70)).unwrap();
        studio.run(SAVE).unwrap();
        let rename = |kind, name: &str, to: &str| ProfileCommand::Rename {
            kind,
            name: name.into(),
            to: to.into(),
        };
        let duplicate = |kind, name: &str, to: &str| ProfileCommand::Duplicate {
            kind,
            name: name.into(),
            to: to.into(),
        };
        let delete = |kind, name: &str| ProfileCommand::Delete {
            kind,
            name: name.into(),
        };

        studio
            .run(rename(Kind::Profile, "Default", "Home"))
            .unwrap();
        studio.run(rename(Kind::Mix, "Default", "Desk")).unwrap();
        let view = studio.poll().unwrap().profiles;
        assert_eq!(view.active.profile, "Home");
        assert_eq!(view.active.mix, "Desk");
        assert!(!view.unsaved, "{:?}", view.dirty);
        assert!(folder.text("state.toml").contains(r#"last = "Home""#));
        assert!(
            folder
                .text("profiles/Home.toml")
                .contains(r#"mix = "Desk""#)
        );

        // The copy of a profile has copies of its pieces.
        studio
            .run(duplicate(Kind::Profile, "Home", "Away"))
            .unwrap();
        assert!(
            folder
                .text("profiles/Away.toml")
                .contains(r#"mix = "Away""#)
        );
        assert!(folder.text("mixes/Away.toml").contains("headphones = 70"));
        assert_eq!(
            studio.run(duplicate(Kind::Profile, "Home", "away")),
            Err(ProfileError::NameTaken)
        );
        studio
            .run(duplicate(Kind::Mic, "Default", "Spare"))
            .unwrap();

        assert_eq!(
            studio.run(delete(Kind::Profile, "Home")),
            Err(ProfileError::InUse)
        );
        assert_eq!(
            studio.run(delete(Kind::Mix, "Desk")),
            Err(ProfileError::InUse)
        );
        assert_eq!(
            studio.run(delete(Kind::Mix, "Away")),
            Err(ProfileError::InUse)
        );
        assert_eq!(
            studio.run(delete(Kind::Mic, "Nothing")),
            Err(ProfileError::NotFound)
        );
        studio.run(delete(Kind::Mic, "Spare")).unwrap();
        studio.run(delete(Kind::Profile, "Away")).unwrap();
        studio.run(delete(Kind::Mix, "Away")).unwrap();
        let view = studio.poll().unwrap().profiles;
        assert_eq!(view.profiles, ["Home"]);
        assert_eq!(view.mixes, ["Desk"]);
        assert_eq!(view.mics, ["Away", "Default"]);

        // A piece in use that no saved profile is made of yet stays too.
        studio.run(select(Kind::Mic, "Away")).unwrap();
        assert_eq!(
            studio.run(delete(Kind::Mic, "Away")),
            Err(ProfileError::InUse)
        );
    }

    #[test]
    fn a_last_profile_that_cannot_be_read_gives_way_to_another() {
        let folder = Folder::new();
        let mut studio = launch(&folder);
        studio.apply(headphones(70)).unwrap();
        studio.run(save_as(Kind::Profile, "Stream")).unwrap();
        drop(studio);

        folder.put("mixes/Stream.toml", "faders = 12");
        let mut studio = launch(&folder);
        let view = studio.poll().unwrap().profiles;
        assert_eq!(view.active.profile, "Default");
        assert!(!view.unsaved);
        // The broken one is still listed, and says so when chosen.
        assert_eq!(view.profiles, ["Default", "Stream"]);
        assert_eq!(
            studio.run(select(Kind::Profile, "Stream")),
            Err(ProfileError::Unreadable)
        );
        assert_eq!(
            studio.run(select(Kind::Profile, "Gone")),
            Err(ProfileError::NotFound)
        );
        assert_eq!(studio.poll().unwrap().profiles.active.profile, "Default");
        drop(studio);

        // Nothing can be read at all: a new profile, beside the broken ones.
        folder.put("mics/Default.toml", "nonsense");
        let mut studio = launch(&folder);
        let view = studio.poll().unwrap().profiles;
        assert_eq!(view.active.profile, "Default 2");
        assert_eq!(view.active.mic, "Default 2");
        assert_eq!(folder.text("mics/Default.toml"), "nonsense");
    }

    #[test]
    fn a_disk_that_refuses_leaves_the_app_running() {
        let folder = Folder::new();
        std::fs::write(&folder.0, "in the way").unwrap();
        let mut studio = launch(&folder);
        let view = studio.poll().unwrap().profiles;
        assert_eq!(view.active.profile, "Default");
        assert!(!view.unsaved);

        studio.apply(headphones(70)).unwrap();
        assert_eq!(studio.run(SAVE), Err(ProfileError::Storage));
        assert!(studio.unsaved(), "nothing was saved");
        assert_eq!(
            studio.run(save_as(Kind::Profile, "Stream")),
            Err(ProfileError::Storage)
        );
        assert_eq!(studio.poll().unwrap().profiles.active.profile, "Default");
        std::fs::remove_file(&folder.0).unwrap();
    }

    #[test]
    fn settings_prepared_without_a_device_reach_it_when_it_comes() {
        let folder = Folder::new();
        let (device, hands) = open_virtual().unwrap();
        let slot = Arc::new(Mutex::new(None));
        let mut studio = Studio::open(Socket(slot.clone()), folder.0.clone()).unwrap();
        studio.scan();
        studio.apply(Intent::SetDeEsser { amount: 20 }).unwrap();
        studio.run(SAVE).unwrap();

        *slot.lock().unwrap() = Some(Box::new(device) as Box<dyn Device>);
        studio.scan();
        assert_eq!(hands.state().effects[&EffectKey::DeEsser], 20);
        assert_eq!(
            hands.state().routed(RoutingInput::Mic, Side::Left),
            OutputSet::of(&[RoutingOutput::BroadcastMix, RoutingOutput::ChatMic])
        );
        assert!(!studio.poll().unwrap().profiles.unsaved);
    }

    #[test]
    fn commands_are_read_from_what_the_interface_sends() {
        let read = |value| serde_json::from_value::<ProfileCommand>(value).unwrap();
        assert_eq!(
            read(json!({ "type": "select", "kind": "mic", "name": "Radio" })),
            select(Kind::Mic, "Radio")
        );
        assert_eq!(read(json!({ "type": "save", "kind": "profile" })), SAVE);
        assert_eq!(
            read(json!({ "type": "saveAs", "kind": "mix", "name": "Desk" })),
            save_as(Kind::Mix, "Desk")
        );
        assert_eq!(
            read(json!({ "type": "rename", "kind": "profile", "name": "A", "to": "B" })),
            ProfileCommand::Rename {
                kind: Kind::Profile,
                name: "A".into(),
                to: "B".into()
            }
        );
        assert_eq!(
            read(json!({ "type": "duplicate", "kind": "mix", "name": "A", "to": "B" })),
            ProfileCommand::Duplicate {
                kind: Kind::Mix,
                name: "A".into(),
                to: "B".into()
            }
        );
        assert_eq!(
            read(json!({ "type": "delete", "kind": "mic", "name": "A" })),
            ProfileCommand::Delete {
                kind: Kind::Mic,
                name: "A".into()
            }
        );
        for refused in [
            json!({ "type": "select", "kind": "lighting", "name": "A" }),
            json!({ "type": "select", "kind": "mic" }),
            json!({ "type": "rename", "kind": "mic", "name": "A" }),
            json!({ "type": "format" }),
        ] {
            assert!(
                serde_json::from_value::<ProfileCommand>(refused.clone()).is_err(),
                "{refused}"
            );
        }
        assert_eq!(
            serde_json::to_value(ProfileError::NameTaken).unwrap(),
            json!("nameTaken")
        );
        assert_eq!(
            read(json!({ "type": "select", "kind": "controls", "name": "A" })),
            ProfileCommand::Select {
                kind: Kind::Controls,
                name: "A".into()
            }
        );
    }

    const PAD: Button = Button::SamplerTopLeft;

    fn mute_music(mode: MuteMode) -> Action {
        Action::Mute {
            target: AudioTarget::Channel {
                channel: Channel::Music,
            },
            mode,
        }
    }

    fn give(button: Button, gesture: Gesture, action: Action) -> Intent {
        Intent::SetGesture {
            button,
            gesture,
            action: Some(action),
        }
    }

    /// What a button does for a gesture, as the interface is told.
    fn does(studio: &mut Studio<Socket>, button: Button, gesture: Gesture) -> Option<Action> {
        let view = studio.poll().unwrap().controls;
        let view = view
            .buttons
            .into_iter()
            .find(|view| view.button == button)
            .unwrap();
        match gesture {
            Gesture::Short => view.short,
            Gesture::Long => view.long,
            Gesture::Double => view.double,
            Gesture::Hold => view.hold,
        }
    }

    #[test]
    fn what_the_buttons_do_is_saved_and_comes_back_with_the_profile() {
        let folder = Folder::new();
        let mut studio = launch(&folder);
        assert!(!studio.unsaved());

        studio
            .apply(give(PAD, Gesture::Long, mute_music(MuteMode::Toggle)))
            .unwrap();
        let view = studio.poll().unwrap().profiles;
        assert_eq!(
            (view.dirty.controls, view.dirty.mix, view.dirty.mic),
            (true, false, false)
        );
        assert!(view.unsaved && studio.unsaved());

        // Put back by hand: nothing to save any more.
        studio
            .apply(Intent::ResetControls { button: Some(PAD) })
            .unwrap();
        assert!(!studio.unsaved());

        studio
            .apply(give(PAD, Gesture::Long, mute_music(MuteMode::Toggle)))
            .unwrap();
        studio
            .apply(Intent::SetPressTimes {
                long_press_ms: 700,
                double_press_ms: 333,
            })
            .unwrap();
        studio.run(SAVE).unwrap();
        assert!(!studio.unsaved());
        let text = folder.text("controls/Default.toml");
        assert!(text.contains("longPressMs = 700"), "{text}");
        assert!(text.contains("samplerTopLeft"), "{text}");

        let mut studio = launch(&folder);
        assert!(!studio.unsaved());
        assert_eq!(
            does(&mut studio, PAD, Gesture::Long),
            Some(mute_music(MuteMode::Toggle))
        );
        assert_eq!(studio.poll().unwrap().controls.long_press_ms, 700);
    }

    #[test]
    fn a_profile_made_before_the_controls_were_kept_shows_no_banner() {
        let folder = Folder::new();
        launch(&folder);
        // As the first versions left it.
        folder.put("controls/Default.toml", "# Nothing to keep yet.\n");
        let mut studio = launch(&folder);
        assert!(!studio.unsaved());
        assert!(
            does(&mut studio, Button::MicMute, Gesture::Short).is_some(),
            "the buttons do what one expects"
        );
        assert_eq!(does(&mut studio, PAD, Gesture::Long), None);

        // Not even a file: the same.
        std::fs::remove_dir_all(folder.0.join("controls")).unwrap();
        let mut studio = launch(&folder);
        assert!(!studio.unsaved());
        assert!(does(&mut studio, Button::MicMute, Gesture::Short).is_some());
        // Saving brings the file back.
        studio
            .apply(give(PAD, Gesture::Long, mute_music(MuteMode::Mute)))
            .unwrap();
        studio.run(SAVE).unwrap();
        assert!(
            folder
                .text("controls/Default.toml")
                .contains("samplerTopLeft")
        );
    }

    #[test]
    fn controls_that_cannot_be_read_do_not_stop_the_launch() {
        let folder = Folder::new();
        launch(&folder);
        folder.put("controls/Default.toml", "format = 1\n[buttons.mic");
        let mut studio = launch(&folder);
        // The first profile that can be read is another one, made on the spot.
        let view = studio.poll().unwrap().profiles;
        assert_eq!(view.active.profile, "Default 2");
        assert!(!studio.unsaved());

        // A file of a newer format is left as it is.
        let folder = Folder::new();
        launch(&folder);
        let later = "format = 2\nshape = \"later\"\n";
        folder.put("controls/Default.toml", later);
        launch(&folder);
        assert_eq!(folder.text("controls/Default.toml"), later);
    }

    #[test]
    fn a_profile_saved_under_a_new_name_gets_controls_of_its_own() {
        let folder = Folder::new();
        let mut studio = launch(&folder);
        studio
            .apply(give(PAD, Gesture::Long, mute_music(MuteMode::Mute)))
            .unwrap();
        studio.run(save_as(Kind::Profile, "Stream")).unwrap();
        let view = studio.poll().unwrap().profiles;
        assert_eq!(view.active.controls, "Stream");
        assert_eq!(view.controls, ["Default", "Stream"]);
        assert!(!view.unsaved);
        assert!(
            folder
                .text("controls/Stream.toml")
                .contains("samplerTopLeft")
        );
        // The old one is as it was.
        assert!(
            !folder
                .text("controls/Default.toml")
                .contains("samplerTopLeft")
        );

        // Changing profile changes the buttons.
        studio.run(select(Kind::Profile, "Default")).unwrap();
        assert_eq!(does(&mut studio, PAD, Gesture::Long), None);
        studio.run(select(Kind::Profile, "Stream")).unwrap();
        assert!(does(&mut studio, PAD, Gesture::Long).is_some());
    }

    #[test]
    fn the_controls_can_be_changed_alone_like_the_mix_and_the_microphone() {
        let folder = Folder::new();
        let mut studio = launch(&folder);
        studio.apply(headphones(70)).unwrap();
        studio
            .apply(give(PAD, Gesture::Long, mute_music(MuteMode::Mute)))
            .unwrap();
        studio.run(save_as(Kind::Controls, "Loud pad")).unwrap();
        let view = studio.poll().unwrap().profiles;
        assert_eq!(view.active.controls, "Loud pad");
        assert!(
            view.dirty.profile,
            "the profile is made of other pieces now"
        );
        assert!(view.dirty.mix);
        assert!(!view.dirty.controls);

        studio.run(select(Kind::Controls, "Default")).unwrap();
        assert_eq!(does(&mut studio, PAD, Gesture::Long), None);
        // The mix was not touched by it.
        assert!(studio.poll().unwrap().profiles.dirty.mix);
        studio.run(select(Kind::Controls, "Loud pad")).unwrap();
        assert!(does(&mut studio, PAD, Gesture::Long).is_some());

        // Renamed, deleted, or refused, like the others.
        studio
            .run(ProfileCommand::Rename {
                kind: Kind::Controls,
                name: "Loud pad".into(),
                to: "Desk".into(),
            })
            .unwrap();
        assert_eq!(studio.poll().unwrap().profiles.active.controls, "Desk");
        let delete = |name: &str| ProfileCommand::Delete {
            kind: Kind::Controls,
            name: name.into(),
        };
        assert_eq!(studio.run(delete("Desk")), Err(ProfileError::InUse));
        // The profile still names it: a piece a profile is made of stays.
        assert_eq!(studio.run(delete("Default")), Err(ProfileError::InUse));
    }

    #[test]
    fn a_profile_copied_gets_copies_of_its_controls() {
        let folder = Folder::new();
        let mut studio = launch(&folder);
        studio
            .apply(give(PAD, Gesture::Long, mute_music(MuteMode::Mute)))
            .unwrap();
        studio.run(SAVE).unwrap();
        studio
            .run(ProfileCommand::Duplicate {
                kind: Kind::Profile,
                name: "Default".into(),
                to: "Copy".into(),
            })
            .unwrap();
        assert!(folder.text("controls/Copy.toml").contains("samplerTopLeft"));
        studio.run(select(Kind::Profile, "Copy")).unwrap();
        assert!(does(&mut studio, PAD, Gesture::Long).is_some());
    }

    #[test]
    fn a_profile_whose_controls_were_never_written_can_still_be_copied() {
        let folder = Folder::new();
        launch(&folder);
        std::fs::remove_dir_all(folder.0.join("controls")).unwrap();
        let mut studio = launch(&folder);
        let copy = ProfileCommand::Duplicate {
            kind: Kind::Profile,
            name: "Default".into(),
            to: "Copy".into(),
        };
        studio.run(copy).unwrap();
        // The copy has all its pieces, the controls made of the starting buttons.
        for piece in ["mixes", "mics", "controls", "profiles"] {
            assert!(folder.0.join(piece).join("Copy.toml").exists(), "{piece}");
        }
        studio.run(select(Kind::Profile, "Copy")).unwrap();
        assert!(does(&mut studio, Button::MicMute, Gesture::Short).is_some());
        assert!(!studio.unsaved());
    }

    #[test]
    fn a_profile_that_cannot_be_copied_leaves_no_piece_behind() {
        let folder = Folder::new();
        launch(&folder);
        // A second profile, so that the first can be torn.
        let mut studio = launch(&folder);
        studio.run(save_as(Kind::Profile, "Other")).unwrap();
        folder.put("controls/Other.toml", "format = 1\n[buttons.mic");
        let copy = ProfileCommand::Duplicate {
            kind: Kind::Profile,
            name: "Other".into(),
            to: "Copy".into(),
        };
        assert_eq!(studio.run(copy), Err(ProfileError::Unreadable));
        for piece in ["mixes", "mics", "controls", "profiles"] {
            assert!(!folder.0.join(piece).join("Copy.toml").exists(), "{piece}");
        }
    }

    #[test]
    fn a_button_held_while_the_profile_changes_ends_with_what_it_began_with() {
        let folder = Folder::new();
        let (mut studio, hands) = launch_plugged(&folder);
        studio
            .apply(Intent::SetGesture {
                button: Button::MicMute,
                gesture: Gesture::Hold,
                action: Some(Action::Mute {
                    target: AudioTarget::Mic,
                    mode: MuteMode::Mute,
                }),
            })
            .unwrap();
        studio.run(save_as(Kind::Profile, "Held")).unwrap();
        // Another profile, where the same button is a plain switch.
        studio.run(select(Kind::Profile, "Default")).unwrap();
        studio.run(select(Kind::Profile, "Held")).unwrap();
        studio.poll().unwrap();

        hands.press(Button::MicMute);
        assert!(studio.poll().unwrap().mic_off);
        studio.run(select(Kind::Profile, "Default")).unwrap();
        assert!(studio.poll().unwrap().mic_off, "still held");
        hands.release(Button::MicMute);
        assert!(
            !studio.poll().unwrap().mic_off,
            "the hold ended as it began"
        );

        // The next press belongs to the profile now in use: a switch.
        hands.press(Button::MicMute);
        assert!(studio.poll().unwrap().mic_off);
        hands.release(Button::MicMute);
        assert!(studio.poll().unwrap().mic_off);
    }
}
