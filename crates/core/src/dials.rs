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

/// How far a dial is asked to go at each reading while its travel is felt for.
const FEEL_STEP: i8 = 8;

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

    /// Takes a reading of the dial.
    pub fn read(self, now: i8) -> Step {
        match self {
            Self::Fresh => feel(true, now, None, now),
            Self::Feeling {
                up, high, asked, ..
            } => feel(up, high, asked, now),
            Self::Centred {
                low,
                high,
                last,
                expecting,
                before,
            } => {
                let centre = middle(low, high);
                let quarter = ((i16::from(high) - i16::from(low)) / 4).max(1);
                let off = (i16::from(now) - i16::from(centre)).abs();
                if expecting && now == before && (now <= low || now >= high) {
                    // It was put back from the end of its travel and did not
                    // move: it cannot be put.
                    return Step::stay(Self::Free { last: now });
                }
                let notches = turned(last, now);
                let strayed = off > quarter;
                Step {
                    dial: Self::Centred {
                        low,
                        high,
                        last: if strayed { centre } else { now },
                        expecting: strayed,
                        before: now,
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
fn further(up: bool, from: i8) -> Option<i8> {
    let to = if up {
        from.saturating_add(FEEL_STEP)
    } else {
        from.saturating_sub(FEEL_STEP)
    };
    (to != from).then_some(to)
}

fn feel(up: bool, high: i8, asked: Option<i8>, now: i8) -> Step {
    // Where it went, when it was asked to go further. Short of it, that is
    // the end of its travel.
    let (at, end) = match asked {
        Some(asked) if now == asked => (now, false),
        Some(_) => (now, true),
        None => (now, false),
    };
    if !end && let Some(to) = further(up, at) {
        return Step {
            dial: Dial::Feeling {
                up,
                at,
                high,
                asked: Some(to),
            },
            notches: 0,
            put: Some(to),
        };
    }
    if up {
        // The top is found: the bottom is next, from here.
        return match further(false, at) {
            Some(to) => Step {
                dial: Dial::Feeling {
                    up: false,
                    at,
                    high: at,
                    asked: Some(to),
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
        };
        let step = centred.read(24);
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
    fn the_first_reading_is_only_where_the_dial_stands() {
        let step = Dial::Fresh.read(10);
        assert_eq!(step.notches, 0);
        assert_eq!(step.put, Some(18));
    }
}
