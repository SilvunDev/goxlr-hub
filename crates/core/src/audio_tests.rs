//! Routes, volumes, dials and profiles asked for by buttons, against the
//! virtual device and a clock that only moves when the test says so.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use goxlr_hub_device::{DeviceKind, Session, VirtualGoXlr, VirtualHandle, open_virtual};
use goxlr_hub_protocol::{
    Button, ButtonLight, Channel, Fader, OutputSet, RoutingInput, RoutingOutput, Side, Wheel,
};
use serde_json::json;

use crate::controls_tests::{Deaf, OTHER_PAD, PAD, after, give, muted, timed_hub, track};
use crate::gestures::ManualClock;
use crate::{
    Action, AudioTarget, Gesture, Hub, Intent, Kind, Load, MuteMode, RouteMode, Settings,
    VolumeMode, WHEEL_STEP_PERCENT, WheelAction, default_routing,
};

const MUSIC: RoutingInput = RoutingInput::Music;
const STREAM: RoutingOutput = RoutingOutput::BroadcastMix;

fn route(input: RoutingInput, output: RoutingOutput, mode: RouteMode) -> Action {
    Action::Route {
        input,
        output,
        mode,
    }
}

fn music_to_stream(mode: RouteMode) -> Action {
    route(MUSIC, STREAM, mode)
}

/// The outputs an input feeds on the device, the same on both sides.
fn routed(hands: &VirtualHandle, input: RoutingInput) -> OutputSet {
    let device = hands.state();
    let left = device.routed(input, Side::Left);
    assert_eq!(left, device.routed(input, Side::Right), "{input:?}");
    left
}

fn volume_of(hands: &VirtualHandle, channel: Channel) -> u8 {
    hands.state().volumes[usize::from(channel.index())]
}

fn volume(channel: Channel, mode: VolumeMode, percent: u8) -> Action {
    Action::Volume {
        target: AudioTarget::Channel { channel },
        mode,
        percent,
    }
}

fn set_headphones(hub: &mut Hub, volume: u8) {
    hub.apply(Intent::SetVolume {
        channel: Channel::Headphones,
        volume,
    })
    .unwrap();
}

/// Presses a button for the time of one reading, then lets it go.
fn tap(hub: &mut Hub, hands: &VirtualHandle, clock: &ManualClock, button: Button) {
    hands.press(button);
    after(hub, clock, 50);
    hands.release(button);
    after(hub, clock, 50);
}

#[test]
fn a_track_is_cut_toward_one_output_and_the_others_are_left_alone() {
    let (mut hub, hands, clock) = timed_hub();
    give(
        &mut hub,
        PAD,
        Gesture::Short,
        music_to_stream(RouteMode::Off),
    );
    assert!(routed(&hands, MUSIC).contains(STREAM));

    tap(&mut hub, &hands, &clock, PAD);
    let music = routed(&hands, MUSIC);
    assert!(!music.contains(STREAM));
    assert!(music.contains(RoutingOutput::Headphones) && music.contains(RoutingOutput::LineOut));
    assert_eq!(
        routed(&hands, RoutingInput::Game),
        default_routing()[RoutingInput::Game as usize]
    );
    assert!(!hub.mixer().routing[MUSIC as usize].contains(STREAM));

    // Cutting what is cut changes nothing.
    tap(&mut hub, &hands, &clock, PAD);
    assert!(!routed(&hands, MUSIC).contains(STREAM));
}

#[test]
fn a_cell_of_the_grid_is_ticked_unticked_or_toggled() {
    let (mut hub, hands, clock) = timed_hub();
    give(
        &mut hub,
        PAD,
        Gesture::Short,
        route(RoutingInput::Mic, RoutingOutput::Headphones, RouteMode::On),
    );
    give(
        &mut hub,
        OTHER_PAD,
        Gesture::Short,
        route(RoutingInput::Mic, RoutingOutput::ChatMic, RouteMode::Toggle),
    );
    assert!(!routed(&hands, RoutingInput::Mic).contains(RoutingOutput::Headphones));

    tap(&mut hub, &hands, &clock, PAD);
    assert!(routed(&hands, RoutingInput::Mic).contains(RoutingOutput::Headphones));
    tap(&mut hub, &hands, &clock, PAD);
    assert!(routed(&hands, RoutingInput::Mic).contains(RoutingOutput::Headphones));

    assert!(routed(&hands, RoutingInput::Mic).contains(RoutingOutput::ChatMic));
    tap(&mut hub, &hands, &clock, OTHER_PAD);
    assert!(!routed(&hands, RoutingInput::Mic).contains(RoutingOutput::ChatMic));
    tap(&mut hub, &hands, &clock, OTHER_PAD);
    assert!(routed(&hands, RoutingInput::Mic).contains(RoutingOutput::ChatMic));
}

#[test]
fn a_route_that_feeds_a_sound_back_to_where_it_comes_from_does_nothing() {
    let (mut hub, hands, clock) = timed_hub();
    give(
        &mut hub,
        PAD,
        Gesture::Hold,
        route(RoutingInput::Chat, RoutingOutput::ChatMic, RouteMode::On),
    );
    give(
        &mut hub,
        OTHER_PAD,
        Gesture::Short,
        route(
            RoutingInput::Samples,
            RoutingOutput::Sampler,
            RouteMode::Toggle,
        ),
    );
    let before = hands.state();
    hands.press(PAD);
    after(&mut hub, &clock, 50);
    hands.release(PAD);
    after(&mut hub, &clock, 50);
    tap(&mut hub, &hands, &clock, OTHER_PAD);
    assert_eq!(hands.state().routing, before.routing);
    assert_eq!(hub.mixer().routing, default_routing());
    assert_eq!(hands.state().lights.get(OTHER_PAD), ButtonLight::Dimmed);
}

#[test]
fn a_button_that_cuts_a_cell_is_lit_for_as_long_as_the_cell_is_cut() {
    let (mut hub, hands, clock) = timed_hub();
    give(
        &mut hub,
        PAD,
        Gesture::Short,
        music_to_stream(RouteMode::Toggle),
    );
    give(
        &mut hub,
        OTHER_PAD,
        Gesture::Short,
        route(RoutingInput::Game, RoutingOutput::LineOut, RouteMode::On),
    );
    let lit = |hands: &VirtualHandle, button| hands.state().lights.get(button) == ButtonLight::Lit;
    // The cell is ticked: the button that cuts it is dark, the one that
    // ticks it is lit.
    assert!(!lit(&hands, PAD) && lit(&hands, OTHER_PAD));

    tap(&mut hub, &hands, &clock, PAD);
    assert!(lit(&hands, PAD));
    tap(&mut hub, &hands, &clock, PAD);
    assert!(!lit(&hands, PAD));
}

#[test]
fn a_cut_held_comes_back_with_the_finger() {
    let (mut hub, hands, clock) = timed_hub();
    give(
        &mut hub,
        PAD,
        Gesture::Hold,
        music_to_stream(RouteMode::Off),
    );

    hands.press(PAD);
    after(&mut hub, &clock, 50);
    assert!(!routed(&hands, MUSIC).contains(STREAM));
    after(&mut hub, &clock, 20_000);
    assert!(!routed(&hands, MUSIC).contains(STREAM));
    hands.release(PAD);
    after(&mut hub, &clock, 50);
    assert!(routed(&hands, MUSIC).contains(STREAM));
    assert_eq!(hub.mixer().routing, default_routing());

    // A cell that was cut already stays cut.
    hub.apply(Intent::SetRoute {
        input: MUSIC,
        output: STREAM,
        on: false,
    })
    .unwrap();
    hands.press(PAD);
    after(&mut hub, &clock, 50);
    hands.release(PAD);
    after(&mut hub, &clock, 50);
    assert!(!routed(&hands, MUSIC).contains(STREAM));
}

#[test]
fn a_cell_ticked_while_held_is_unticked_again_when_it_was_not_before() {
    let (mut hub, hands, clock) = timed_hub();
    give(
        &mut hub,
        PAD,
        Gesture::Hold,
        route(RoutingInput::Mic, RoutingOutput::Headphones, RouteMode::On),
    );
    hands.press(PAD);
    after(&mut hub, &clock, 50);
    assert!(routed(&hands, RoutingInput::Mic).contains(RoutingOutput::Headphones));
    hands.release(PAD);
    after(&mut hub, &clock, 50);
    assert!(!routed(&hands, RoutingInput::Mic).contains(RoutingOutput::Headphones));
    assert_eq!(hub.mixer().routing, default_routing());
}

#[test]
fn two_holds_on_one_cell_put_back_what_the_first_one_found() {
    let (mut hub, hands, clock) = timed_hub();
    give(
        &mut hub,
        PAD,
        Gesture::Hold,
        music_to_stream(RouteMode::Off),
    );
    give(
        &mut hub,
        OTHER_PAD,
        Gesture::Hold,
        music_to_stream(RouteMode::Off),
    );

    hands.press(PAD);
    after(&mut hub, &clock, 50);
    hands.press(OTHER_PAD);
    after(&mut hub, &clock, 50);
    hands.release(PAD);
    after(&mut hub, &clock, 50);
    assert!(
        !routed(&hands, MUSIC).contains(STREAM),
        "one finger is left"
    );
    hands.release(OTHER_PAD);
    after(&mut hub, &clock, 50);
    assert!(routed(&hands, MUSIC).contains(STREAM));
}

#[test]
fn a_cut_and_a_mute_on_the_same_track_do_not_put_each_other_back() {
    let (mut hub, hands, clock) = timed_hub();
    give(
        &mut hub,
        PAD,
        Gesture::Hold,
        music_to_stream(RouteMode::Off),
    );
    give(
        &mut hub,
        OTHER_PAD,
        Gesture::Hold,
        track(Channel::Music, MuteMode::Mute),
    );

    hands.press(PAD);
    after(&mut hub, &clock, 50);
    hands.press(OTHER_PAD);
    after(&mut hub, &clock, 50);
    hands.release(PAD);
    after(&mut hub, &clock, 50);
    assert!(routed(&hands, MUSIC).contains(STREAM));
    assert!(muted(&hands, Channel::Music), "the mute is still held");
    hands.release(OTHER_PAD);
    after(&mut hub, &clock, 50);
    assert!(!muted(&hands, Channel::Music));
}

#[test]
fn holds_on_a_cell_are_all_let_go_with_the_device() {
    let (mut hub, hands, clock) = timed_hub();
    give(
        &mut hub,
        PAD,
        Gesture::Hold,
        music_to_stream(RouteMode::Off),
    );
    hands.press(PAD);
    after(&mut hub, &clock, 50);
    assert!(!hub.mixer().routing[MUSIC as usize].contains(STREAM));

    hub.release_holds();
    assert_eq!(hub.settings().mixer.routing, default_routing());
}

#[test]
fn a_cut_the_device_does_not_hear_end_still_puts_the_mixer_back() {
    let (inner, hands) = VirtualGoXlr::new();
    let deaf = Arc::new(AtomicBool::new(false));
    let link = Deaf {
        inner,
        deaf: deaf.clone(),
    };
    let device = Session::open(link, DeviceKind::Hardware).unwrap();
    let mut hub = Hub::connect(Box::new(device)).unwrap();
    let clock = ManualClock::default();
    hub.clock = Box::new(clock.clone());
    hub.poll().unwrap();
    give(
        &mut hub,
        PAD,
        Gesture::Hold,
        music_to_stream(RouteMode::Off),
    );

    hands.press(PAD);
    after(&mut hub, &clock, 50);
    assert!(!routed(&hands, MUSIC).contains(STREAM));
    deaf.store(true, Ordering::Relaxed);
    hands.release(PAD);
    clock.advance(Duration::from_millis(50));
    assert!(hub.poll().is_err(), "the device did not hear the end");

    // The mixer is what the app sends the device when it is taken again.
    assert_eq!(hub.settings().mixer.routing, default_routing());
}

#[test]
fn a_routing_loaded_during_a_hold_on_a_cell_wins_over_the_hold() {
    let (mut hub, hands, clock) = timed_hub();
    give(
        &mut hub,
        PAD,
        Gesture::Hold,
        music_to_stream(RouteMode::Off),
    );
    hands.press(PAD);
    after(&mut hub, &clock, 50);

    // A profile whose music goes to the line output only.
    let mut settings = hub.settings();
    settings.mixer.routing[MUSIC as usize] = OutputSet::of(&[RoutingOutput::LineOut]);
    let loaded = settings.mixer.routing;
    hub.load(settings).unwrap();
    assert_eq!(routed(&hands, MUSIC), loaded[MUSIC as usize]);

    // The finger leaves: the profile's routing is not touched.
    hands.release(PAD);
    after(&mut hub, &clock, 50);
    assert_eq!(hub.mixer().routing, loaded);
    assert_eq!(routed(&hands, MUSIC), loaded[MUSIC as usize]);
}

#[test]
fn a_load_that_leaves_the_routing_as_it_is_leaves_the_hold_too() {
    let (mut hub, hands, clock) = timed_hub();
    give(
        &mut hub,
        PAD,
        Gesture::Hold,
        music_to_stream(RouteMode::Off),
    );
    hands.press(PAD);
    after(&mut hub, &clock, 50);

    // Another piece of the profile: the routing in use goes with it.
    let settings = hub.settings();
    hub.load(settings).unwrap();
    assert!(!routed(&hands, MUSIC).contains(STREAM));
    hands.release(PAD);
    after(&mut hub, &clock, 50);
    assert!(routed(&hands, MUSIC).contains(STREAM));
}

#[test]
fn a_volume_is_set_raised_or_lowered_in_percent() {
    let (mut hub, hands, clock) = timed_hub();
    let headphones = |mode, percent| volume(Channel::Headphones, mode, percent);
    give(
        &mut hub,
        PAD,
        Gesture::Short,
        headphones(VolumeMode::Set, 40),
    );
    give(
        &mut hub,
        OTHER_PAD,
        Gesture::Short,
        headphones(VolumeMode::Up, 10),
    );
    give(
        &mut hub,
        Button::Bleep,
        Gesture::Short,
        headphones(VolumeMode::Down, 10),
    );

    tap(&mut hub, &hands, &clock, PAD);
    assert_eq!(volume_of(&hands, Channel::Headphones), 102);
    tap(&mut hub, &hands, &clock, OTHER_PAD);
    assert_eq!(volume_of(&hands, Channel::Headphones), 128);
    tap(&mut hub, &hands, &clock, Button::Bleep);
    tap(&mut hub, &hands, &clock, Button::Bleep);
    assert_eq!(volume_of(&hands, Channel::Headphones), 76);
    assert_eq!(
        hub.mixer().volumes[usize::from(Channel::Headphones.index())],
        Some(76)
    );
}

#[test]
fn a_volume_stops_at_the_ends_of_its_range() {
    let (mut hub, hands, clock) = timed_hub();
    let headphones = |mode, percent| volume(Channel::Headphones, mode, percent);
    give(
        &mut hub,
        PAD,
        Gesture::Short,
        headphones(VolumeMode::Up, 100),
    );
    give(
        &mut hub,
        OTHER_PAD,
        Gesture::Short,
        headphones(VolumeMode::Down, 100),
    );
    set_headphones(&mut hub, 200);
    tap(&mut hub, &hands, &clock, PAD);
    assert_eq!(volume_of(&hands, Channel::Headphones), 255);
    tap(&mut hub, &hands, &clock, OTHER_PAD);
    assert_eq!(volume_of(&hands, Channel::Headphones), 0);
    tap(&mut hub, &hands, &clock, OTHER_PAD);
    assert_eq!(volume_of(&hands, Channel::Headphones), 0);

    // Out of range in a file or an intent, it is a whole range at most.
    give(
        &mut hub,
        PAD,
        Gesture::Long,
        headphones(VolumeMode::Set, 250),
    );
    assert_eq!(
        hub.controls().action(PAD, Gesture::Long),
        Some(headphones(VolumeMode::Set, 100))
    );
}

#[test]
fn a_volume_of_the_track_under_a_fader_follows_the_fader() {
    let (mut hub, hands, clock) = timed_hub();
    give(
        &mut hub,
        PAD,
        Gesture::Short,
        Action::Volume {
            target: AudioTarget::FaderTrack { fader: Fader::C },
            mode: VolumeMode::Set,
            percent: 20,
        },
    );
    tap(&mut hub, &hands, &clock, PAD);
    assert_eq!(volume_of(&hands, Channel::Music), 51);

    hub.apply(Intent::AssignFader {
        fader: Fader::C,
        channel: Channel::Game,
    })
    .unwrap();
    tap(&mut hub, &hands, &clock, PAD);
    assert_eq!(volume_of(&hands, Channel::Game), 51);
    assert_eq!(hub.poll().unwrap().faders[2].volume, 51);
}

/// A real device the app has not set any volume of yet.
fn unknown_hub() -> (Hub, VirtualHandle, ManualClock) {
    let (device, hands) = open_virtual().unwrap();
    let clock = ManualClock::default();
    let mut hub = Hub::adopt(Box::new(device), None).unwrap();
    hub.clock = Box::new(clock.clone());
    hub.poll().unwrap();
    (hub, hands, clock)
}

#[test]
fn a_volume_the_app_does_not_know_is_left_alone_by_raising_and_lowering_it() {
    let (mut hub, hands, clock) = unknown_hub();
    let index = usize::from(Channel::Headphones.index());
    let headphones = |mode, percent| volume(Channel::Headphones, mode, percent);
    assert_eq!(hub.mixer().volumes[index], None);
    give(
        &mut hub,
        PAD,
        Gesture::Short,
        headphones(VolumeMode::Down, 10),
    );
    give(
        &mut hub,
        OTHER_PAD,
        Gesture::Short,
        headphones(VolumeMode::Up, 10),
    );
    give(
        &mut hub,
        Button::Bleep,
        Gesture::Short,
        headphones(VolumeMode::Set, 40),
    );
    let before = volume_of(&hands, Channel::Headphones);

    // A guess could be louder than what the headphones are at.
    tap(&mut hub, &hands, &clock, PAD);
    tap(&mut hub, &hands, &clock, OTHER_PAD);
    assert_eq!(hub.mixer().volumes[index], None);
    assert_eq!(volume_of(&hands, Channel::Headphones), before);

    // Set once, it is known, and the others follow from there.
    tap(&mut hub, &hands, &clock, Button::Bleep);
    assert_eq!(hub.mixer().volumes[index], Some(102));
    tap(&mut hub, &hands, &clock, PAD);
    assert_eq!(volume_of(&hands, Channel::Headphones), 76);
}

fn wheel_to(channel: Channel, step: u8) -> Intent {
    Intent::SetWheel {
        wheel: Wheel::Pitch,
        action: Some(WheelAction::Volume {
            target: AudioTarget::Channel { channel },
            step,
        }),
    }
}

#[test]
fn a_dial_turned_sets_the_volume_it_was_given_notch_by_notch() {
    let (mut hub, hands, clock) = timed_hub();
    set_headphones(&mut hub, 100);
    hub.apply(wheel_to(Channel::Headphones, 4)).unwrap();

    hands.turn(Wheel::Pitch, 3);
    after(&mut hub, &clock, 50);
    assert_eq!(volume_of(&hands, Channel::Headphones), 131);
    hands.turn(Wheel::Pitch, -5);
    after(&mut hub, &clock, 50);
    assert_eq!(volume_of(&hands, Channel::Headphones), 80);
    // Still: nothing more.
    after(&mut hub, &clock, 50);
    assert_eq!(volume_of(&hands, Channel::Headphones), 80);
    assert_eq!(
        hub.mixer().volumes[usize::from(Channel::Headphones.index())],
        Some(80)
    );
}

#[test]
fn a_dial_is_followed_from_where_it_was_first_seen_and_never_by_where_it_is() {
    let (device, hands) = open_virtual().unwrap();
    // The dial is far from zero when the app meets it.
    hands.turn(Wheel::Pitch, 17);
    let clock = ManualClock::default();
    let mut hub = Hub::connect(Box::new(device)).unwrap();
    hub.clock = Box::new(clock.clone());
    set_headphones(&mut hub, 100);
    hub.apply(wheel_to(Channel::Headphones, 4)).unwrap();

    hub.poll().unwrap();
    assert_eq!(volume_of(&hands, Channel::Headphones), 100);
    hands.turn(Wheel::Pitch, 1);
    after(&mut hub, &clock, 50);
    assert_eq!(volume_of(&hands, Channel::Headphones), 110);
}

#[test]
fn a_dial_at_the_end_of_its_travel_stops_and_a_wild_reading_is_ignored() {
    let (mut hub, hands, clock) = timed_hub();
    set_headphones(&mut hub, 100);
    hub.apply(wheel_to(Channel::Headphones, 4)).unwrap();

    // Further than a hand can turn in a reading.
    hands.turn(Wheel::Pitch, 30);
    after(&mut hub, &clock, 50);
    assert_eq!(volume_of(&hands, Channel::Headphones), 100);
    // From there on, notches count again.
    hands.turn(Wheel::Pitch, 1);
    after(&mut hub, &clock, 50);
    assert_eq!(volume_of(&hands, Channel::Headphones), 110);

    // A dial pushed against the end of what it can tell says nothing more.
    hands.turn(Wheel::Pitch, i8::MAX);
    hands.turn(Wheel::Pitch, i8::MAX);
    after(&mut hub, &clock, 50);
    let stuck = volume_of(&hands, Channel::Headphones);
    hands.turn(Wheel::Pitch, 5);
    after(&mut hub, &clock, 50);
    assert_eq!(volume_of(&hands, Channel::Headphones), stuck);
}

#[test]
fn a_volume_turned_by_a_dial_stays_in_its_range() {
    let (mut hub, hands, clock) = timed_hub();
    set_headphones(&mut hub, 250);
    hub.apply(wheel_to(Channel::Headphones, WHEEL_STEP_PERCENT.2))
        .unwrap();
    hands.turn(Wheel::Pitch, 10);
    after(&mut hub, &clock, 50);
    assert_eq!(volume_of(&hands, Channel::Headphones), 255);
    hands.turn(Wheel::Pitch, -24);
    after(&mut hub, &clock, 50);
    hands.turn(Wheel::Pitch, -24);
    after(&mut hub, &clock, 50);
    assert_eq!(volume_of(&hands, Channel::Headphones), 0);
}

#[test]
fn each_dial_has_its_own_job_and_a_dial_with_none_does_nothing() {
    let (mut hub, hands, clock) = timed_hub();
    set_headphones(&mut hub, 100);
    hub.apply(wheel_to(Channel::Headphones, 4)).unwrap();
    hub.apply(Intent::SetWheel {
        wheel: Wheel::Echo,
        action: Some(WheelAction::Volume {
            target: AudioTarget::FaderTrack { fader: Fader::C },
            step: 2,
        }),
    })
    .unwrap();
    let music = volume_of(&hands, Channel::Music);

    hands.turn(Wheel::Gender, 4);
    hands.turn(Wheel::Echo, 5);
    after(&mut hub, &clock, 50);
    assert_eq!(volume_of(&hands, Channel::Headphones), 100);
    assert_eq!(volume_of(&hands, Channel::Music), music + 26);
    // The fader under the track travels there.
    assert_eq!(hub.poll().unwrap().faders[2].volume, music + 26);

    // Taking the job away.
    hub.apply(Intent::SetWheel {
        wheel: Wheel::Pitch,
        action: None,
    })
    .unwrap();
    hands.turn(Wheel::Pitch, 3);
    after(&mut hub, &clock, 50);
    assert_eq!(volume_of(&hands, Channel::Headphones), 100);

    // Putting every button and dial back takes the jobs away.
    hub.apply(Intent::ResetControls { button: None }).unwrap();
    hands.turn(Wheel::Echo, 5);
    after(&mut hub, &clock, 50);
    assert_eq!(volume_of(&hands, Channel::Music), music + 26);
}

#[test]
fn turns_made_while_the_device_was_out_of_sight_are_not_counted() {
    let (mut hub, hands, clock) = timed_hub();
    set_headphones(&mut hub, 100);
    hub.apply(wheel_to(Channel::Headphones, 4)).unwrap();

    hub.forget_presses();
    hands.turn(Wheel::Pitch, 10);
    after(&mut hub, &clock, 50);
    assert_eq!(volume_of(&hands, Channel::Headphones), 100);
    hands.turn(Wheel::Pitch, 1);
    after(&mut hub, &clock, 50);
    assert_eq!(volume_of(&hands, Channel::Headphones), 110);
}

#[test]
fn a_dial_leaves_a_volume_the_app_does_not_know_alone_until_it_is_set() {
    let (mut hub, hands, clock) = unknown_hub();
    hub.apply(wheel_to(Channel::Headphones, 4)).unwrap();
    let before = volume_of(&hands, Channel::Headphones);
    // A turn that is fast, and a turn that is slow: the same nothing.
    hands.turn(Wheel::Pitch, 2);
    after(&mut hub, &clock, 50);
    hands.turn(Wheel::Pitch, 20);
    after(&mut hub, &clock, 50);
    assert_eq!(volume_of(&hands, Channel::Headphones), before);
    assert_eq!(
        hub.mixer().volumes[usize::from(Channel::Headphones.index())],
        None
    );

    set_headphones(&mut hub, 100);
    hands.turn(Wheel::Pitch, 2);
    after(&mut hub, &clock, 50);
    assert_eq!(volume_of(&hands, Channel::Headphones), 120);
}

#[test]
fn a_tap_begun_under_other_buttons_does_not_end_under_these() {
    let (mut hub, hands, clock) = timed_hub();
    set_headphones(&mut hub, 50);
    let set = |percent| volume(Channel::Headphones, VolumeMode::Set, percent);
    give(&mut hub, PAD, Gesture::Short, set(20));
    give(
        &mut hub,
        PAD,
        Gesture::Double,
        track(Channel::Chat, MuteMode::Mute),
    );
    let mut other = hub.settings();
    other.controls = crate::Controls::default();
    other.controls.set(PAD, Gesture::Short, Some(set(100)));
    let before = volume_of(&hands, Channel::Headphones);

    // A tap that waits for a second one, then other buttons come.
    tap_without_waiting(&mut hub, &hands, &clock, PAD);
    hub.load(other.clone()).unwrap();
    after(&mut hub, &clock, 1000);
    assert_eq!(volume_of(&hands, Channel::Headphones), before);

    // A press that is still going down, then other buttons come.
    let (mut hub, hands, clock) = timed_hub();
    set_headphones(&mut hub, 50);
    give(&mut hub, PAD, Gesture::Short, set(20));
    give(
        &mut hub,
        PAD,
        Gesture::Long,
        track(Channel::Chat, MuteMode::Mute),
    );
    hands.press(PAD);
    after(&mut hub, &clock, 100);
    hub.load(other).unwrap();
    hands.release(PAD);
    after(&mut hub, &clock, 1000);
    assert_eq!(volume_of(&hands, Channel::Headphones), before);

    // The new buttons work from the next press.
    tap(&mut hub, &hands, &clock, PAD);
    assert_eq!(volume_of(&hands, Channel::Headphones), 255);
}

/// A press and a release, one reading each, and no waiting after.
fn tap_without_waiting(hub: &mut Hub, hands: &VirtualHandle, clock: &ManualClock, button: Button) {
    hands.press(button);
    after(hub, clock, 50);
    hands.release(button);
    after(hub, clock, 50);
}

#[test]
fn a_button_that_switches_profile_notes_it_once_for_as_long_as_it_is_held() {
    let (mut hub, hands, clock) = timed_hub();
    let to_stream = Action::Profile {
        kind: Kind::Mix,
        name: "Stream".into(),
    };
    give(&mut hub, PAD, Gesture::Short, to_stream);
    assert!(hub.take_loads().is_empty());

    hands.press(PAD);
    for _ in 0..5 {
        after(&mut hub, &clock, 50);
    }
    assert_eq!(
        hub.take_loads(),
        [Load {
            kind: Kind::Mix,
            name: "Stream".into()
        }]
    );
    assert!(hub.take_loads().is_empty());
    hands.release(PAD);
    after(&mut hub, &clock, 50);
    assert!(hub.take_loads().is_empty());

    // Nothing of the device changes by itself.
    assert_eq!(hub.mixer().routing, default_routing());
}

#[test]
fn the_settings_a_device_is_brought_to_carry_the_dials() {
    let (mut hub, _hands, _clock) = timed_hub();
    hub.apply(wheel_to(Channel::Headphones, 4)).unwrap();
    let settings = hub.settings();
    assert!(settings.controls.wheel(Wheel::Pitch).is_some());
    assert_eq!(Settings::unknown().controls.wheel(Wheel::Pitch), None);
}

#[test]
fn the_interface_asks_for_the_new_actions_in_the_shape_the_core_expects() {
    let intent = |value: serde_json::Value| serde_json::from_value::<Intent>(value).unwrap();
    assert_eq!(
        intent(json!({
            "type": "setGesture", "button": "samplerTopLeft", "gesture": "hold",
            "action": {
                "type": "route", "input": "music", "output": "broadcastMix", "mode": "off"
            }
        })),
        Intent::SetGesture {
            button: Button::SamplerTopLeft,
            gesture: Gesture::Hold,
            action: Some(music_to_stream(RouteMode::Off)),
        }
    );
    assert_eq!(
        intent(json!({
            "type": "setGesture", "button": "bleep", "gesture": "long",
            "action": {
                "type": "volume", "target": { "type": "channel", "channel": "headphones" },
                "mode": "down", "percent": 5
            }
        })),
        Intent::SetGesture {
            button: Button::Bleep,
            gesture: Gesture::Long,
            action: Some(volume(Channel::Headphones, VolumeMode::Down, 5)),
        }
    );
    assert_eq!(
        intent(json!({
            "type": "setGesture", "button": "bleep", "gesture": "double",
            "action": { "type": "profile", "kind": "mic", "name": "Radio" }
        })),
        Intent::SetGesture {
            button: Button::Bleep,
            gesture: Gesture::Double,
            action: Some(Action::Profile {
                kind: Kind::Mic,
                name: "Radio".into()
            }),
        }
    );
    assert_eq!(
        intent(json!({
            "type": "setWheel", "wheel": "echo",
            "action": { "type": "volume", "target": { "type": "mic" }, "step": 6 }
        })),
        Intent::SetWheel {
            wheel: Wheel::Echo,
            action: Some(WheelAction::Volume {
                target: AudioTarget::Mic,
                step: 6
            }),
        }
    );
    assert_eq!(
        intent(json!({ "type": "setWheel", "wheel": "pitch", "action": null })),
        Intent::SetWheel {
            wheel: Wheel::Pitch,
            action: None
        }
    );
    assert_eq!(
        intent(json!({ "type": "turnWheel", "wheel": "gender", "notches": -2 })),
        Intent::TurnWheel {
            wheel: Wheel::Gender,
            notches: -2
        }
    );
}

#[test]
fn the_snapshot_tells_what_each_dial_does() {
    let (mut hub, _hands, _clock) = timed_hub();
    hub.apply(wheel_to(Channel::Headphones, 4)).unwrap();
    let value = serde_json::to_value(hub.poll().unwrap().controls).unwrap();
    assert_eq!(value["wheels"][0]["wheel"], "pitch");
    assert_eq!(
        value["wheels"][0]["action"],
        json!({
            "type": "volume", "target": { "type": "channel", "channel": "headphones" }, "step": 4
        })
    );
    assert!(value["wheels"][3]["action"].is_null());
}
