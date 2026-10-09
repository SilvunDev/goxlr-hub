//! Routes, volumes, dials and profiles asked for by buttons, against the
//! virtual device and a clock that only moves when the test says so.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use goxlr_hub_device::{
    DeviceKind, PastTheEnd, Session, VirtualGoXlr, VirtualHandle, open_virtual,
};
use goxlr_hub_protocol::{
    Button, ButtonLight, Channel, Fader, OutputSet, RoutingInput, RoutingOutput, Side, Wheel,
};
use serde_json::json;

use crate::controls_tests::{Deaf, OTHER_PAD, PAD, after, give, muted, timed_hub, track};
use crate::dials::{place, volume_at};
use crate::gestures::ManualClock;
use crate::{
    Action, AudioTarget, Gesture, Hub, Intent, Kind, Load, MuteMode, RouteMode, Settings,
    VolumeMode, WheelAction, default_routing,
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

fn wheel_of(wheel: Wheel, channel: Channel, step: u8) -> Intent {
    Intent::SetWheel {
        wheel,
        action: Some(WheelAction::Volume {
            target: AudioTarget::Channel { channel },
            step,
        }),
    }
}

fn wheel_to(channel: Channel, step: u8) -> Intent {
    wheel_of(Wheel::Pitch, channel, step)
}

/// Lets the dials that have a job find their travel and go where their volume
/// is, as they do when they are given one.
fn ready(hub: &mut Hub, clock: &ManualClock) {
    for _ in 0..400 {
        if hub.dials_are_ready() {
            // One more reading sees the dial where it was put.
            after(hub, clock, 50);
            return;
        }
        after(hub, clock, 50);
    }
    panic!("the dials never got ready");
}

/// Readings for the dials to notice that a volume changed, then be ready.
fn settled(hub: &mut Hub, clock: &ManualClock) {
    for _ in 0..3 {
        after(hub, clock, 50);
    }
    ready(hub, clock);
}

/// A hub, its device and its clock, with the pitch dial given the volume of
/// the headphones, known and set to 100.
fn with_a_dial(step: u8) -> (Hub, VirtualHandle, ManualClock) {
    let (mut hub, hands, clock) = timed_hub();
    set_headphones(&mut hub, 100);
    hub.apply(wheel_to(Channel::Headphones, step)).unwrap();
    ready(&mut hub, &clock);
    (hub, hands, clock)
}

/// Where a dial is on the device, and how far it can go there.
fn dial_at(hands: &VirtualHandle, wheel: Wheel) -> (i8, (i8, i8)) {
    let state = hands.state();
    (
        state.encoders[wheel.index()],
        state.encoder_ranges[wheel.index()],
    )
}

/// Whether a dial stands where the volume says.
fn says(hands: &VirtualHandle, wheel: Wheel, volume: u8) -> bool {
    let (at, (low, high)) = dial_at(hands, wheel);
    at == place(low, high, volume)
}

#[test]
fn a_dial_given_a_job_is_put_where_the_volume_is_and_the_volume_is_not_changed() {
    for wheel in Wheel::ALL {
        for volume in [0u8, 1, 100, 128, 254, 255] {
            let (mut hub, hands, clock) = timed_hub();
            set_headphones(&mut hub, volume);
            hub.apply(wheel_of(wheel, Channel::Headphones, 4)).unwrap();
            let seen = hub.poll().unwrap();
            assert_eq!(seen.dials[wheel.index()].state, "measuring");
            ready(&mut hub, &clock);
            assert!(says(&hands, wheel, volume), "{wheel:?} at {volume}");
            assert_eq!(volume_of(&hands, Channel::Headphones), volume);
            assert_eq!(
                hub.mixer().volumes[usize::from(Channel::Headphones.index())],
                Some(volume)
            );
        }
    }
}

#[test]
fn the_hand_sets_the_volume_and_a_dial_stops_at_zero_and_a_hundred_percent() {
    for wheel in Wheel::ALL {
        let (mut hub, hands, clock) = timed_hub();
        set_headphones(&mut hub, 100);
        hub.apply(wheel_of(wheel, Channel::Headphones, 4)).unwrap();
        ready(&mut hub, &clock);
        let (_, (low, high)) = dial_at(&hands, wheel);

        let mut last = volume_of(&hands, Channel::Headphones);
        for _ in 0..70 {
            hands.turn(wheel, 1);
            after(&mut hub, &clock, 50);
            let now = volume_of(&hands, Channel::Headphones);
            assert!(now >= last, "{wheel:?} going up");
            assert_eq!(now, volume_at(low, high, dial_at(&hands, wheel).0));
            last = now;
        }
        assert_eq!(last, 255, "{wheel:?}");
        assert_eq!(dial_at(&hands, wheel).0, high);
        // It stays there, and the dial is not put anywhere else.
        for _ in 0..5 {
            hands.turn(wheel, 3);
            after(&mut hub, &clock, 50);
        }
        assert_eq!(
            (
                volume_of(&hands, Channel::Headphones),
                dial_at(&hands, wheel).0
            ),
            (255, high)
        );

        for _ in 0..70 {
            hands.turn(wheel, -1);
            after(&mut hub, &clock, 50);
            let now = volume_of(&hands, Channel::Headphones);
            assert!(now <= last, "{wheel:?} going down");
            last = now;
        }
        assert_eq!(last, 0, "{wheel:?}");
        assert_eq!(dial_at(&hands, wheel).0, low);
        hands.turn(wheel, -5);
        after(&mut hub, &clock, 50);
        assert_eq!(volume_of(&hands, Channel::Headphones), 0);
    }
}

#[test]
fn a_volume_changed_by_other_means_puts_the_dial_where_it_says() {
    let (mut hub, hands, clock) = with_a_dial(4);
    let headphones = |hands: &VirtualHandle| volume_of(hands, Channel::Headphones);

    // The screen.
    set_headphones(&mut hub, 30);
    settled(&mut hub, &clock);
    assert!(says(&hands, Wheel::Pitch, 30));
    assert_eq!(headphones(&hands), 30);

    // A button, set, raised and lowered.
    let h = |mode, percent| volume(Channel::Headphones, mode, percent);
    give(&mut hub, PAD, Gesture::Short, h(VolumeMode::Set, 80));
    give(&mut hub, OTHER_PAD, Gesture::Short, h(VolumeMode::Down, 30));
    tap(&mut hub, &hands, &clock, PAD);
    settled(&mut hub, &clock);
    assert_eq!(headphones(&hands), 204);
    assert!(says(&hands, Wheel::Pitch, 204));
    tap(&mut hub, &hands, &clock, OTHER_PAD);
    settled(&mut hub, &clock);
    assert_eq!(headphones(&hands), 127);
    assert!(says(&hands, Wheel::Pitch, 127));

    // A profile with another volume.
    let mut other = hub.settings();
    other.mixer.volumes[usize::from(Channel::Headphones.index())] = Some(40);
    hub.load(other).unwrap();
    settled(&mut hub, &clock);
    assert_eq!(headphones(&hands), 40);
    assert!(says(&hands, Wheel::Pitch, 40));
}

#[test]
fn a_dial_on_the_track_under_a_fader_follows_the_fader() {
    let (mut hub, hands, clock) = timed_hub();
    hub.apply(Intent::SetWheel {
        wheel: Wheel::Echo,
        action: Some(WheelAction::Volume {
            target: AudioTarget::FaderTrack { fader: Fader::C },
            step: 4,
        }),
    })
    .unwrap();
    ready(&mut hub, &clock);
    let music = volume_of(&hands, Channel::Music);
    assert!(says(&hands, Wheel::Echo, music));

    hands.move_fader(Fader::C, 200);
    settled(&mut hub, &clock);
    assert_eq!(volume_of(&hands, Channel::Music), 200);
    assert!(says(&hands, Wheel::Echo, 200));

    // The dial moves the fader, and does not fight it.
    let (_, (low, high)) = dial_at(&hands, Wheel::Echo);
    hands.turn(Wheel::Echo, -4);
    after(&mut hub, &clock, 50);
    let now = volume_of(&hands, Channel::Music);
    assert_eq!(now, volume_at(low, high, dial_at(&hands, Wheel::Echo).0));
    assert!(now < 200);
    settled(&mut hub, &clock);
    assert_eq!(volume_of(&hands, Channel::Music), now);
}

#[test]
fn dials_on_the_same_track_follow_each_other() {
    let (mut hub, hands, clock) = timed_hub();
    set_headphones(&mut hub, 100);
    for wheel in Wheel::ALL {
        hub.apply(wheel_of(wheel, Channel::Headphones, 4)).unwrap();
        ready(&mut hub, &clock);
    }
    for wheel in Wheel::ALL {
        assert!(says(&hands, wheel, 100), "{wheel:?}");
    }

    for turned in Wheel::ALL {
        hands.turn(turned, 5);
        after(&mut hub, &clock, 50);
        let volume = volume_of(&hands, Channel::Headphones);
        after(&mut hub, &clock, 50);
        after(&mut hub, &clock, 50);
        assert_eq!(volume_of(&hands, Channel::Headphones), volume, "{turned:?}");
        for wheel in Wheel::ALL {
            assert!(says(&hands, wheel, volume), "{wheel:?} after {turned:?}");
        }
    }
}

#[test]
fn a_hand_that_moves_a_dial_as_the_volume_changes_does_not_make_it_jump() {
    let (mut hub, hands, clock) = with_a_dial(4);
    let before = volume_of(&hands, Channel::Headphones);
    // The volume goes down by the screen and, in the same reading, a hand
    // nudges the dial where it still says the old volume.
    set_headphones(&mut hub, 10);
    hands.turn(Wheel::Pitch, 1);
    after(&mut hub, &clock, 50);
    assert_eq!(volume_of(&hands, Channel::Headphones), 10, "was {before}");
    settled(&mut hub, &clock);
    assert_eq!(volume_of(&hands, Channel::Headphones), 10);
    assert!(says(&hands, Wheel::Pitch, 10));
}

#[test]
fn no_volume_jumps_when_the_dial_is_met_again_or_a_profile_is_loaded() {
    // The dial moved while the device was out of sight.
    let (mut hub, hands, clock) = with_a_dial(4);
    hub.forget_presses();
    hands.turn(Wheel::Pitch, -7);
    ready(&mut hub, &clock);
    assert_eq!(volume_of(&hands, Channel::Headphones), 100);
    assert!(says(&hands, Wheel::Pitch, 100));

    // Another job for the same dial, or for another: nothing is set.
    hub.apply(wheel_to(Channel::Music, 4)).unwrap();
    ready(&mut hub, &clock);
    assert_eq!(volume_of(&hands, Channel::Headphones), 100);
    let music = volume_of(&hands, Channel::Music);
    assert!(says(&hands, Wheel::Pitch, music));
    assert_eq!(volume_of(&hands, Channel::Music), music);

    // A profile loaded with other jobs for the dials.
    let mut other = hub.settings();
    other.controls.set_wheel(
        Wheel::Pitch,
        Some(WheelAction::Volume {
            target: AudioTarget::Channel {
                channel: Channel::Headphones,
            },
            step: 2,
        }),
    );
    hub.load(other).unwrap();
    assert!(!hub.dials_are_ready());
    ready(&mut hub, &clock);
    assert_eq!(volume_of(&hands, Channel::Headphones), 100);
    assert!(says(&hands, Wheel::Pitch, 100));
}

#[test]
fn a_dial_that_has_no_job_is_left_alone_by_the_app() {
    let (mut hub, hands, clock) = timed_hub();
    let before = hands.state().encoders;
    hands.turn(Wheel::Gender, 5);
    for _ in 0..40 {
        after(&mut hub, &clock, 50);
    }
    // The finger moved it, the app did not.
    assert_eq!(hands.state().encoders[Wheel::Gender.index()], before[1] + 5);
    assert_eq!(hands.state().encoders[Wheel::Pitch.index()], before[0]);
}

#[test]
fn a_volume_the_app_does_not_know_leaves_the_dial_alone_until_it_is_set() {
    let (mut hub, hands, clock) = unknown_hub();
    hub.apply(wheel_to(Channel::Headphones, 4)).unwrap();
    let before = hands.state().encoders;
    for _ in 0..100 {
        after(&mut hub, &clock, 50);
    }
    // Not measured, not put anywhere.
    assert_eq!(hands.state().encoders, before);
    assert_eq!(
        hub.poll().unwrap().dials[0].state,
        "unknownVolume",
        "the screen says it"
    );
    hands.turn(Wheel::Pitch, 4);
    hands.turn(Wheel::Pitch, 20);
    after(&mut hub, &clock, 50);
    assert_eq!(
        hub.mixer().volumes[usize::from(Channel::Headphones.index())],
        None
    );
    assert_eq!(dial_at(&hands, Wheel::Pitch).0, 24);

    // Set once, the dial is measured and put there, and the volume stays.
    set_headphones(&mut hub, 100);
    ready(&mut hub, &clock);
    assert_eq!(volume_of(&hands, Channel::Headphones), 100);
    assert!(says(&hands, Wheel::Pitch, 100));
}

#[test]
fn a_dial_that_the_device_does_not_put_is_only_followed_and_says_so() {
    // A device that does not hear the command that puts a dial somewhere.
    let (mut hub, hands, clock) = timed_hub();
    hands.ignore_encoder_writes(true);
    set_headphones(&mut hub, 100);
    hub.apply(wheel_to(Channel::Headphones, 4)).unwrap();
    ready(&mut hub, &clock);
    assert_eq!(volume_of(&hands, Channel::Headphones), 100, "no jump");
    assert_eq!(hub.poll().unwrap().dials[0].state, "followOnly");

    // The volume is followed by how far the dial turns, a few percent a notch.
    hands.turn(Wheel::Pitch, 3);
    after(&mut hub, &clock, 50);
    assert_eq!(volume_of(&hands, Channel::Headphones), 131);
    // And it stops where the device stops it, nothing is made up.
    hands.turn(Wheel::Pitch, 100);
    after(&mut hub, &clock, 50);
    let stuck = volume_of(&hands, Channel::Headphones);
    hands.turn(Wheel::Pitch, 5);
    after(&mut hub, &clock, 50);
    assert_eq!(volume_of(&hands, Channel::Headphones), stuck);
}

#[test]
fn a_reading_too_far_to_be_a_hand_does_not_set_the_volume() {
    let (mut hub, hands, clock) = timed_hub();
    // A wide travel, wider than a hand can turn in a reading.
    hands.limit(Wheel::Pitch, -100, 100);
    set_headphones(&mut hub, 100);
    hub.apply(wheel_to(Channel::Headphones, 4)).unwrap();
    ready(&mut hub, &clock);
    assert!(says(&hands, Wheel::Pitch, 100));

    hands.turn(Wheel::Pitch, 40);
    after(&mut hub, &clock, 50);
    assert_eq!(volume_of(&hands, Channel::Headphones), 100);
    settled(&mut hub, &clock);
    assert_eq!(volume_of(&hands, Channel::Headphones), 100);
    assert!(says(&hands, Wheel::Pitch, 100));
}

#[test]
fn a_device_that_refuses_to_put_a_dial_does_not_fail_the_reading() {
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
    hub.apply(wheel_to(Channel::Headphones, 4)).unwrap();
    after(&mut hub, &clock, 50);
    assert!(!hub.dials_are_ready());

    // From here the device answers readings, and nothing else.
    deaf.store(true, Ordering::Relaxed);
    hands.turn(Wheel::Pitch, 2);
    for _ in 0..3 {
        clock.advance(Duration::from_millis(50));
        assert!(hub.poll().is_ok(), "a dial that stays put is no failure");
    }
    assert!(hub.dials_are_ready(), "it is followed as it stands");
    assert_eq!(hub.poll().unwrap().dials[0].state, "followOnly");
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
    ready(&mut hub, &clock);
    let music = volume_of(&hands, Channel::Music);

    hands.turn(Wheel::Gender, 4);
    hands.turn(Wheel::Echo, 5);
    after(&mut hub, &clock, 50);
    assert_eq!(volume_of(&hands, Channel::Headphones), 100);
    let (_, (low, high)) = dial_at(&hands, Wheel::Echo);
    let now = volume_of(&hands, Channel::Music);
    assert!(now > music);
    assert_eq!(now, volume_at(low, high, dial_at(&hands, Wheel::Echo).0));

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
    assert_eq!(volume_of(&hands, Channel::Music), now);
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

/// Gives the four dials the same track, one after the other with seconds
/// between, as a user does from the screen, then turns each far in both
/// directions.
fn four_dials_one_after_the_other(past: PastTheEnd, travels: [(i8, i8); 4]) {
    let (mut hub, hands, clock) = timed_hub();
    hands.past_the_end(past);
    for (wheel, (low, high)) in Wheel::ALL.into_iter().zip(travels) {
        hands.limit(wheel, low, high);
    }
    set_headphones(&mut hub, 77);
    for wheel in Wheel::ALL {
        hub.apply(wheel_of(wheel, Channel::Headphones, 2)).unwrap();
        ready(&mut hub, &clock);
        // Seconds pass, nobody touches the device.
        for _ in 0..60 {
            after(&mut hub, &clock, 50);
        }
        assert_eq!(volume_of(&hands, Channel::Headphones), 77);
    }
    for wheel in Wheel::ALL {
        assert!(says(&hands, wheel, 77), "{wheel:?}, {past:?}, {travels:?}");
    }
    for wheel in Wheel::ALL {
        let (low, high) = hands.state().encoder_ranges[wheel.index()];
        let travel = (i16::from(high) - i16::from(low)) as usize;
        for _ in 0..travel + 4 {
            hands.turn(wheel, 1);
            after(&mut hub, &clock, 50);
        }
        assert_eq!(
            volume_of(&hands, Channel::Headphones),
            255,
            "{wheel:?} up, {past:?}, {travels:?}"
        );
        for _ in 0..travel + 4 {
            hands.turn(wheel, -1);
            after(&mut hub, &clock, 50);
        }
        assert_eq!(
            volume_of(&hands, Channel::Headphones),
            0,
            "{wheel:?} down, {past:?}, {travels:?}"
        );
        set_headphones(&mut hub, 77);
        settled(&mut hub, &clock);
        for other in Wheel::ALL {
            assert!(says(&hands, other, 77), "{other:?} after {wheel:?}");
        }
    }
}

#[test]
fn four_dials_given_the_same_track_one_after_the_other_all_set_it() {
    for past in [PastTheEnd::Stops, PastTheEnd::Refused, PastTheEnd::Resets] {
        for travels in [
            // What the real ones were found to be.
            [(-24, 24), (-24, 24), (0, 24), (0, 24)],
            [(-24, 24), (-12, 12), (0, 36), (0, 36)],
            [(-12, 12), (-12, 12), (0, 100), (0, 100)],
            [(-24, 24), (-24, 24), (0, 127), (-128, 127)],
            [(-20, 20), (-15, 15), (0, 20), (0, 15)],
        ] {
            four_dials_one_after_the_other(past, travels);
        }
    }
}

#[test]
fn the_snapshot_says_what_each_dial_is_doing_and_what_it_sets() {
    let (mut hub, hands, clock) = timed_hub();
    set_headphones(&mut hub, 100);
    hub.apply(wheel_to(Channel::Headphones, 4)).unwrap();
    hands.turn(Wheel::Gender, 4);

    let snapshot = after(&mut hub, &clock, 50);
    assert_eq!(snapshot.dials.len(), 4);
    assert_eq!(snapshot.dials[0].state, "measuring");
    assert_eq!(snapshot.dials[0].asked, Some(8));
    // A dial with no job is not touched, but its position is told.
    assert_eq!(snapshot.dials[1].state, "idle");
    assert_eq!(snapshot.dials[1].reading, 4);

    ready(&mut hub, &clock);
    let snapshot = hub.poll().unwrap();
    assert_eq!(
        serde_json::to_value(snapshot.dials[0]).unwrap(),
        json!({
            "wheel": "pitch", "reading": -5, "state": "ready",
            "low": -24, "high": 24, "percent": 39, "notch": 19, "notches": 48,
            "asked": null, "refused": false
        })
    );

    // A volume that changes by other means: the dial says "syncing" until it
    // is where the volume is.
    set_headphones(&mut hub, 255);
    let snapshot = after(&mut hub, &clock, 50);
    assert_eq!(snapshot.dials[0].state, "syncing");
    assert_eq!(snapshot.dials[0].percent, Some(100));
    settled(&mut hub, &clock);
    let snapshot = hub.poll().unwrap();
    assert_eq!(snapshot.dials[0].state, "ready");
    assert_eq!(
        (snapshot.dials[0].notch, snapshot.dials[0].notches),
        (Some(48), Some(48))
    );

    // A dial the device does not obey is followed only, and says so.
    let (mut hub, hands, clock) = timed_hub();
    hands.ignore_encoder_writes(true);
    hub.apply(wheel_to(Channel::Headphones, 4)).unwrap();
    ready(&mut hub, &clock);
    let snapshot = hub.poll().unwrap();
    assert_eq!(snapshot.dials[0].state, "followOnly");
    assert_eq!(
        (snapshot.dials[0].low, snapshot.dials[0].high),
        (None, None)
    );
    assert_eq!(snapshot.dials[0].notch, None);
}
