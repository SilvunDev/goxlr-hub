//! Tells a short press from a long one, a double press from two short ones,
//! and a hold from all of them, from nothing but which buttons are down at
//! each reading of the device.

use std::time::{Duration, Instant};

use goxlr_hub_protocol::{Button, ButtonSet};
use serde::{Deserialize, Serialize};

/// What a press of a button can mean.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Gesture {
    /// A quick press and release.
    Short,
    /// A press kept down for a moment.
    Long,
    /// Two quick presses.
    Double,
    /// The press itself: the action lasts as long as the button is down.
    Hold,
}

impl Gesture {
    pub const ALL: [Self; 4] = [Self::Short, Self::Long, Self::Double, Self::Hold];
}

/// What happened to a button.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    /// A gesture that is over: its action is due.
    Fire(Gesture),
    /// The button of a hold went down.
    HoldStart,
    /// The button of a hold came back up.
    HoldEnd,
}

/// What a button has an action for. The others are not waited for.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Wanted {
    pub long: bool,
    pub double: bool,
    pub hold: bool,
}

/// How long a press must last to be a long one, and how long a second press
/// is waited for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Timing {
    pub long: Duration,
    pub double: Duration,
}

/// The time, as a count from some start. Replaceable, so that tests do not
/// wait.
pub trait Clock: Send {
    fn now(&self) -> Duration;
}

/// The clock of the computer.
pub struct SystemClock(Instant);

impl Default for SystemClock {
    fn default() -> Self {
        Self(Instant::now())
    }
}

impl Clock for SystemClock {
    fn now(&self) -> Duration {
        self.0.elapsed()
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum State {
    #[default]
    Idle,
    /// Down when the device was taken over: no press, wait for it to come up.
    Ignored,
    /// Down, and it may become a long press.
    Down { since: Duration },
    /// A hold started: its end is due.
    Holding,
    /// Came up quickly and a second press is waited for.
    Waiting { until: Duration },
    /// Its gesture was told already: wait for it to come up.
    Spent,
}

/// Follows every button from reading to reading.
pub struct Recognizer {
    states: [State; Button::ALL.len()],
    started: bool,
}

impl Default for Recognizer {
    fn default() -> Self {
        Self {
            states: [State::Idle; Button::ALL.len()],
            started: false,
        }
    }
}

impl Recognizer {
    /// What the buttons do has changed: a gesture begun under the old
    /// buttons must not end under the new ones. A press that was waited on
    /// for a second one is dropped, a button still going down is spent until
    /// it comes up. A hold goes on: it ends as it began.
    pub fn buttons_changed(&mut self) {
        for state in &mut self.states {
            *state = match *state {
                State::Waiting { .. } => State::Idle,
                State::Down { .. } => State::Spent,
                other => other,
            };
        }
    }

    /// Takes a reading of the device and says what it means for each button.
    /// Meant to be called at every reading, even when nothing moved: waiting
    /// is part of a gesture.
    pub fn update(
        &mut self,
        now: Duration,
        pressed: ButtonSet,
        wanted: impl Fn(Button) -> Wanted,
        timing: Timing,
    ) -> Vec<(Button, Event)> {
        let first = !std::mem::replace(&mut self.started, true);
        let mut events = Vec::new();
        for (at, button) in Button::ALL.into_iter().enumerate() {
            let down = pressed.contains(button);
            // Buttons already down when the device is taken over are no press.
            if first && down {
                self.states[at] = State::Ignored;
                continue;
            }
            let wanted = wanted(button);
            let mut tell = |event| events.push((button, event));

            // A second press that never came: the first one stood alone.
            if let State::Waiting { until } = self.states[at]
                && now >= until
            {
                tell(Event::Fire(Gesture::Short));
                self.states[at] = State::Idle;
            }

            self.states[at] = match (self.states[at], down) {
                (State::Idle, true) if wanted.hold => {
                    tell(Event::HoldStart);
                    State::Holding
                }
                // Nothing to wait for: it is a short press as soon as it goes down.
                (State::Idle, true) if !wanted.long && !wanted.double => {
                    tell(Event::Fire(Gesture::Short));
                    State::Spent
                }
                (State::Idle, true) => State::Down { since: now },
                (State::Down { since }, true) => {
                    if wanted.long && now.saturating_sub(since) >= timing.long {
                        tell(Event::Fire(Gesture::Long));
                        State::Spent
                    } else {
                        State::Down { since }
                    }
                }
                (State::Down { .. }, false) => {
                    if wanted.double {
                        State::Waiting {
                            until: now + timing.double,
                        }
                    } else {
                        tell(Event::Fire(Gesture::Short));
                        State::Idle
                    }
                }
                (State::Waiting { .. }, true) => {
                    tell(Event::Fire(Gesture::Double));
                    State::Spent
                }
                (State::Holding, false) => {
                    tell(Event::HoldEnd);
                    State::Idle
                }
                (State::Ignored | State::Spent, false) => State::Idle,
                (state, _) => state,
            };
        }
        events
    }
}

/// A clock that only moves when told to, for tests.
#[cfg(test)]
#[derive(Clone, Default)]
pub struct ManualClock(std::sync::Arc<std::sync::atomic::AtomicU64>);

#[cfg(test)]
impl ManualClock {
    pub fn advance(&self, by: Duration) {
        self.0.fetch_add(
            u64::try_from(by.as_millis()).unwrap(),
            std::sync::atomic::Ordering::SeqCst,
        );
    }
}

#[cfg(test)]
impl Clock for ManualClock {
    fn now(&self) -> Duration {
        Duration::from_millis(self.0.load(std::sync::atomic::Ordering::SeqCst))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TIMING: Timing = Timing {
        long: Duration::from_millis(500),
        double: Duration::from_millis(333),
    };

    const PAD: Button = Button::SamplerTopLeft;
    const OTHER: Button = Button::SamplerTopRight;

    /// A device read at chosen times, with the actions of the buttons.
    struct Bench {
        recognizer: Recognizer,
        pressed: ButtonSet,
        wanted: Wanted,
    }

    impl Bench {
        fn new(wanted: Wanted) -> Self {
            let mut bench = Self {
                recognizer: Recognizer::default(),
                pressed: ButtonSet::default(),
                wanted,
            };
            // The first reading, with nothing down.
            bench.read(0);
            bench
        }

        fn read(&mut self, ms: u64) -> Vec<(Button, Event)> {
            self.recognizer.update(
                Duration::from_millis(ms),
                self.pressed,
                |_| self.wanted,
                TIMING,
            )
        }

        fn down(&mut self, button: Button, ms: u64) -> Vec<(Button, Event)> {
            self.pressed.insert(button);
            self.read(ms)
        }

        fn up(&mut self, button: Button, ms: u64) -> Vec<(Button, Event)> {
            self.pressed.remove(button);
            self.read(ms)
        }
    }

    const NOTHING: Wanted = Wanted {
        long: false,
        double: false,
        hold: false,
    };
    const LONG: Wanted = Wanted {
        long: true,
        ..NOTHING
    };
    const DOUBLE: Wanted = Wanted {
        double: true,
        ..NOTHING
    };
    const BOTH: Wanted = Wanted {
        long: true,
        double: true,
        hold: false,
    };
    const HOLD: Wanted = Wanted {
        hold: true,
        ..NOTHING
    };

    fn short(button: Button) -> (Button, Event) {
        (button, Event::Fire(Gesture::Short))
    }

    fn fired(button: Button, gesture: Gesture) -> (Button, Event) {
        (button, Event::Fire(gesture))
    }

    #[test]
    fn a_button_with_nothing_to_wait_for_is_short_as_soon_as_it_goes_down() {
        let mut bench = Bench::new(NOTHING);
        assert_eq!(bench.down(PAD, 100), [short(PAD)]);
        // Nothing more however long it is held, and nothing on release.
        assert_eq!(bench.read(2000), []);
        assert_eq!(bench.up(PAD, 2100), []);
        assert_eq!(bench.down(PAD, 2200), [short(PAD)]);
    }

    #[test]
    fn a_button_with_a_long_press_is_short_when_it_comes_up_quickly() {
        let mut bench = Bench::new(LONG);
        assert_eq!(bench.down(PAD, 100), []);
        assert_eq!(bench.read(300), []);
        assert_eq!(bench.up(PAD, 350), [short(PAD)]);
    }

    #[test]
    fn a_press_that_lasts_is_long_once_and_says_nothing_on_release() {
        let mut bench = Bench::new(LONG);
        bench.down(PAD, 100);
        assert_eq!(bench.read(599), []);
        assert_eq!(bench.read(600), [fired(PAD, Gesture::Long)]);
        assert_eq!(bench.read(1500), []);
        assert_eq!(bench.up(PAD, 1600), []);
        // The next press starts afresh.
        bench.down(PAD, 2000);
        assert_eq!(bench.up(PAD, 2100), [short(PAD)]);
    }

    #[test]
    fn a_long_press_is_not_waited_for_when_there_is_no_action_for_it() {
        let mut bench = Bench::new(DOUBLE);
        bench.down(PAD, 100);
        assert_eq!(bench.read(900), []);
        // It is a short press after all, told once the second press is no
        // longer waited for.
        assert_eq!(bench.up(PAD, 1000), []);
        assert_eq!(bench.read(1332), []);
        assert_eq!(bench.read(1333), [short(PAD)]);
    }

    #[test]
    fn two_quick_presses_are_a_double_press() {
        let mut bench = Bench::new(DOUBLE);
        assert_eq!(bench.down(PAD, 100), []);
        assert_eq!(bench.up(PAD, 150), []);
        // Told at the second press, not at its release.
        assert_eq!(bench.down(PAD, 300), [fired(PAD, Gesture::Double)]);
        assert_eq!(bench.up(PAD, 350), []);
        // Nothing else follows, however long one waits.
        assert_eq!(bench.read(2000), []);
    }

    #[test]
    fn a_press_alone_is_short_a_third_of_a_second_after_it_came_up() {
        let mut bench = Bench::new(DOUBLE);
        bench.down(PAD, 100);
        assert_eq!(bench.up(PAD, 150), []);
        assert_eq!(bench.read(482), []);
        assert_eq!(bench.read(483), [short(PAD)]);
        assert_eq!(bench.read(900), []);
    }

    #[test]
    fn two_presses_too_far_apart_are_two_short_presses() {
        let mut bench = Bench::new(DOUBLE);
        bench.down(PAD, 100);
        bench.up(PAD, 150);
        // The reading that sees the second press is late: the first press
        // stood alone, and this one begins another.
        assert_eq!(bench.down(PAD, 600), [short(PAD)]);
        assert_eq!(bench.up(PAD, 650), []);
        assert_eq!(bench.read(983), [short(PAD)]);
    }

    #[test]
    fn long_and_double_live_on_one_button() {
        let mut bench = Bench::new(BOTH);
        // Quick, quick: double.
        bench.down(PAD, 100);
        bench.up(PAD, 150);
        assert_eq!(bench.down(PAD, 250), [fired(PAD, Gesture::Double)]);
        bench.up(PAD, 300);

        // Held: long, and no double waited for afterwards.
        bench.down(PAD, 1000);
        assert_eq!(bench.read(1500), [fired(PAD, Gesture::Long)]);
        assert_eq!(bench.up(PAD, 1600), []);
        assert_eq!(bench.read(2500), []);

        // One quick press: short, after the wait.
        bench.down(PAD, 3000);
        bench.up(PAD, 3050);
        assert_eq!(bench.read(3383), [short(PAD)]);
    }

    #[test]
    fn a_hold_starts_when_the_finger_lands_and_ends_when_it_leaves() {
        let mut bench = Bench::new(HOLD);
        assert_eq!(bench.down(PAD, 100), [(PAD, Event::HoldStart)]);
        assert_eq!(bench.read(5000), []);
        assert_eq!(bench.up(PAD, 5100), [(PAD, Event::HoldEnd)]);
        assert_eq!(bench.read(6000), []);
        // A tap shorter than a reading still ends.
        assert_eq!(bench.down(PAD, 7000), [(PAD, Event::HoldStart)]);
        assert_eq!(bench.up(PAD, 7050), [(PAD, Event::HoldEnd)]);
    }

    #[test]
    fn two_buttons_at_once_each_follow_their_own_path() {
        let mut bench = Bench::new(BOTH);
        bench.down(PAD, 100);
        bench.down(OTHER, 200);
        assert_eq!(bench.read(600), [fired(PAD, Gesture::Long)]);
        assert_eq!(bench.up(OTHER, 650), []);
        assert_eq!(bench.up(PAD, 700), []);
        assert_eq!(bench.read(983), [short(OTHER)]);
    }

    #[test]
    fn a_button_down_when_the_device_is_taken_over_is_no_press() {
        let mut recognizer = Recognizer::default();
        let mut pressed = ButtonSet::default();
        pressed.insert(PAD);
        let mut read = |ms: u64, pressed: ButtonSet| {
            recognizer.update(Duration::from_millis(ms), pressed, |_| NOTHING, TIMING)
        };
        assert_eq!(read(0, pressed), []);
        assert_eq!(read(100, pressed), []);
        assert_eq!(read(200, ButtonSet::default()), []);
        pressed.remove(PAD);
        pressed.insert(PAD);
        assert_eq!(read(300, pressed), [short(PAD)]);
    }

    #[test]
    fn what_a_button_is_for_may_change_in_the_middle_of_a_press() {
        let mut recognizer = Recognizer::default();
        let mut pressed = ButtonSet::default();
        let mut read = |ms: u64, pressed: ButtonSet, wanted: Wanted| {
            recognizer.update(Duration::from_millis(ms), pressed, |_| wanted, TIMING)
        };
        read(0, pressed, NOTHING);
        pressed.insert(PAD);
        assert_eq!(read(100, pressed, LONG), []);
        // The long press is taken away: the press is short, once.
        assert_eq!(read(900, pressed, NOTHING), []);
        pressed.remove(PAD);
        assert_eq!(read(1000, pressed, NOTHING), [short(PAD)]);
        assert_eq!(read(1100, pressed, NOTHING), []);

        // A hold is taken away while held: its end is still told.
        pressed.insert(PAD);
        assert_eq!(read(2000, pressed, HOLD), [(PAD, Event::HoldStart)]);
        pressed.remove(PAD);
        assert_eq!(read(2100, pressed, NOTHING), [(PAD, Event::HoldEnd)]);
    }
}
