//! What each button does: one action for each gesture. This is the piece of
//! a profile the user calls "controls".

use std::collections::BTreeMap;
use std::time::Duration;

use goxlr_hub_protocol::{Button, Channel, Fader, RoutingInput, RoutingOutput, Wheel};
use serde::{Deserialize, Serialize};

use crate::gestures::{Gesture, Timing, Wanted};
use crate::library::Kind;

/// A press is long from this many milliseconds. The limits keep a press from
/// being long before it is a press, or never.
pub const LONG_PRESS_MS: (u16, u16, u16) = (200, 500, 2000);

/// A second press is waited for this many milliseconds.
pub const DOUBLE_PRESS_MS: (u16, u16, u16) = (150, 333, 1000);

/// A dial moves a volume by this many percent for each notch: the least, the
/// one it starts with, the most.
pub const WHEEL_STEP_PERCENT: (u8, u8, u8) = (1, 4, 10);

/// What a mute action silences.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum AudioTarget {
    /// The microphone itself, like its button does.
    Mic,
    /// One track.
    Channel { channel: Channel },
    /// Whichever track is under a fader.
    FaderTrack { fader: Fader },
}

/// What a mute action does to its target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MuteMode {
    Mute,
    Unmute,
    Toggle,
}

/// What a route action does to a cell of the routing grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RouteMode {
    /// Cuts the input from the output: the cell is unticked.
    Off,
    /// Sends the input to the output: the cell is ticked.
    On,
    Toggle,
}

/// What a volume action does to a volume.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum VolumeMode {
    /// Puts it at a percentage.
    Set,
    /// Raises it by a percentage of the whole range.
    Up,
    /// Lowers it by a percentage of the whole range.
    Down,
}

/// The three banks of the pads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Bank {
    #[default]
    A,
    B,
    C,
}

/// What a gesture does.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Action {
    /// Silences a track or the microphone, or opens it again. Held, it does
    /// so for as long as the button is down.
    Mute { target: AudioTarget, mode: MuteMode },
    /// Ticks or unticks one cell of the routing grid: a track cut toward one
    /// output only. Held, it does so for as long as the button is down.
    Route {
        input: RoutingInput,
        output: RoutingOutput,
        mode: RouteMode,
    },
    /// Sets, raises or lowers the volume of a track, in percent.
    Volume {
        target: AudioTarget,
        mode: VolumeMode,
        percent: u8,
    },
    /// Switches to a profile, or to one piece of a profile, without asking:
    /// what is not saved is lost.
    Profile { kind: Kind, name: String },
    /// Switches the pads to a bank.
    Bank { bank: Bank },
}

impl Action {
    /// Whether it can last as long as the button is held.
    pub fn can_hold(&self) -> bool {
        matches!(self, Self::Mute { .. } | Self::Route { .. })
    }

    /// What an action that turns something on or off wants it to be, given
    /// what it is now. For a mute, on is silenced; for a route, on is sent.
    pub(crate) fn wanted(&self, current: bool) -> Option<bool> {
        match self {
            Self::Mute { mode, .. } => Some(match mode {
                MuteMode::Mute => true,
                MuteMode::Unmute => false,
                MuteMode::Toggle => !current,
            }),
            Self::Route { mode, .. } => Some(match mode {
                RouteMode::On => true,
                RouteMode::Off => false,
                RouteMode::Toggle => !current,
            }),
            _ => None,
        }
    }

    /// The action with what it carries brought back into range.
    fn kept_in_range(self) -> Self {
        match self {
            Self::Volume {
                target,
                mode,
                percent,
            } => Self::Volume {
                target,
                mode,
                percent: percent.min(100),
            },
            other => other,
        }
    }
}

/// What a dial does when it is turned.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum WheelAction {
    /// Each notch changes the volume of a track by `step` percent of the
    /// whole range.
    Volume { target: AudioTarget, step: u8 },
}

impl WheelAction {
    fn kept_in_range(self) -> Self {
        match self {
            Self::Volume { target, step } => Self::Volume {
                target,
                step: step.clamp(WHEEL_STEP_PERCENT.0, WHEEL_STEP_PERCENT.2),
            },
        }
    }
}

/// The actions of one button, by gesture. A button that has a hold has no
/// other gesture: the hold reacts as the finger lands, which leaves no time
/// to tell the others.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ButtonActions {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub short: Option<Action>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub long: Option<Action>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub double: Option<Action>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hold: Option<Action>,
}

impl ButtonActions {
    pub fn get(&self, gesture: Gesture) -> Option<Action> {
        match gesture {
            Gesture::Short => self.short.clone(),
            Gesture::Long => self.long.clone(),
            Gesture::Double => self.double.clone(),
            Gesture::Hold => self.hold.clone(),
        }
    }

    fn slot(&mut self, gesture: Gesture) -> &mut Option<Action> {
        match gesture {
            Gesture::Short => &mut self.short,
            Gesture::Long => &mut self.long,
            Gesture::Double => &mut self.double,
            Gesture::Hold => &mut self.hold,
        }
    }

    pub fn is_empty(&self) -> bool {
        Gesture::ALL
            .into_iter()
            .all(|gesture| self.get(gesture).is_none())
    }

    pub fn all(&self) -> impl Iterator<Item = Action> {
        Gesture::ALL
            .into_iter()
            .filter_map(|gesture| self.get(gesture))
    }

    /// A hold keeps the others out, and the others keep a hold out. A hold
    /// can only be an action that lasts.
    fn set(&mut self, gesture: Gesture, action: Option<Action>) {
        let action = action
            .map(Action::kept_in_range)
            .filter(|action| gesture != Gesture::Hold || action.can_hold());
        if action.is_some() {
            if gesture == Gesture::Hold {
                *self = Self::default();
            } else {
                self.hold = None;
            }
        }
        *self.slot(gesture) = action;
    }

    /// Takes a hold, if there is one, over the other gestures.
    fn settled(mut self) -> Self {
        if let Some(hold) = self.hold.take() {
            self = Self {
                hold: Some(hold),
                ..Self::default()
            };
        }
        self
    }

    fn kept_in_range(self) -> Self {
        Self {
            short: self.short.map(Action::kept_in_range),
            long: self.long.map(Action::kept_in_range),
            double: self.double.map(Action::kept_in_range),
            hold: self.hold.map(Action::kept_in_range),
        }
    }
}

/// What every button does, and how long a press lasts to be long.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Controls {
    /// In milliseconds.
    long_press: u16,
    double_press: u16,
    /// Only the buttons that do something.
    buttons: BTreeMap<Button, ButtonActions>,
    /// Only the dials that do something.
    wheels: BTreeMap<Wheel, WheelAction>,
}

/// What a button does when nobody chose: the mute button of a fader mutes
/// the track under it, the microphone button mutes the microphone, the
/// three bank buttons switch banks.
fn starting(button: Button) -> ButtonActions {
    let mute = |target| Action::Mute {
        target,
        mode: MuteMode::Toggle,
    };
    let under = |fader| mute(AudioTarget::FaderTrack { fader });
    let short = match button {
        Button::Fader1Mute => Some(under(Fader::A)),
        Button::Fader2Mute => Some(under(Fader::B)),
        Button::Fader3Mute => Some(under(Fader::C)),
        Button::Fader4Mute => Some(under(Fader::D)),
        Button::MicMute => Some(mute(AudioTarget::Mic)),
        Button::SamplerSelectA => Some(Action::Bank { bank: Bank::A }),
        Button::SamplerSelectB => Some(Action::Bank { bank: Bank::B }),
        Button::SamplerSelectC => Some(Action::Bank { bank: Bank::C }),
        _ => None,
    };
    ButtonActions {
        short,
        ..ButtonActions::default()
    }
}

impl Default for Controls {
    fn default() -> Self {
        Self {
            long_press: LONG_PRESS_MS.1,
            double_press: DOUBLE_PRESS_MS.1,
            buttons: Button::ALL
                .into_iter()
                .map(|button| (button, starting(button)))
                .filter(|(_, actions)| !actions.is_empty())
                .collect(),
            wheels: BTreeMap::new(),
        }
    }
}

impl Controls {
    /// Nothing at all: every button does nothing.
    #[cfg(test)]
    pub fn blank() -> Self {
        Self {
            buttons: BTreeMap::new(),
            ..Self::default()
        }
    }

    pub fn actions(&self, button: Button) -> ButtonActions {
        self.buttons.get(&button).cloned().unwrap_or_default()
    }

    pub fn action(&self, button: Button, gesture: Gesture) -> Option<Action> {
        self.actions(button).get(gesture)
    }

    /// What a button is waited for.
    pub fn wanted(&self, button: Button) -> Wanted {
        let actions = self.actions(button);
        Wanted {
            long: actions.long.is_some(),
            double: actions.double.is_some(),
            hold: actions.hold.is_some(),
        }
    }

    pub fn timing(&self) -> Timing {
        Timing {
            long: Duration::from_millis(self.long_press.into()),
            double: Duration::from_millis(self.double_press.into()),
        }
    }

    /// What a dial does, if anything.
    pub fn wheel(&self, wheel: Wheel) -> Option<WheelAction> {
        self.wheels.get(&wheel).copied()
    }

    /// Gives a dial something to do, or nothing.
    pub fn set_wheel(&mut self, wheel: Wheel, action: Option<WheelAction>) {
        match action {
            Some(action) => self.wheels.insert(wheel, action.kept_in_range()),
            None => self.wheels.remove(&wheel),
        };
    }

    pub fn long_press(&self) -> u16 {
        self.long_press
    }

    pub fn double_press(&self) -> u16 {
        self.double_press
    }

    /// Gives a gesture of a button an action, or none. A hold takes the
    /// other gestures away, and the other gestures take the hold away.
    pub fn set(&mut self, button: Button, gesture: Gesture, action: Option<Action>) {
        let mut actions = self.actions(button);
        actions.set(gesture, action);
        self.put(button, actions);
    }

    fn put(&mut self, button: Button, actions: ButtonActions) {
        if actions.is_empty() {
            self.buttons.remove(&button);
        } else {
            self.buttons.insert(button, actions);
        }
    }

    /// Puts one button back to what it does when nobody chose.
    pub fn reset_button(&mut self, button: Button) {
        self.put(button, starting(button));
    }

    /// Puts everything back, the times of a press and the dials included.
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Sets how long a press lasts to be long, and how long a second press
    /// is waited for, in milliseconds. Out of range, they are brought back
    /// into it.
    pub fn set_times(&mut self, long_press: u16, double_press: u16) {
        self.long_press = long_press.clamp(LONG_PRESS_MS.0, LONG_PRESS_MS.2);
        self.double_press = double_press.clamp(DOUBLE_PRESS_MS.0, DOUBLE_PRESS_MS.2);
    }

    pub fn view(&self) -> ControlsView {
        ControlsView {
            long_press_ms: self.long_press,
            double_press_ms: self.double_press,
            buttons: Button::ALL
                .into_iter()
                .map(|button| {
                    let actions = self.actions(button);
                    ButtonView {
                        button,
                        short: actions.short,
                        long: actions.long,
                        double: actions.double,
                        hold: actions.hold,
                    }
                })
                .collect(),
            wheels: Wheel::ALL
                .into_iter()
                .map(|wheel| WheelView {
                    wheel,
                    action: self.wheel(wheel),
                })
                .collect(),
        }
    }

    pub(crate) fn to_file(&self) -> ControlsFile {
        ControlsFile {
            long_press_ms: self.long_press,
            double_press_ms: self.double_press,
            buttons: Some(self.buttons.clone()),
            wheels: self.wheels.clone(),
        }
    }

    /// A file that never said what the buttons do, as the first versions
    /// wrote it, gives what buttons do when nobody chose. Times out of range
    /// are brought back into it.
    pub(crate) fn from_file(file: ControlsFile) -> Self {
        let mut controls = Self::default();
        controls.set_times(file.long_press_ms, file.double_press_ms);
        if let Some(buttons) = file.buttons {
            controls.buttons.clear();
            for (button, actions) in buttons {
                let mut actions = actions;
                // What cannot last is no hold, and then the others stay.
                if actions.hold.as_ref().is_some_and(|hold| !hold.can_hold()) {
                    actions.hold = None;
                }
                controls.put(button, actions.kept_in_range().settled());
            }
        }
        for (wheel, action) in file.wheels {
            controls.set_wheel(wheel, Some(action));
        }
        controls
    }
}

/// The controls as they are saved. A file written by this version says what
/// the buttons do, even when it is nothing; a file that does not was never
/// set up.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ControlsFile {
    #[serde(default = "starting_long")]
    long_press_ms: u16,
    #[serde(default = "starting_double")]
    double_press_ms: u16,
    #[serde(default)]
    buttons: Option<BTreeMap<Button, ButtonActions>>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    wheels: BTreeMap<Wheel, WheelAction>,
}

fn starting_long() -> u16 {
    LONG_PRESS_MS.1
}

fn starting_double() -> u16 {
    DOUBLE_PRESS_MS.1
}

/// The controls as the interface receives them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ControlsView {
    pub long_press_ms: u16,
    pub double_press_ms: u16,
    /// Every button, in the order of the device.
    pub buttons: Vec<ButtonView>,
    /// Every dial, in the order of the device.
    pub wheels: Vec<WheelView>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct WheelView {
    pub wheel: Wheel,
    pub action: Option<WheelAction>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ButtonView {
    pub button: Button,
    pub short: Option<Action>,
    pub long: Option<Action>,
    pub double: Option<Action>,
    pub hold: Option<Action>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mute(channel: Channel) -> Action {
        Action::Mute {
            target: AudioTarget::Channel { channel },
            mode: MuteMode::Mute,
        }
    }

    #[test]
    fn a_launch_with_nothing_chosen_has_the_buttons_do_what_one_expects() {
        let controls = Controls::default();
        let short = |button| controls.action(button, Gesture::Short);
        assert_eq!(
            short(Button::Fader3Mute),
            Some(Action::Mute {
                target: AudioTarget::FaderTrack { fader: Fader::C },
                mode: MuteMode::Toggle
            })
        );
        assert_eq!(
            short(Button::MicMute),
            Some(Action::Mute {
                target: AudioTarget::Mic,
                mode: MuteMode::Toggle
            })
        );
        assert_eq!(
            short(Button::SamplerSelectB),
            Some(Action::Bank { bank: Bank::B })
        );
        // The rest is empty.
        let busy = Button::ALL
            .into_iter()
            .filter(|button| !controls.actions(*button).is_empty())
            .count();
        assert_eq!(busy, 8);
        assert_eq!(short(Button::SamplerTopLeft), None);
        assert_eq!(short(Button::Bleep), None);
        assert_eq!(controls.timing().long, Duration::from_millis(500));
        assert_eq!(controls.timing().double, Duration::from_millis(333));
    }

    #[test]
    fn a_hold_takes_the_other_gestures_away_and_the_others_take_the_hold_away() {
        let mut controls = Controls::default();
        let pad = Button::SamplerTopLeft;
        controls.set(pad, Gesture::Short, Some(mute(Channel::Music)));
        controls.set(pad, Gesture::Long, Some(mute(Channel::Chat)));
        controls.set(pad, Gesture::Double, Some(mute(Channel::Game)));
        assert_eq!(controls.actions(pad).all().count(), 3);

        controls.set(pad, Gesture::Hold, Some(mute(Channel::Mic)));
        let actions = controls.actions(pad);
        assert_eq!(actions.hold, Some(mute(Channel::Mic)));
        assert_eq!(actions.all().count(), 1);

        controls.set(pad, Gesture::Long, Some(mute(Channel::Chat)));
        let actions = controls.actions(pad);
        assert_eq!(actions.hold, None);
        assert_eq!(actions.long, Some(mute(Channel::Chat)));

        // Taking an action away leaves the rest, and the hold if there is one.
        controls.set(pad, Gesture::Long, None);
        assert!(controls.actions(pad).is_empty());
        assert_eq!(controls.wanted(pad), Wanted::default());
    }

    #[test]
    fn only_what_can_last_can_be_held() {
        let mut controls = Controls::default();
        let button = Button::SamplerSelectA;
        controls.set(button, Gesture::Hold, Some(Action::Bank { bank: Bank::C }));
        // Refused: the short press stays.
        assert_eq!(
            controls.action(button, Gesture::Short),
            Some(Action::Bank { bank: Bank::A })
        );
        assert_eq!(controls.action(button, Gesture::Hold), None);
    }

    #[test]
    fn what_a_button_is_waited_for_follows_its_actions() {
        let mut controls = Controls::blank();
        let pad = Button::SamplerBottomLeft;
        controls.set(pad, Gesture::Long, Some(mute(Channel::Music)));
        controls.set(pad, Gesture::Double, Some(mute(Channel::Chat)));
        assert_eq!(
            controls.wanted(pad),
            Wanted {
                long: true,
                double: true,
                hold: false
            }
        );
        controls.set(pad, Gesture::Hold, Some(mute(Channel::Chat)));
        assert_eq!(
            controls.wanted(pad),
            Wanted {
                long: false,
                double: false,
                hold: true
            }
        );
    }

    #[test]
    fn one_button_or_everything_goes_back_to_what_it_was() {
        let mut controls = Controls::default();
        controls.set(
            Button::MicMute,
            Gesture::Hold,
            Some(Action::Mute {
                target: AudioTarget::Mic,
                mode: MuteMode::Mute,
            }),
        );
        controls.set(
            Button::SamplerTopLeft,
            Gesture::Long,
            Some(mute(Channel::Chat)),
        );
        controls.set_times(900, 700);

        controls.reset_button(Button::MicMute);
        assert_eq!(
            controls.actions(Button::MicMute),
            Controls::default().actions(Button::MicMute)
        );
        assert!(!controls.actions(Button::SamplerTopLeft).is_empty());
        assert_eq!(controls.long_press(), 900);

        controls.reset();
        assert_eq!(controls, Controls::default());
    }

    #[test]
    fn the_times_of_a_press_stay_in_range() {
        let mut controls = Controls::default();
        controls.set_times(0, 5000);
        assert_eq!(
            (controls.long_press(), controls.double_press()),
            (200, 1000)
        );
        controls.set_times(60000, 1);
        assert_eq!(
            (controls.long_press(), controls.double_press()),
            (2000, 150)
        );
        controls.set_times(450, 250);
        assert_eq!(controls.timing().long, Duration::from_millis(450));
        assert_eq!(controls.timing().double, Duration::from_millis(250));
    }

    #[test]
    fn a_file_is_read_back_the_same() {
        let mut controls = Controls::default();
        controls.set(
            Button::SamplerTopLeft,
            Gesture::Long,
            Some(mute(Channel::Music)),
        );
        controls.set(
            Button::SamplerTopLeft,
            Gesture::Double,
            Some(mute(Channel::Chat)),
        );
        controls.set(
            Button::Bleep,
            Gesture::Hold,
            Some(Action::Mute {
                target: AudioTarget::Mic,
                mode: MuteMode::Mute,
            }),
        );
        controls.set_times(650, 400);
        let text = toml::to_string(&controls.to_file()).unwrap();
        assert_eq!(
            Controls::from_file(toml::from_str(&text).unwrap()),
            controls,
            "{text}"
        );

        // Even a button taken away stays taken away.
        let mut empty = Controls::default();
        for button in Button::ALL {
            for gesture in Gesture::ALL {
                empty.set(button, gesture, None);
            }
        }
        assert_eq!(empty, Controls::blank());
        let text = toml::to_string(&empty.to_file()).unwrap();
        assert_eq!(
            Controls::from_file(toml::from_str(&text).unwrap()),
            Controls::blank(),
            "{text}"
        );
    }

    #[test]
    fn a_file_that_never_said_what_the_buttons_do_gives_the_starting_buttons() {
        for text in ["# Nothing to keep yet.\n", "", "longPressMs = 700\n"] {
            let controls = Controls::from_file(toml::from_str(text).unwrap());
            assert_eq!(controls.actions(Button::MicMute), starting(Button::MicMute));
            assert_eq!(
                controls.actions(Button::SamplerTopLeft),
                ButtonActions::default()
            );
        }
        let controls = Controls::from_file(toml::from_str("longPressMs = 700\n").unwrap());
        assert_eq!(controls.long_press(), 700);
        assert_eq!(controls.double_press(), 333);
    }

    #[test]
    fn a_file_written_by_hand_is_taken_with_care() {
        let text = r#"
            longPressMs = 5
            doublePressMs = 9000

            [buttons.samplerTopLeft]
            hold = { type = "mute", target = { type = "mic" }, mode = "mute" }
            short = { type = "bank", bank = "b" }

            [buttons.samplerTopRight]
            hold = { type = "bank", bank = "c" }
            long = { type = "bank", bank = "a" }

            [buttons.bleep]
        "#;
        let controls = Controls::from_file(toml::from_str(text).unwrap());
        assert_eq!(
            (controls.long_press(), controls.double_press()),
            (200, 1000)
        );
        // The hold wins.
        let left = controls.actions(Button::SamplerTopLeft);
        assert!(left.hold.is_some() && left.short.is_none());
        // What cannot be held is not: the rest stays.
        let right = controls.actions(Button::SamplerTopRight);
        assert_eq!(right.hold, None);
        assert_eq!(right.long, Some(Action::Bank { bank: Bank::A }));
        assert!(controls.actions(Button::Bleep).is_empty());
        // The buttons the file does not mention do nothing: it said what they do.
        assert!(controls.actions(Button::MicMute).is_empty());
    }

    #[test]
    fn an_action_no_version_knows_makes_the_file_unreadable() {
        let text = r#"
            [buttons.bleep]
            short = { type = "teleport" }
        "#;
        assert!(toml::from_str::<ControlsFile>(text).is_err());
        assert!(toml::from_str::<ControlsFile>("buttons = 3").is_err());
    }

    #[test]
    fn the_interface_receives_every_button() {
        let view = serde_json::to_value(Controls::default().view()).unwrap();
        assert_eq!(view["longPressMs"], 500);
        assert_eq!(view["doublePressMs"], 333);
        let buttons = view["buttons"].as_array().unwrap();
        assert_eq!(buttons.len(), 24);
        assert_eq!(buttons[0]["button"], "effectSelect1");
        let mic = buttons.iter().find(|b| b["button"] == "micMute").unwrap();
        assert_eq!(
            mic["short"],
            serde_json::json!({ "type": "mute", "target": { "type": "mic" }, "mode": "toggle" })
        );
        assert!(mic["long"].is_null());
        let fader = buttons
            .iter()
            .find(|b| b["button"] == "fader2Mute")
            .unwrap();
        assert_eq!(
            fader["short"]["target"],
            serde_json::json!({ "type": "faderTrack", "fader": "b" })
        );
    }

    fn cut(input: RoutingInput, output: RoutingOutput) -> Action {
        Action::Route {
            input,
            output,
            mode: RouteMode::Off,
        }
    }

    fn headphones(mode: VolumeMode, percent: u8) -> Action {
        Action::Volume {
            target: AudioTarget::Channel {
                channel: Channel::Headphones,
            },
            mode,
            percent,
        }
    }

    fn profile(kind: Kind, name: &str) -> Action {
        Action::Profile {
            kind,
            name: name.into(),
        }
    }

    #[test]
    fn only_what_lasts_can_be_held_routes_included() {
        let mut controls = Controls::blank();
        let pad = Button::SamplerBottomRight;
        let route = cut(RoutingInput::Music, RoutingOutput::BroadcastMix);
        controls.set(pad, Gesture::Hold, Some(route.clone()));
        assert_eq!(controls.action(pad, Gesture::Hold), Some(route));

        // A volume or a profile change lasts no longer than a press: refused,
        // and what the button had stays.
        controls.set(pad, Gesture::Short, Some(profile(Kind::Mix, "Stream")));
        controls.set(pad, Gesture::Hold, Some(headphones(VolumeMode::Down, 5)));
        controls.set(pad, Gesture::Hold, Some(profile(Kind::Mix, "Stream")));
        assert_eq!(controls.action(pad, Gesture::Hold), None);
        assert_eq!(
            controls.action(pad, Gesture::Short),
            Some(profile(Kind::Mix, "Stream"))
        );
    }

    #[test]
    fn the_new_actions_and_the_dials_are_read_back_the_same() {
        let mut controls = Controls::default();
        let pad = Button::SamplerTopLeft;
        controls.set(
            pad,
            Gesture::Short,
            Some(cut(RoutingInput::Music, RoutingOutput::BroadcastMix)),
        );
        controls.set(pad, Gesture::Long, Some(headphones(VolumeMode::Set, 40)));
        controls.set(pad, Gesture::Double, Some(profile(Kind::Profile, "Stream")));
        controls.set(
            Button::Bleep,
            Gesture::Hold,
            Some(Action::Route {
                input: RoutingInput::Chat,
                output: RoutingOutput::Headphones,
                mode: RouteMode::On,
            }),
        );
        controls.set_wheel(
            Wheel::Pitch,
            Some(WheelAction::Volume {
                target: AudioTarget::Channel {
                    channel: Channel::Headphones,
                },
                step: 3,
            }),
        );
        controls.set_wheel(
            Wheel::Echo,
            Some(WheelAction::Volume {
                target: AudioTarget::FaderTrack { fader: Fader::C },
                step: 7,
            }),
        );
        let text = toml::to_string(&controls.to_file()).unwrap();
        assert!(text.contains("[wheels.pitch]"), "{text}");
        assert!(text.contains("type = \"route\""), "{text}");
        assert_eq!(
            Controls::from_file(toml::from_str(&text).unwrap()),
            controls,
            "{text}"
        );

        // A file written before the dials existed has none.
        let text = toml::to_string(&Controls::default().to_file()).unwrap();
        assert!(!text.contains("wheels"), "{text}");
        let old = Controls::from_file(toml::from_str(&text).unwrap());
        assert_eq!(old.wheel(Wheel::Pitch), None);
    }

    #[test]
    fn a_file_written_by_hand_with_the_new_actions_is_taken_with_care() {
        let text = r#"
            [buttons.samplerTopLeft]
            short = { type = "route", input = "music", output = "broadcastMix", mode = "off" }
            long = { type = "volume", target = { type = "mic" }, mode = "up", percent = 250 }
            double = { type = "profile", kind = "mic", name = "Radio" }

            [buttons.samplerTopRight]
            hold = { type = "volume", target = { type = "mic" }, mode = "down", percent = 5 }
            short = { type = "bank", bank = "b" }

            [wheels.gender]
            type = "volume"
            target = { type = "channel", channel = "music" }
            step = 90

            [wheels.reverb]
            type = "volume"
            target = { type = "mic" }
            step = 0
        "#;
        let controls = Controls::from_file(toml::from_str(text).unwrap());
        let left = controls.actions(Button::SamplerTopLeft);
        assert_eq!(left.long, Some(headphones_mic(VolumeMode::Up, 100)));
        assert_eq!(left.double, Some(profile(Kind::Mic, "Radio")));
        // What cannot be held is no hold: the rest stays.
        let right = controls.actions(Button::SamplerTopRight);
        assert_eq!(right.hold, None);
        assert_eq!(right.short, Some(Action::Bank { bank: Bank::B }));
        // A step brought into range.
        assert_eq!(
            controls.wheel(Wheel::Gender),
            Some(WheelAction::Volume {
                target: AudioTarget::Channel {
                    channel: Channel::Music
                },
                step: WHEEL_STEP_PERCENT.2
            })
        );
        assert_eq!(
            controls.wheel(Wheel::Reverb),
            Some(WheelAction::Volume {
                target: AudioTarget::Mic,
                step: WHEEL_STEP_PERCENT.0
            })
        );
        assert_eq!(controls.wheel(Wheel::Echo), None);
    }

    fn headphones_mic(mode: VolumeMode, percent: u8) -> Action {
        Action::Volume {
            target: AudioTarget::Mic,
            mode,
            percent,
        }
    }

    #[test]
    fn an_action_or_a_mode_no_version_knows_makes_the_file_unreadable() {
        for text in [
            "[buttons.bleep]\nshort = { type = \"route\", input = \"music\", output = \"nowhere\", mode = \"off\" }",
            "[buttons.bleep]\nshort = { type = \"route\", input = \"music\", output = \"lineOut\", mode = \"sideways\" }",
            "[buttons.bleep]\nshort = { type = \"volume\", target = { type = \"mic\" }, mode = \"set\", percent = -3 }",
            "[buttons.bleep]\nshort = { type = \"profile\", kind = \"nothing\", name = \"x\" }",
            "[wheels.pitch]\ntype = \"pitch\"",
            "[wheels.spin]\ntype = \"volume\"\ntarget = { type = \"mic\" }\nstep = 3",
        ] {
            assert!(toml::from_str::<ControlsFile>(text).is_err(), "{text}");
        }
    }

    #[test]
    fn dials_are_given_a_volume_or_nothing_and_go_back_with_everything() {
        let mut controls = Controls::default();
        let volume = |step| {
            Some(WheelAction::Volume {
                target: AudioTarget::Mic,
                step,
            })
        };
        controls.set_wheel(Wheel::Pitch, volume(50));
        assert_eq!(
            controls.wheel(Wheel::Pitch),
            volume(WHEEL_STEP_PERCENT.2),
            "brought into range"
        );
        controls.set_wheel(Wheel::Pitch, None);
        assert_eq!(controls.wheel(Wheel::Pitch), None);
        assert_eq!(controls, Controls::default());

        controls.set_wheel(Wheel::Gender, volume(4));
        // One button back to what it was leaves the dials alone.
        controls.reset_button(Button::MicMute);
        assert!(controls.wheel(Wheel::Gender).is_some());
        controls.reset();
        assert_eq!(controls, Controls::default());
    }

    #[test]
    fn the_interface_receives_the_new_actions_and_every_dial() {
        let mut controls = Controls::default();
        controls.set(
            Button::SamplerTopLeft,
            Gesture::Short,
            Some(cut(RoutingInput::Music, RoutingOutput::BroadcastMix)),
        );
        controls.set(
            Button::SamplerTopLeft,
            Gesture::Long,
            Some(profile(Kind::Controls, "Live")),
        );
        controls.set_wheel(
            Wheel::Reverb,
            Some(WheelAction::Volume {
                target: AudioTarget::FaderTrack { fader: Fader::B },
                step: 4,
            }),
        );
        let view = serde_json::to_value(controls.view()).unwrap();
        let pad = view["buttons"]
            .as_array()
            .unwrap()
            .iter()
            .find(|b| b["button"] == "samplerTopLeft")
            .unwrap();
        assert_eq!(
            pad["short"],
            serde_json::json!({
                "type": "route", "input": "music", "output": "broadcastMix", "mode": "off"
            })
        );
        assert_eq!(
            pad["long"],
            serde_json::json!({ "type": "profile", "kind": "controls", "name": "Live" })
        );
        let wheels = view["wheels"].as_array().unwrap();
        assert_eq!(
            wheels
                .iter()
                .map(|w| w["wheel"].as_str().unwrap())
                .collect::<Vec<_>>(),
            ["pitch", "gender", "reverb", "echo"]
        );
        assert!(wheels[0]["action"].is_null());
        assert_eq!(
            wheels[2]["action"],
            serde_json::json!({
                "type": "volume", "target": { "type": "faderTrack", "fader": "b" }, "step": 4
            })
        );
    }
}
