//! Follows a dial that sets a volume: the place of the dial in its travel is
//! the volume.
//!
//! The bottom of the travel is 0 %, the top is 100 %, and the ring of lights
//! of the dial shows it. The dial stops by itself at the two ends. How far the
//! travel goes is not told by the device: it is felt for, once, when the dial
//! is given a job, and only when the volume it sets is known. When the volume
//! changes by other means (the screen, a button, a fader, a profile, another
//! dial), the dial is put at the place that says the new volume.
//!
//! A dial whose travel cannot be measured, or that the device does not put
//! where it is told, is only followed by how far it turns, a few percent for
//! each notch. It can then stop at the end of its travel.

/// A dial that seems to have moved further than this between two readings did
/// not: it is a reading to ignore.
const MAX_JUMP: i16 = 24;

/// How far a dial is first asked to go at each reading while its travel is
/// felt for.
const FEEL_STEP: i8 = 8;

/// Readings a dial is felt for at most: about four seconds. After that it is
/// only followed, whatever it did meanwhile.
const FEEL_READINGS: u8 = 80;

/// A travel narrower than this is not one worth setting a volume with.
const LEAST_TRAVEL: i16 = 4;

/// A volume that moves by no more than this, out of 255, is the same: a fader
/// stops a little off where it was sent.
const SAME_VOLUME: u8 = 6;

/// Where a dial is in its life with the app.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dial {
    /// Not seen yet since it was given a job, or the volume it sets is not
    /// known: nothing is asked of it.
    Fresh,
    /// Being asked to go further, step by step, to find the ends of its
    /// travel. It is not followed meanwhile.
    Feeling {
        /// Going up, then down.
        up: bool,
        /// The furthest it was seen to go in this direction.
        at: i8,
        /// The top, once found.
        high: i8,
        /// What it was asked, and is expected to report.
        asked: Option<i8>,
        /// How far it is asked to go: less each time a command is refused,
        /// to find the end of the travel to the notch.
        step: i8,
        /// Readings left before it is given up on.
        left: u8,
    },
    /// Its travel is known: its place is the volume.
    Tracking {
        low: i8,
        high: i8,
        /// Where it was at the last reading.
        last: i8,
        /// Where it was last told to go, and not seen there yet.
        sent: Option<i8>,
        /// Commands in a row that the device did not obey.
        doubts: u8,
        /// Its place says the volume, as far as it is known.
        synced: bool,
        /// The volume when it was last in step with it.
        seen: u8,
    },
    /// It cannot be put where the volume is: it is followed by how far it
    /// turns, and stops where the device stops it.
    Free { last: i8 },
}

/// What a reading makes of a dial.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Step {
    pub dial: Dial,
    /// The volume the hand set, 0 to 255.
    pub volume: Option<u8>,
    /// How far a dial that is only followed was turned: positive is up.
    pub notches: i16,
    /// Where it is to be put.
    pub put: Option<i8>,
}

impl Step {
    fn stay(dial: Dial) -> Self {
        Self {
            dial,
            volume: None,
            notches: 0,
            put: None,
        }
    }
}

fn turned(last: i8, now: i8) -> i16 {
    let notches = i16::from(now) - i16::from(last);
    if notches.abs() > MAX_JUMP { 0 } else { notches }
}

/// The place of a dial in its travel that says a volume.
pub fn place(low: i8, high: i8, volume: u8) -> i8 {
    let travel = i32::from(high) - i32::from(low);
    let at = (i32::from(volume) * travel * 2 + 255).div_euclid(510);
    (i32::from(low) + at).clamp(i32::from(low), i32::from(high)) as i8
}

/// The volume a place in the travel of a dial says.
pub fn volume_at(low: i8, high: i8, place: i8) -> u8 {
    let travel = i32::from(high) - i32::from(low);
    let place = i32::from(place).clamp(i32::from(low), i32::from(high));
    ((place - i32::from(low)) * 255 * 2 + travel).div_euclid(travel * 2) as u8
}

impl Dial {
    /// Whether it can be followed, and in step with the volume.
    #[cfg(test)]
    pub fn is_ready(self) -> bool {
        matches!(
            self,
            Self::Tracking { synced: true, .. } | Self::Free { .. }
        )
    }

    /// What the dial is doing, for whoever looks: `waiting`, `measuring`,
    /// `ready`, `syncing` or `followOnly`.
    pub fn state(self) -> &'static str {
        match self {
            Self::Fresh => "waiting",
            Self::Feeling { .. } => "measuring",
            Self::Tracking { synced: true, .. } => "ready",
            Self::Tracking { .. } => "syncing",
            Self::Free { .. } => "followOnly",
        }
    }

    /// The travel found, lowest and highest.
    pub fn travel(self) -> Option<(i8, i8)> {
        match self {
            Self::Tracking { low, high, .. } => Some((low, high)),
            _ => None,
        }
    }

    /// What it was last asked while it is felt for.
    pub fn asked(self) -> Option<i8> {
        match self {
            Self::Feeling { asked, .. } => asked,
            _ => None,
        }
    }

    /// Takes a reading of the dial, with the volume it sets as it is now:
    /// `None` when the app does not know it.
    pub fn read(self, now: i8, volume: Option<u8>) -> Step {
        match self {
            Self::Fresh => {
                if volume.is_none() {
                    return Step::stay(Self::Fresh);
                }
                feel(true, now, now, None, FEEL_STEP, FEEL_READINGS, now)
            }
            Self::Feeling {
                up,
                at,
                high,
                asked,
                step,
                left,
            } => feel(up, at, high, asked, step, left, now),
            Self::Tracking {
                low,
                high,
                last,
                sent,
                doubts,
                synced,
                seen,
            } => {
                let Some(volume) = volume else {
                    // What it would set is not known any more: it is left be.
                    return Step::stay(Self::Fresh);
                };
                track(low, high, last, sent, doubts, synced, seen, now, volume)
            }
            Self::Free { last } => Step {
                dial: Self::Free { last: now },
                volume: None,
                notches: turned(last, now),
                put: None,
            },
        }
    }

    /// The device did not take what it was asked: the dial is only followed.
    pub fn cannot_be_put(now: i8) -> Self {
        Self::Free { last: now }
    }
}

#[allow(clippy::too_many_arguments)]
fn track(
    low: i8,
    high: i8,
    mut last: i8,
    sent: Option<i8>,
    doubts: u8,
    mut synced: bool,
    seen: u8,
    now: i8,
    volume: u8,
) -> Step {
    let tracking = |last, sent, doubts, synced, seen| Dial::Tracking {
        low,
        high,
        last,
        sent,
        doubts,
        synced,
        seen,
    };

    // A command was sent at the last reading: did the dial go there?
    let mut doubts = doubts;
    if let Some(told) = sent {
        if now == told {
            return settle(low, high, volume, now);
        } else if now == last {
            // Not obeyed. Twice in a row, the device does not.
            if doubts >= 1 {
                return Step::stay(Dial::Free { last: now });
            }
            doubts += 1;
            let put = place(low, high, volume);
            return Step {
                dial: tracking(last, Some(put), doubts, false, seen),
                volume: None,
                notches: 0,
                put: Some(put),
            };
        }
        // It went somewhere else: a hand. Its place says nothing yet.
        last = now;
        synced = false;
    } else if now != last {
        // A hand moved it. It sets the volume only if its place said the
        // volume, which has not changed by other means since, and if the move
        // is one a hand can make.
        let trusted = synced && volume.abs_diff(seen) <= SAME_VOLUME && turned(last, now) != 0;
        if trusted {
            let set = volume_at(low, high, now);
            return Step {
                dial: tracking(now, None, 0, true, set),
                volume: Some(set),
                notches: 0,
                put: None,
            };
        }
        last = now;
        synced = false;
    } else if synced && volume.abs_diff(seen) > SAME_VOLUME {
        // The volume changed by other means: the place no longer says it.
        synced = false;
    }

    if synced {
        return Step::stay(tracking(last, None, 0, true, seen));
    }
    settle(low, high, volume, now)
}

/// Brings a dial that is where the app left it to the place of the volume, or
/// finds it there.
fn settle(low: i8, high: i8, volume: u8, now: i8) -> Step {
    let want = place(low, high, volume);
    if now == want {
        return Step::stay(Dial::Tracking {
            low,
            high,
            last: now,
            sent: None,
            doubts: 0,
            synced: true,
            seen: volume,
        });
    }
    Step {
        dial: Dial::Tracking {
            low,
            high,
            last: now,
            sent: Some(want),
            doubts: 0,
            synced: false,
            seen: volume,
        },
        volume: None,
        notches: 0,
        put: Some(want),
    }
}

/// The next ask of a dial that is being felt for, a step further.
fn further(up: bool, from: i8, step: i8) -> Option<i8> {
    let to = if up {
        from.saturating_add(step)
    } else {
        from.saturating_sub(step)
    };
    (to != from).then_some(to)
}

fn feel(up: bool, prev: i8, high: i8, asked: Option<i8>, step: i8, left: u8, now: i8) -> Step {
    // A dial that does not answer as expected is not waited for for ever.
    if left == 0 {
        return Step::stay(Dial::Free { last: now });
    }
    let left = left - 1;
    // What the dial did with what it was asked. It went where it was asked:
    // on to the next step. It stopped short but went on: that is the end of
    // its travel. It did not go on, or went back: the command was not obeyed,
    // the step was too long, a shorter one is tried until it is one notch.
    let moved_on = if up { now > prev } else { now < prev };
    let (at, step, end) = match asked {
        Some(asked) if now == asked => (now, step, false),
        Some(_) if !moved_on && step > 1 => (prev, step / 2, false),
        Some(_) if !moved_on => (prev, step, true),
        Some(_) => (now, step, true),
        None => (now, step, false),
    };
    if !end && let Some(to) = further(up, at, step) {
        return Step {
            dial: Dial::Feeling {
                up,
                at,
                high,
                asked: Some(to),
                step,
                left,
            },
            volume: None,
            notches: 0,
            put: Some(to),
        };
    }
    if up {
        // The top is found: the bottom is next, from here.
        return match further(false, at, FEEL_STEP) {
            Some(to) => Step {
                dial: Dial::Feeling {
                    up: false,
                    at,
                    high: at,
                    asked: Some(to),
                    step: FEEL_STEP,
                    left,
                },
                volume: None,
                notches: 0,
                put: Some(to),
            },
            None => Step::stay(Dial::Free { last: now }),
        };
    }
    // The bottom is found too.
    let low = at;
    if i16::from(high) - i16::from(low) < LEAST_TRAVEL {
        return Step::stay(Dial::Free { last: now });
    }
    // The dial stands at the bottom, and its place does not say the volume
    // yet: the next reading puts it where the volume is.
    Step::stay(Dial::Tracking {
        low,
        high,
        last: at,
        sent: None,
        doubts: 0,
        synced: false,
        seen: 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A device with a travel, answering what a dial is asked.
    struct Knob {
        low: i8,
        high: i8,
        at: i8,
    }

    impl Knob {
        fn put(&mut self, value: i8) {
            self.at = value.clamp(self.low, self.high);
        }

        fn turn(&mut self, notches: i8) {
            self.put(self.at.saturating_add(notches));
        }
    }

    /// One reading: what the dial makes of the knob, and what is done of it.
    fn read(dial: &mut Dial, knob: &mut Knob, volume: &mut Option<u8>) -> Step {
        let step = dial.read(knob.at, *volume);
        if let Some(value) = step.put {
            knob.put(value);
        }
        if let Some(set) = step.volume {
            *volume = Some(set);
        }
        *dial = step.dial;
        step
    }

    /// Reads the knob until the dial is ready.
    fn settle_down(dial: &mut Dial, knob: &mut Knob, volume: &mut Option<u8>) -> usize {
        let mut readings = 0;
        while !dial.is_ready() {
            let before = *volume;
            let step = read(dial, knob, volume);
            assert_eq!(step.volume, None, "nothing is set while it is put");
            assert_eq!(*volume, before);
            readings += 1;
            assert!(readings < 100, "never settled: {dial:?}");
        }
        readings
    }

    const TRAVELS: [(i8, i8); 5] = [(-24, 24), (-12, 12), (0, 24), (0, 36), (-3, 9)];

    #[test]
    fn the_place_of_a_dial_and_the_volume_it_says_are_the_same_both_ways() {
        for (low, high) in TRAVELS {
            assert_eq!(place(low, high, 0), low);
            assert_eq!(place(low, high, 255), high);
            assert_eq!(volume_at(low, high, low), 0);
            assert_eq!(volume_at(low, high, high), 255);
            // Out of its travel, the dial says the ends.
            assert_eq!(volume_at(low, high, high.saturating_add(9)), 255);
            for at in low..=high {
                assert_eq!(
                    place(low, high, volume_at(low, high, at)),
                    at,
                    "{low}..{high}"
                );
            }
            let mut last = 0;
            for volume in 0..=255u8 {
                let at = place(low, high, volume);
                assert!(at >= last || volume == 0);
                last = at;
            }
        }
    }

    #[test]
    fn a_dial_finds_its_travel_and_is_put_where_the_volume_is() {
        for (low, high) in TRAVELS {
            for start in [low, high, (low + high) / 2] {
                for volume in [0u8, 77, 128, 255] {
                    let mut knob = Knob {
                        low,
                        high,
                        at: start,
                    };
                    let mut dial = Dial::Fresh;
                    let mut volume = Some(volume);
                    let wanted = volume;
                    settle_down(&mut dial, &mut knob, &mut volume);
                    assert_eq!(volume, wanted, "the volume is not changed");
                    assert_eq!(dial.travel(), Some((low, high)));
                    assert_eq!(knob.at, place(low, high, wanted.unwrap()), "{low}..{high}");
                }
            }
        }
    }

    #[test]
    fn a_dial_that_does_not_know_its_volume_is_left_alone() {
        let mut knob = Knob {
            low: -24,
            high: 24,
            at: 5,
        };
        let mut dial = Dial::Fresh;
        for _ in 0..10 {
            let step = dial.read(knob.at, None);
            assert_eq!((step.put, step.volume, step.notches), (None, None, 0));
            assert_eq!(step.dial, Dial::Fresh);
            knob.turn(3);
        }
        // Known later, it is felt for and put at the volume.
        let mut volume = Some(200);
        settle_down(&mut dial, &mut knob, &mut volume);
        assert_eq!(volume, Some(200));
    }

    #[test]
    fn the_hand_sets_the_volume_and_the_dial_stops_at_the_ends() {
        for (low, high) in TRAVELS {
            let mut knob = Knob { low, high, at: low };
            let mut dial = Dial::Fresh;
            let mut volume = Some(100);
            settle_down(&mut dial, &mut knob, &mut volume);

            // Up all the way and further: it stops at 100 %.
            let mut last = volume.unwrap();
            for _ in 0..40 {
                knob.turn(2);
                read(&mut dial, &mut knob, &mut volume);
                assert!(volume.unwrap() >= last, "{low}..{high}");
                assert_eq!(volume.unwrap(), volume_at(low, high, knob.at));
                last = volume.unwrap();
            }
            assert_eq!(volume, Some(255));
            assert_eq!(knob.at, high);
            // And down all the way.
            for _ in 0..40 {
                knob.turn(-2);
                read(&mut dial, &mut knob, &mut volume);
            }
            assert_eq!(volume, Some(0));
            assert_eq!(knob.at, low);
            // Nothing was put back on the way: the dial is where the hand left it.
            assert!(dial.is_ready());
        }
    }

    #[test]
    fn a_volume_changed_by_other_means_puts_the_dial_where_it_says() {
        let mut knob = Knob {
            low: 0,
            high: 24,
            at: 0,
        };
        let mut dial = Dial::Fresh;
        let mut volume = Some(100);
        settle_down(&mut dial, &mut knob, &mut volume);
        for other in [10u8, 255, 0, 128, 150] {
            volume = Some(other);
            let mut readings = 0;
            while !dial.is_ready() || knob.at != place(0, 24, other) {
                let step = read(&mut dial, &mut knob, &mut volume);
                assert_eq!(step.volume, None);
                readings += 1;
                assert!(readings < 10);
            }
            assert_eq!(volume, Some(other), "not changed by the dial");
        }
    }

    #[test]
    fn a_hand_that_moves_a_dial_while_the_volume_changes_does_not_set_it() {
        let mut knob = Knob {
            low: -24,
            high: 24,
            at: 0,
        };
        let mut dial = Dial::Fresh;
        let mut volume = Some(100);
        settle_down(&mut dial, &mut knob, &mut volume);

        // The volume goes to 10 by other means, and at the same instant a hand
        // nudges the dial where it still says 100.
        volume = Some(10);
        knob.turn(1);
        let step = dial.read(knob.at, volume);
        assert_eq!(step.volume, None);
        dial = step.dial;
        if let Some(value) = step.put {
            knob.put(value);
        }
        assert_eq!(knob.at, place(-24, 24, 10), "put where it says 10");
        let step = dial.read(knob.at, volume);
        assert_eq!(step.volume, None);
        assert!(step.dial.is_ready());
    }

    #[test]
    fn a_dial_that_a_hand_turns_before_it_was_put_does_not_set_the_volume() {
        let mut knob = Knob {
            low: 0,
            high: 24,
            at: 0,
        };
        let mut dial = Dial::Fresh;
        let mut volume = Some(200);
        // Measured, at the bottom, not put yet.
        while !matches!(dial, Dial::Tracking { .. }) {
            read(&mut dial, &mut knob, &mut volume);
        }
        knob.turn(1);
        let step = read(&mut dial, &mut knob, &mut volume);
        assert_eq!(step.volume, None);
        assert_eq!(volume, Some(200));
        settle_down(&mut dial, &mut knob, &mut volume);
        assert_eq!(volume, Some(200));
    }

    #[test]
    fn a_reading_too_far_to_be_a_hand_does_not_set_the_volume() {
        let mut knob = Knob {
            low: -100,
            high: 100,
            at: 0,
        };
        let mut dial = Dial::Fresh;
        let mut volume = Some(128);
        settle_down(&mut dial, &mut knob, &mut volume);
        knob.turn(40);
        let step = read(&mut dial, &mut knob, &mut volume);
        assert_eq!(step.volume, None);
        assert_eq!(volume, Some(128));
        // It is put back where it says the volume.
        settle_down(&mut dial, &mut knob, &mut volume);
        assert_eq!(knob.at, place(-100, 100, 128));
    }

    #[test]
    fn a_volume_that_a_fader_leaves_a_little_off_does_not_move_the_dial() {
        let mut knob = Knob {
            low: 0,
            high: 24,
            at: 0,
        };
        let mut dial = Dial::Fresh;
        let mut volume = Some(100);
        settle_down(&mut dial, &mut knob, &mut volume);
        let at = knob.at;
        volume = Some(104);
        let step = read(&mut dial, &mut knob, &mut volume);
        assert_eq!(step.put, None);
        assert_eq!(knob.at, at);
        // A hand still sets the volume from where the dial is.
        knob.turn(1);
        read(&mut dial, &mut knob, &mut volume);
        assert_eq!(volume, Some(volume_at(0, 24, at + 1)));
    }

    #[test]
    fn a_dial_that_does_not_go_where_it_is_put_is_only_followed() {
        let mut knob = Knob {
            low: -24,
            high: 24,
            at: 0,
        };
        let mut dial = Dial::Fresh;
        let mut readings = 0;
        // The device hears nothing: it reports the same position.
        while !dial.is_ready() {
            let step = dial.read(knob.at, Some(100));
            dial = step.dial;
            readings += 1;
            assert!(readings < 30);
        }
        assert!(matches!(dial, Dial::Free { .. }), "{dial:?}");
        knob.turn(3);
        assert_eq!(dial.read(knob.at, Some(100)).notches, 3);

        // The same for a dial that was measured, then is not put where the
        // volume is, twice in a row.
        let mut dial = Dial::Tracking {
            low: 0,
            high: 24,
            last: 0,
            sent: None,
            doubts: 0,
            synced: false,
            seen: 0,
        };
        let step = dial.read(0, Some(200));
        let want = place(0, 24, 200);
        assert_eq!(step.put, Some(want));
        dial = step.dial;
        // Told, not obeyed: told again, once.
        let step = dial.read(0, Some(200));
        assert_eq!(step.put, Some(want));
        // Not obeyed again: it is the device.
        let step = step.dial.read(0, Some(200));
        assert!(matches!(step.dial, Dial::Free { last: 0 }));
        assert_eq!(step.put, None);
    }

    #[test]
    fn a_reading_too_far_to_be_a_hand_is_not_followed_by_a_dial_only_followed() {
        let dial = Dial::Free { last: 0 };
        assert_eq!(dial.read(30, None).notches, 0);
        assert_eq!(dial.read(24, None).notches, 24);
        assert_eq!(Dial::Free { last: 5 }.read(-3, None).notches, -8);
    }

    #[test]
    fn a_dial_felt_for_too_long_is_given_up_on() {
        let mut dial = Dial::Feeling {
            up: true,
            at: 0,
            high: 0,
            asked: Some(8),
            step: 8,
            left: 2,
        };
        // It does what it is asked each time, and the readings run out.
        let mut reading = 8;
        for _ in 0..2 {
            let step = dial.read(reading, Some(100));
            assert_eq!(step.dial.state(), "measuring");
            reading = step.put.unwrap();
            dial = step.dial;
        }
        let step = dial.read(reading, Some(100));
        assert_eq!(step.dial.state(), "followOnly");
        assert_eq!((step.notches, step.put), (0, None));
    }

    #[test]
    fn a_dial_tells_what_it_is_doing() {
        assert_eq!(Dial::Fresh.state(), "waiting");
        let step = Dial::Fresh.read(0, Some(100));
        assert_eq!(
            (step.dial.state(), step.dial.asked()),
            ("measuring", Some(8))
        );
        let tracking = |synced| Dial::Tracking {
            low: 0,
            high: 36,
            last: 18,
            sent: None,
            doubts: 0,
            synced,
            seen: 100,
        };
        assert_eq!(
            (tracking(true).state(), tracking(true).travel()),
            ("ready", Some((0, 36)))
        );
        assert_eq!(tracking(false).state(), "syncing");
    }
}
