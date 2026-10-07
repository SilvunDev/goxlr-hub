//! What the buttons do, against the virtual device and a clock that only
//! moves when the test says so.

use std::time::Duration;

use goxlr_hub_device::{VirtualHandle, open_virtual};
use goxlr_hub_protocol::{Button, ButtonLight, Channel, Fader};
use serde_json::json;

use crate::gestures::ManualClock;
use crate::tests::resting_lights;
use crate::{Action, AudioTarget, Bank, Gesture, Hub, Intent, MuteMode, Settings, Snapshot};

const PAD: Button = Button::SamplerTopLeft;

/// A device that has been read once, at time zero.
fn timed_hub() -> (Hub, VirtualHandle, ManualClock) {
    let (device, hands) = open_virtual().unwrap();
    let clock = ManualClock::default();
    let mut hub = Hub::connect(Box::new(device)).unwrap();
    hub.clock = Box::new(clock.clone());
    hub.poll().unwrap();
    (hub, hands, clock)
}

fn mute(target: AudioTarget, mode: MuteMode) -> Action {
    Action::Mute { target, mode }
}

fn track(channel: Channel, mode: MuteMode) -> Action {
    mute(AudioTarget::Channel { channel }, mode)
}

fn give(hub: &mut Hub, button: Button, gesture: Gesture, action: Action) {
    hub.apply(Intent::SetGesture {
        button,
        gesture,
        action: Some(action),
    })
    .unwrap();
}

fn muted(hands: &VirtualHandle, channel: Channel) -> bool {
    hands.state().muted[usize::from(channel.index())]
}

/// Lets time pass, then reads the device.
fn after(hub: &mut Hub, clock: &ManualClock, ms: u64) -> Snapshot {
    clock.advance(Duration::from_millis(ms));
    hub.poll().unwrap()
}

#[test]
fn a_pad_does_one_thing_for_a_long_press_and_another_for_a_double_press() {
    let (mut hub, hands, clock) = timed_hub();
    give(
        &mut hub,
        PAD,
        Gesture::Long,
        track(Channel::Music, MuteMode::Mute),
    );
    give(
        &mut hub,
        PAD,
        Gesture::Double,
        track(Channel::Chat, MuteMode::Mute),
    );

    // Held for a while: the music goes quiet, the chat does not.
    hands.press(PAD);
    after(&mut hub, &clock, 100);
    assert!(!muted(&hands, Channel::Music));
    after(&mut hub, &clock, 500);
    assert!(muted(&hands, Channel::Music) && !muted(&hands, Channel::Chat));
    hands.release(PAD);
    after(&mut hub, &clock, 50);
    // Letting go says nothing more.
    after(&mut hub, &clock, 1000);
    assert!(!muted(&hands, Channel::Chat));

    // Two quick presses: the chat goes quiet.
    hands.press(PAD);
    after(&mut hub, &clock, 50);
    hands.release(PAD);
    after(&mut hub, &clock, 100);
    hands.press(PAD);
    after(&mut hub, &clock, 50);
    assert!(muted(&hands, Channel::Chat));
    hands.release(PAD);
    after(&mut hub, &clock, 50);

    // One quick press has no action: nothing happens, and nothing waits.
    let before = hands.state();
    hands.press(PAD);
    after(&mut hub, &clock, 50);
    hands.release(PAD);
    after(&mut hub, &clock, 1000);
    assert_eq!(hands.state().muted, before.muted);
}

#[test]
fn a_short_press_waits_for_a_second_one_only_when_there_is_a_double_press() {
    let (mut hub, hands, clock) = timed_hub();
    give(
        &mut hub,
        PAD,
        Gesture::Short,
        track(Channel::Game, MuteMode::Toggle),
    );

    // Alone, it goes as the finger lands.
    hands.press(PAD);
    after(&mut hub, &clock, 50);
    assert!(muted(&hands, Channel::Game));
    hands.release(PAD);
    after(&mut hub, &clock, 50);

    // With a double press on the same button, it goes a third of a second
    // after the finger left.
    give(
        &mut hub,
        PAD,
        Gesture::Double,
        track(Channel::Chat, MuteMode::Toggle),
    );
    hands.press(PAD);
    after(&mut hub, &clock, 50);
    hands.release(PAD);
    after(&mut hub, &clock, 100);
    assert!(muted(&hands, Channel::Game), "still waiting");
    after(&mut hub, &clock, 333);
    assert!(!muted(&hands, Channel::Game), "toggled back");
    assert!(!muted(&hands, Channel::Chat));
}

#[test]
fn the_microphone_is_off_for_as_long_as_its_button_is_held() {
    let (mut hub, hands, clock) = timed_hub();
    let off = |hands: &VirtualHandle| hands.state().mic_input_muted;
    give(
        &mut hub,
        Button::MicMute,
        Gesture::Hold,
        mute(AudioTarget::Mic, MuteMode::Mute),
    );
    // A hold takes the short press the button had.
    assert!(
        hub.controls()
            .action(Button::MicMute, Gesture::Short)
            .is_none()
    );

    hands.press(Button::MicMute);
    let snapshot = after(&mut hub, &clock, 50);
    assert!(snapshot.mic_off && off(&hands));
    // However long it lasts.
    assert!(after(&mut hub, &clock, 20_000).mic_off);
    hands.release(Button::MicMute);
    let snapshot = after(&mut hub, &clock, 50);
    assert!(!snapshot.mic_off && !off(&hands));
}

#[test]
fn a_hold_puts_back_what_it_found() {
    let (mut hub, hands, clock) = timed_hub();
    give(
        &mut hub,
        Button::MicMute,
        Gesture::Hold,
        mute(AudioTarget::Mic, MuteMode::Mute),
    );
    hub.apply(Intent::SetMicOff { off: true }).unwrap();

    // Already off: it stays off after the hold.
    hands.press(Button::MicMute);
    assert!(after(&mut hub, &clock, 50).mic_off);
    hands.release(Button::MicMute);
    assert!(after(&mut hub, &clock, 50).mic_off);

    // "Open while held", on a track that was muted: it closes again.
    give(
        &mut hub,
        PAD,
        Gesture::Hold,
        track(Channel::Chat, MuteMode::Unmute),
    );
    hub.apply(Intent::SetMuted {
        channel: Channel::Chat,
        muted: true,
    })
    .unwrap();
    hands.press(PAD);
    after(&mut hub, &clock, 50);
    assert!(!muted(&hands, Channel::Chat));
    hands.release(PAD);
    after(&mut hub, &clock, 50);
    assert!(muted(&hands, Channel::Chat));
}

#[test]
fn a_hold_ends_with_what_it_began_with_whatever_was_loaded_meanwhile() {
    let (mut hub, hands, clock) = timed_hub();
    give(
        &mut hub,
        Button::MicMute,
        Gesture::Hold,
        mute(AudioTarget::Mic, MuteMode::Mute),
    );
    hands.press(Button::MicMute);
    assert!(after(&mut hub, &clock, 50).mic_off);

    // Another profile, whose microphone button does something else.
    let mut settings = hub.settings();
    settings.controls = crate::Controls::default();
    hub.load(settings).unwrap();
    assert!(after(&mut hub, &clock, 50).mic_off);

    hands.release(Button::MicMute);
    assert!(!after(&mut hub, &clock, 50).mic_off);
    // The new profile's button works from the next press.
    hands.press(Button::MicMute);
    assert!(after(&mut hub, &clock, 50).mic_off);
}

#[test]
fn a_fader_button_follows_the_track_under_its_fader() {
    let (mut hub, hands, clock) = timed_hub();
    hub.apply(Intent::AssignFader {
        fader: Fader::A,
        channel: Channel::Game,
    })
    .unwrap();
    hands.press(Button::Fader1Mute);
    after(&mut hub, &clock, 50);
    assert!(muted(&hands, Channel::Game));
    assert!(!muted(&hands, Channel::Mic) && !hands.state().mic_input_muted);
}

#[test]
fn a_button_is_lit_when_what_one_of_its_actions_does_is_in_force() {
    let (mut hub, hands, _clock) = timed_hub();
    // Nothing muted, nothing to light, but the bank the pads are on.
    assert_eq!(hands.state().lights, resting_lights());

    give(
        &mut hub,
        PAD,
        Gesture::Long,
        track(Channel::Music, MuteMode::Mute),
    );
    give(
        &mut hub,
        PAD,
        Gesture::Double,
        track(Channel::Chat, MuteMode::Unmute),
    );
    let lit = |hands: &VirtualHandle, button| hands.state().lights.get(button) == ButtonLight::Lit;
    // Chat is open, and the pad opens it: in force. Music is open: not.
    assert!(lit(&hands, PAD));

    hub.apply(Intent::SetMuted {
        channel: Channel::Chat,
        muted: true,
    })
    .unwrap();
    assert!(!lit(&hands, PAD));
    hub.apply(Intent::SetMuted {
        channel: Channel::Music,
        muted: true,
    })
    .unwrap();
    assert!(lit(&hands, PAD), "the music is muted, as the pad does");

    // A button with nothing to do stays in standby, whatever happens.
    assert!(!lit(&hands, Button::Bleep));
    assert!(!lit(&hands, Button::EffectFx));

    // Taking the action away takes the light away.
    hub.apply(Intent::ResetControls { button: Some(PAD) })
        .unwrap();
    assert!(!lit(&hands, PAD));
}

#[test]
fn the_pads_are_on_one_bank_at_a_time() {
    let (mut hub, hands, clock) = timed_hub();
    let lit = |hands: &VirtualHandle| {
        [
            Button::SamplerSelectA,
            Button::SamplerSelectB,
            Button::SamplerSelectC,
        ]
        .map(|button| hands.state().lights.get(button) == ButtonLight::Lit)
    };
    assert_eq!(hub.poll().unwrap().bank, Bank::A);
    assert_eq!(lit(&hands), [true, false, false]);

    hands.press(Button::SamplerSelectC);
    assert_eq!(after(&mut hub, &clock, 50).bank, Bank::C);
    assert_eq!(lit(&hands), [false, false, true]);
    hands.release(Button::SamplerSelectC);
    hands.press(Button::SamplerSelectB);
    assert_eq!(after(&mut hub, &clock, 50).bank, Bank::B);
    assert_eq!(lit(&hands), [false, true, false]);

    // A device that comes back is brought back to the bank it was on.
    let settings = hub.settings();
    let (device, hands) = open_virtual().unwrap();
    let mut again = Hub::adopt(Box::new(device), Some(&settings)).unwrap();
    assert_eq!(again.poll().unwrap().bank, Bank::B);
    assert_eq!(lit(&hands), [false, true, false]);
}

#[test]
fn a_button_stays_lit_on_screen_a_moment_after_a_short_press() {
    let (mut hub, hands, clock) = timed_hub();
    hands.press(PAD);
    hands.release(PAD);
    // The press was over before the reading, and still shows.
    let snapshot = after(&mut hub, &clock, 50);
    assert!(snapshot.touched.contains(&PAD));
    assert!(after(&mut hub, &clock, 500).touched.contains(&PAD));
    assert!(!after(&mut hub, &clock, 100).touched.contains(&PAD));

    // Held, it stays as long as it is held.
    hands.press(PAD);
    after(&mut hub, &clock, 50);
    let snapshot = after(&mut hub, &clock, 5000);
    assert!(snapshot.touched.contains(&PAD) && snapshot.pressed.contains(&PAD));
    hands.release(PAD);
    assert!(after(&mut hub, &clock, 100).touched.contains(&PAD));
    assert!(after(&mut hub, &clock, 400).touched.contains(&PAD));
    assert!(after(&mut hub, &clock, 200).touched.is_empty());
}

#[test]
fn the_last_button_pressed_is_told_with_a_count_that_changes_at_every_press() {
    let (mut hub, hands, clock) = timed_hub();
    assert_eq!(hub.poll().unwrap().last_press, None);

    hands.press(PAD);
    let first = after(&mut hub, &clock, 50).last_press.unwrap();
    assert_eq!(first.button, PAD);
    // Held: still the same press.
    assert_eq!(after(&mut hub, &clock, 50).last_press, Some(first));
    hands.release(PAD);
    assert_eq!(after(&mut hub, &clock, 50).last_press, Some(first));

    // The same button again is another press, even a very short one.
    hands.press(PAD);
    hands.release(PAD);
    let second = after(&mut hub, &clock, 50).last_press.unwrap();
    assert_eq!(second.button, PAD);
    assert_ne!(second.count, first.count);

    hands.press(Button::Bleep);
    let third = after(&mut hub, &clock, 50).last_press.unwrap();
    assert_eq!(third.button, Button::Bleep);
    assert_ne!(third.count, second.count);
}

#[test]
fn a_button_held_when_the_device_is_taken_is_no_press_even_for_the_screen() {
    let (device, hands) = open_virtual().unwrap();
    hands.press(PAD);
    let mut hub = Hub::connect(Box::new(device)).unwrap();
    let snapshot = hub.poll().unwrap();
    assert_eq!(snapshot.last_press, None);
    assert_eq!(snapshot.pressed, [PAD]);
}

#[test]
fn the_time_of_a_long_press_is_the_one_that_was_set() {
    let (mut hub, hands, clock) = timed_hub();
    give(
        &mut hub,
        PAD,
        Gesture::Long,
        track(Channel::Music, MuteMode::Mute),
    );
    hub.apply(Intent::SetPressTimes {
        long_press_ms: 300,
        double_press_ms: 200,
    })
    .unwrap();
    assert_eq!(hub.controls().long_press(), 300);

    hands.press(PAD);
    after(&mut hub, &clock, 10);
    after(&mut hub, &clock, 250);
    assert!(!muted(&hands, Channel::Music));
    after(&mut hub, &clock, 100);
    assert!(muted(&hands, Channel::Music));

    // Out of range, it is brought back into range.
    hub.apply(Intent::SetPressTimes {
        long_press_ms: 5,
        double_press_ms: 60_000,
    })
    .unwrap();
    assert_eq!(
        (hub.controls().long_press(), hub.controls().double_press()),
        (200, 1000)
    );
}

#[test]
fn controls_go_back_to_what_they_were_one_button_or_all() {
    let (mut hub, _hands, _clock) = timed_hub();
    give(
        &mut hub,
        Button::MicMute,
        Gesture::Hold,
        mute(AudioTarget::Mic, MuteMode::Mute),
    );
    give(
        &mut hub,
        PAD,
        Gesture::Long,
        track(Channel::Chat, MuteMode::Mute),
    );
    hub.apply(Intent::SetPressTimes {
        long_press_ms: 900,
        double_press_ms: 500,
    })
    .unwrap();

    hub.apply(Intent::ResetControls {
        button: Some(Button::MicMute),
    })
    .unwrap();
    let start = crate::Controls::default();
    assert_eq!(
        hub.controls().actions(Button::MicMute),
        start.actions(Button::MicMute)
    );
    assert!(hub.controls().action(PAD, Gesture::Long).is_some());
    assert_eq!(hub.controls().long_press(), 900);

    hub.apply(Intent::ResetControls { button: None }).unwrap();
    assert_eq!(hub.controls(), &start);
}

#[test]
fn what_the_buttons_do_is_part_of_the_settings_a_device_is_brought_to() {
    let (mut hub, _hands, _clock) = timed_hub();
    give(
        &mut hub,
        PAD,
        Gesture::Long,
        track(Channel::Chat, MuteMode::Mute),
    );
    let settings = hub.settings();
    assert!(settings.controls.action(PAD, Gesture::Long).is_some());

    let (device, hands) = open_virtual().unwrap();
    let mut other = Hub::adopt(Box::new(device), Some(&settings)).unwrap();
    assert_eq!(other.controls(), &settings.controls);
    let clock = ManualClock::default();
    other.clock = Box::new(clock.clone());
    other.poll().unwrap();
    hands.press(PAD);
    other.poll().unwrap();
    clock.advance(Duration::from_millis(600));
    other.poll().unwrap();
    assert!(hands.state().muted[usize::from(Channel::Chat.index())]);

    // A device met for the first time has the starting controls.
    assert_eq!(Settings::unknown().controls, crate::Controls::default());
}

#[test]
fn the_interface_asks_in_the_shape_the_core_expects() {
    let intent = |value: serde_json::Value| serde_json::from_value::<Intent>(value).unwrap();
    assert_eq!(
        intent(json!({
            "type": "setGesture",
            "button": "samplerTopLeft",
            "gesture": "long",
            "action": {
                "type": "mute",
                "target": { "type": "channel", "channel": "music" },
                "mode": "toggle"
            }
        })),
        Intent::SetGesture {
            button: PAD,
            gesture: Gesture::Long,
            action: Some(track(Channel::Music, MuteMode::Toggle)),
        }
    );
    assert_eq!(
        intent(json!({
            "type": "setGesture",
            "button": "fader1Mute",
            "gesture": "hold",
            "action": {
                "type": "mute",
                "target": { "type": "faderTrack", "fader": "c" },
                "mode": "mute"
            }
        })),
        Intent::SetGesture {
            button: Button::Fader1Mute,
            gesture: Gesture::Hold,
            action: Some(mute(
                AudioTarget::FaderTrack { fader: Fader::C },
                MuteMode::Mute
            )),
        }
    );
    assert_eq!(
        intent(json!({
            "type": "setGesture", "button": "bleep", "gesture": "double", "action": null
        })),
        Intent::SetGesture {
            button: Button::Bleep,
            gesture: Gesture::Double,
            action: None,
        }
    );
    assert_eq!(
        intent(json!({ "type": "resetControls", "button": null })),
        Intent::ResetControls { button: None }
    );
    assert_eq!(
        intent(json!({ "type": "resetControls", "button": "micMute" })),
        Intent::ResetControls {
            button: Some(Button::MicMute)
        }
    );
    assert_eq!(
        intent(json!({ "type": "setPressTimes", "longPressMs": 600, "doublePressMs": 250 })),
        Intent::SetPressTimes {
            long_press_ms: 600,
            double_press_ms: 250
        }
    );
    assert_eq!(
        intent(json!({ "type": "pressButton", "button": "samplerClear", "down": true })),
        Intent::PressButton {
            button: Button::SamplerClear,
            down: true
        }
    );
    assert!(
        serde_json::from_value::<Intent>(json!({
            "type": "setGesture", "button": "nowhere", "gesture": "long", "action": null
        }))
        .is_err()
    );
}

#[test]
fn the_snapshot_tells_the_controls_what_was_touched_and_the_bank() {
    let (mut hub, hands, clock) = timed_hub();
    hands.press(Button::SamplerSelectB);
    let value = serde_json::to_value(after(&mut hub, &clock, 50)).unwrap();
    assert_eq!(value["bank"], "b");
    assert_eq!(value["touched"], json!(["samplerSelectB"]));
    assert_eq!(value["lastPress"]["button"], "samplerSelectB");
    assert!(value["lastPress"]["count"].is_u64());
    assert_eq!(value["controls"]["longPressMs"], 500);
    assert_eq!(value["controls"]["buttons"].as_array().unwrap().len(), 24);

    let (mut fresh, _hands, _clock) = timed_hub();
    let value = serde_json::to_value(fresh.poll().unwrap()).unwrap();
    assert!(value["lastPress"].is_null());
    assert_eq!(value["touched"], json!([]));
    assert_eq!(value["bank"], "a");
}
