//! What can be wrong with a fight spec. Every error is a value; the arena never panics on a bad spec.

use std::fmt;

#[derive(Clone, Debug, PartialEq)]
pub enum ArenaError {
    /// The spec's RON text does not read (with line and column).
    Parse(String),
    /// A number out of range: finite, above zero, at most `max`.
    Number {
        field: &'static str,
        value: f32,
        max: f32,
    },
    EmptySide {
        side: usize,
    },
    ZeroCount {
        side: usize,
        entry: usize,
    },
    TooManyShips {
        side: usize,
        ships: u32,
    },
    /// No ship of this id in the content.
    UnknownShip {
        side: usize,
        entry: usize,
        id: String,
    },
    /// A blueprint text that does not read, fails the content's checks or does not compile.
    Blueprint {
        side: usize,
        entry: usize,
        problems: Vec<String>,
    },
    /// No behavior of this id in the content.
    UnknownBehavior {
        side: usize,
        entry: usize,
        id: String,
    },
    /// A behavior text that does not parse or check, by text line.
    BehaviorText {
        side: usize,
        entry: usize,
        errors: Vec<String>,
    },
    /// The entry names no behavior and its design has none (neither its own nor its faction's).
    NoBehavior {
        side: usize,
        entry: usize,
        design: String,
    },
    /// A second stand-in pilot on a side, or one in a reinforcement wave.
    Pilot {
        side: usize,
        entry: usize,
    },
    /// The content's own behaviors do not compile.
    Content(String),
    /// No voyage of this id in the content.
    UnknownVoyage(String),
    /// A voyage run cannot start: its design or behavior is not usable, or the voyage does not roll.
    Voyage(String),
}

impl fmt::Display for ArenaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let at = |f: &mut fmt::Formatter<'_>, side: &usize, entry: &usize| {
            write!(f, "{}, entry {entry}: ", who(*side))
        };
        match self {
            ArenaError::Parse(e) => write!(f, "the fight spec does not read: {e}"),
            ArenaError::Number { field, value, max } => {
                write!(f, "`{field}` is {value}; it must be above 0 and at most {max}")
            }
            ArenaError::EmptySide { side } => write!(f, "{} has no entries", who(*side)),
            ArenaError::ZeroCount { side, entry } => {
                at(f, side, entry)?;
                f.write_str("`count` is 0")
            }
            ArenaError::TooManyShips { side, ships } => write!(
                f,
                "{} fields {ships} ships, at most {}",
                who(*side),
                crate::spec::MAX_SHIPS_PER_SIDE
            ),
            ArenaError::UnknownShip { side, entry, id } => {
                at(f, side, entry)?;
                write!(f, "no ship `{id}` in the content")
            }
            ArenaError::Blueprint {
                side,
                entry,
                problems,
            } => {
                at(f, side, entry)?;
                write!(f, "the blueprint is not usable: {}", problems.join("; "))
            }
            ArenaError::UnknownBehavior { side, entry, id } => {
                at(f, side, entry)?;
                write!(f, "no behavior `{id}` in the content")
            }
            ArenaError::BehaviorText {
                side,
                entry,
                errors,
            } => {
                at(f, side, entry)?;
                write!(f, "the behavior text is not usable: {}", errors.join("; "))
            }
            ArenaError::NoBehavior {
                side,
                entry,
                design,
            } => {
                at(f, side, entry)?;
                write!(
                    f,
                    "`{design}` has no default behavior (neither its own nor its faction's); name one"
                )
            }
            ArenaError::Pilot { side, entry } => {
                at(f, side, entry)?;
                f.write_str("a side has one stand-in pilot at most, and a reinforcement wave none")
            }
            ArenaError::Content(e) => write!(f, "the content's behaviors do not compile: {e}"),
            ArenaError::UnknownVoyage(id) => write!(f, "no voyage `{id}` in the content"),
            ArenaError::Voyage(e) => write!(f, "the voyage run cannot start: {e}"),
        }
    }
}

/// A side in words: side 0 or 1, or a gauntlet's reinforcements (numbered from 2 in the errors: wave 1 is 2).
fn who(side: usize) -> String {
    match side {
        0 | 1 => format!("side {side}"),
        k => format!("reinforcement wave {}", k - 1),
    }
}

impl std::error::Error for ArenaError {}
