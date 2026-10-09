//! Follows a dial that has a job, and keeps it from running out of travel.
//!
//! A dial reports where it stands, and the device keeps it within the travel
//! it has: turned to the end, it says the same position however far the
//! finger goes on. The app follows how far it turns, and puts the dial back
//! to the middle of its travel when it strays, so that it never gets there.
//! How far the travel goes is not told: it is felt for, once, when the dial is
//! given a job.

/// A dial that seems to have moved further than this between two readings did
/// not: it is a reading to ignore.
const MAX_JUMP: i16 = 24;

/// How far a dial is first asked to go at each reading while its travel is
/// felt for.
const FEEL_STEP: i8 = 8;

/// Readings a dial is felt for at most: about four seconds. After that it is
/// only followed, whatever it did meanwhile.
const FEEL_READINGS: u8 = 80;

/// A travel narrower than this is not one worth putting the dial back in.
const LEAST_TRAVEL: i16 = 4;

/// Where a dial is in its life with the app.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dial {
    /// Not seen yet since it was given a job: the next reading is only where
    /// it stands.
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
    /// Its travel is known: it is followed, and put back to the middle when
    /// it strays.
    Centred {
        low: i8,
        high: i8,
        last: i8,
        /// It was just put back: the next reading is expected near the
        /// middle. If it is where the dial was before, the device did not
        /// take it.
        expecting: bool,
        before: i8,
        /// Readings in a row that looked like a dial that was not put.
        doubts: u8,
    },
    /// It cannot be put back: it is followed by how far it turns, and stops
    /// where the device stops it.
    Free { last: i8 },
}

/// What a reading makes of a dial.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Step {
    pub dial: Dial,
    /// How far it was turned: positive is up.
    pub notches: i16,
    /// Where it is to be put.
    pub put: Option<i8>,
}

impl Step {
    fn stay(dial: Dial) -> Self {
        Self {
            dial,
            notches: 0,
            put: None,
        }
    }
}

fn middle(low: i8, high: i8) -> i8 {
    ((i16::from(low) + i16::from(high)).div_euclid(2)) as i8
}

fn turned(last: i8, now: i8) -> i16 {
    let notches = i16::from(now) - i16::from(last);
    if notches.abs() > MAX_JUMP { 0 } else { notches }
}

impl Dial {
    /// Whether it can be followed.
    #[cfg(test)]
    pub fn is_ready(self) -> bool {
        matches!(self, Self::Centred { .. } | Self::Free { .. })
    }

    /// What the dial is doing, for whoever looks: `waiting`, `measuring`,
    /// `ready` or `followOnly`.
    pub fn state(self) -> &'static str {
        match self {
            Self::Fresh => "waiting",
            Self::Feeling { .. } => "measuring",
            Self::Centred { .. } => "ready",
            Self::Free { .. } => "followOnly",
        }
    }

    /// The travel found, lowest and highest.
    pub fn travel(self) -> Option<(i8, i8)> {
        match self {
            Self::Centred { low, high, .. } => Some((low, high)),
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

    /// Takes a reading of the dial.
    pub fn read(self, now: i8) -> Step {
        match self {
            Self::Fresh => feel(true, now, now, None, FEEL_STEP, FEEL_READINGS, now),
            Self::Feeling {
                up,
                at,
                high,
                asked,
                step,
                left,
            } => feel(up, at, high, asked, step, left, now),
            Self::Centred {
                low,
                high,
                last,
                expecting,
                before,
                doubts,
            } => {
                let centre = middle(low, high);
                let quarter = ((i16::from(high) - i16::from(low)) / 4).max(1);
                let off = (i16::from(now) - i16::from(centre)).abs();
                // Put back from the end of its travel and still there: the
                // device did not take it, or a hand went straight back. Twice
                // in a row, it is the device.
                let suspect = expecting && now == before && (now <= low || now >= high);
                if suspect && doubts >= 1 {
                    return Step::stay(Self::Free { last: now });
                }
                let notches = if suspect { 0 } else { turned(last, now) };
                let strayed = off > quarter;
                Step {
                    dial: Self::Centred {
                        low,
                        high,
                        last: if strayed { centre } else { now },
                        expecting: strayed,
                        before: now,
                        doubts: if suspect { doubts + 1 } else { 0 },
                    },
                    notches,
                    put: strayed.then_some(centre),
                }
            }
            Self::Free { last } => Step {
                dial: Self::Free { last: now },
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
    let centre = middle(low, high);
    Step {
        dial: Dial::Centred {
            low,
            high,
            last: centre,
            expecting: true,
            before: at,
            doubts: 0,
        },
        notches: 0,
        put: Some(centre),
    }
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

    /// Reads the knob until the dial is ready, doing what it asks.
    fn settle(dial: &mut Dial, knob: &mut Knob) -> usize {
        let mut readings = 0;
        while !dial.is_ready() {
            let step = dial.read(knob.at);
            assert_eq!(step.notches, 0, "nothing is followed while it is felt for");
            if let Some(value) = step.put {
                knob.put(value);
            }
            *dial = step.dial;
            readings += 1;
            assert!(readings < 100, "never settled: {dial:?}");
        }
        readings
    }

    #[test]
    fn a_dial_finds_its_travel_and_goes_to_the_middle_of_it() {
        for (low, high, start) in [
            (-24, 24, 17),
            (-12, 12, -12),
            (0, 36, 0),
            (0, 36, 36),
            (-3, 5, 1),
        ] {
            let mut knob = Knob {
                low,
                high,
                at: start,
            };
            let mut dial = Dial::Fresh;
            settle(&mut dial, &mut knob);
            let Dial::Centred {
                low: found_low,
                high: found_high,
                ..
            } = dial
            else {
                panic!("{dial:?} for {low}..{high}");
            };
            assert_eq!((found_low, found_high), (low, high));
            assert_eq!(
                i16::from(knob.at),
                (i16::from(low) + i16::from(high)).div_euclid(2)
            );
        }
    }

    #[test]
    fn a_dial_followed_never_runs_out_of_travel() {
        for (low, high) in [(-24, 24), (0, 36), (-12, 12)] {
            let mut knob = Knob { low, high, at: low };
            let mut dial = Dial::Fresh;
            settle(&mut dial, &mut knob);
            let mut total = 0;
            for _ in 0..40 {
                knob.turn(5);
                let step = dial.read(knob.at);
                total += step.notches;
                if let Some(value) = step.put {
                    knob.put(value);
                }
                dial = step.dial;
            }
            assert_eq!(total, 200, "{low}..{high}");
            for _ in 0..40 {
                knob.turn(-5);
                let step = dial.read(knob.at);
                total += step.notches;
                if let Some(value) = step.put {
                    knob.put(value);
                }
                dial = step.dial;
            }
            assert_eq!(total, 0, "{low}..{high}");
        }
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
            let step = dial.read(knob.at);
            dial = step.dial;
            readings += 1;
            assert!(readings < 20);
        }
        assert!(matches!(dial, Dial::Free { .. }), "{dial:?}");
        knob.turn(3);
        assert_eq!(dial.read(knob.at).notches, 3);

        // A dial that was put back and did not go gives up on it.
        let centred = Dial::Centred {
            low: -24,
            high: 24,
            last: 0,
            expecting: true,
            before: 24,
            doubts: 0,
        };
        // Once, it may be a hand that went straight back: put again.
        let step = centred.read(24);
        assert_eq!((step.notches, step.put), (0, Some(0)));
        // Twice, it is the device.
        let step = step.dial.read(24);
        assert!(matches!(step.dial, Dial::Free { last: 24 }));
        assert_eq!((step.notches, step.put), (0, None));

        // A hand that went far in the instant after it was put back is not a
        // failure: it is followed from the middle, and the dial is put back
        // again.
        let step = centred.read(15);
        assert_eq!((step.notches, step.put), (15, Some(0)));
        assert!(matches!(
            step.dial,
            Dial::Centred {
                expecting: true,
                ..
            }
        ));
    }

    #[test]
    fn a_reading_too_far_to_be_a_hand_is_not_followed() {
        let dial = Dial::Free { last: 0 };
        assert_eq!(dial.read(30).notches, 0);
        assert_eq!(dial.read(24).notches, 24);
        assert_eq!(Dial::Free { last: 5 }.read(-3).notches, -8);
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
            let step = dial.read(reading);
            assert_eq!(step.dial.state(), "measuring");
            reading = step.put.unwrap();
            dial = step.dial;
        }
        let step = dial.read(reading);
        assert_eq!(step.dial.state(), "followOnly");
        assert_eq!((step.notches, step.put), (0, None));

        // A fresh dial is given the whole of the time.
        let mut dial = Dial::Fresh;
        let mut reading = 0i8;
        for _ in 0..u32::from(FEEL_READINGS) {
            let step = dial.read(reading);
            dial = step.dial;
            if let Some(put) = step.put {
                reading = put;
            }
            if dial.is_ready() {
                break;
            }
        }
        assert!(dial.is_ready(), "{dial:?}");
    }

    #[test]
    fn a_dial_tells_what_it_is_doing() {
        assert_eq!(Dial::Fresh.state(), "waiting");
        let step = Dial::Fresh.read(0);
        assert_eq!(
            (step.dial.state(), step.dial.asked()),
            ("measuring", Some(8))
        );
        let ready = Dial::Centred {
            low: 0,
            high: 36,
            last: 18,
            expecting: false,
            before: 18,
            doubts: 0,
        };
        assert_eq!((ready.state(), ready.travel()), ("ready", Some((0, 36))));
    }

    #[test]
    fn the_first_reading_is_only_where_the_dial_stands() {
        let step = Dial::Fresh.read(10);
        assert_eq!(step.notches, 0);
        assert_eq!(step.put, Some(18));
    }
}
