// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_audio::BankError;
use thiserror::Error;

use super::{AudioFileError, IndexedSoundbank, PathStep, WwiseEvent};

fn no_localisation(language: &str, fallback: &str) -> String {
    match (language.is_empty(), fallback.is_empty()) {
        (true, true) => "no localisation in any language folder".to_owned(),
        (false, false) if !language.eq_ignore_ascii_case(fallback) => {
            format!("no localisation in the language {language}, nor in the fallback {fallback}")
        }
        (false, _) => format!("no localisation in the language {language}"),
        (true, false) => format!("no localisation in the language {fallback}"),
    }
}

/// Why the way from a Wwise event to a WEM breaks.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum BreakReason {
    /// No soundbank holds the Wwise event.
    #[error("no soundbank holds the Wwise event")]
    NoSoundbank,
    /// The soundbank holding the event cannot be read again.
    #[error("the soundbank cannot be read: {0}")]
    Unreadable(AudioFileError),
    /// The object refers to an object the soundbank does not hold.
    #[error("the soundbank holds no object {id}")]
    Missing {
        /// The ID referred to.
        id: u32,
    },
    /// A Play action names a soundbank absent from the eligible indexed candidates.
    #[error("no eligible indexed soundbank {bank_id} holds object {id}")]
    OtherSoundbank {
        /// The ID referred to.
        id: u32,
        /// The ID of the soundbank the action names.
        bank_id: u32,
    },
    /// The object's body does not hold its fields.
    #[error("the object cannot be read: {0}")]
    Malformed(BankError),
    /// The object is a sound or container in a bank version whose layout this reader does not
    /// know.
    #[error("the object's layout in soundbank version {version} is not known")]
    UnknownLayout {
        /// The bank version.
        version: u32,
    },
    /// The object is of a kind that is not followed, e.g. an actor-mixer.
    #[error("objects of this kind are not followed")]
    Unsupported,
    /// The reference returns to the same object in the same soundbank Entry on its path.
    #[error("the object refers back to object {id} on its own path")]
    Cycle {
        /// The ID referred to.
        id: u32,
    },
    /// The path is as long as [`MAX_PATH_DEPTH`](super::MAX_PATH_DEPTH) allows.
    #[error("the path is deeper than {limit} objects")]
    TooDeep {
        /// The limit.
        limit: usize,
    },
    /// `PlayEvent` and `Trigger` actions follow one another more often than
    /// [`MAX_ACTION_CHAIN`](super::MAX_ACTION_CHAIN) allows.
    #[error("more than {limit} PlayEvent and Trigger actions follow one another")]
    ChainTooLong {
        /// The limit.
        limit: usize,
    },
    /// The container has more children than
    /// [`MAX_CONTAINER_CHILDREN`](super::MAX_CONTAINER_CHILDREN); the rest are left out.
    #[error("the container has {count} children; only the first {limit} are followed")]
    TooManyChildren {
        /// How many children it has.
        count: usize,
        /// The limit.
        limit: usize,
    },
    /// A localised sound has no WEM in the language asked for or in the fallback language.
    #[error("{}", no_localisation(.language, .fallback))]
    NoLocalisation {
        /// The language asked for, empty when none was.
        language: Box<str>,
        /// The fallback language, empty when there is none.
        fallback: Box<str>,
    },
    /// The walk followed as many references as its budget allows, at most
    /// [`MAX_EVENT_OBJECTS`](super::MAX_EVENT_OBJECTS) and a share of
    /// [`MAX_TRIGGER_OBJECTS`](super::MAX_TRIGGER_OBJECTS); the rest are left out.
    #[error("the event leads to more than {limit} references; the rest are left out")]
    TooManyObjects {
        /// The limit.
        limit: usize,
    },
}

impl BreakReason {
    /// Returns a stable code for the reason, e.g. `missing`, for scripts.
    #[must_use]
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::NoSoundbank => "no_soundbank",
            Self::Unreadable(_) => "unreadable",
            Self::Missing { .. } => "missing",
            Self::OtherSoundbank { .. } => "other_soundbank",
            Self::Malformed(_) => "malformed",
            Self::UnknownLayout { .. } => "unknown_layout",
            Self::Unsupported => "unsupported",
            Self::Cycle { .. } => "cycle",
            Self::TooDeep { .. } => "too_deep",
            Self::ChainTooLong { .. } => "chain_too_long",
            Self::TooManyChildren { .. } => "too_many_children",
            Self::TooManyObjects { .. } => "too_many_objects",
            Self::NoLocalisation { .. } => "no_localisation",
        }
    }
}

/// A place where the way from a Wwise event to a WEM breaks: the object it breaks at, the path
/// to it and the reason.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrokenLink<'i> {
    pub(crate) event: &'i WwiseEvent,
    pub(crate) soundbank: Option<&'i IndexedSoundbank>,
    pub(crate) path: Box<[PathStep]>,
    pub(crate) reason: BreakReason,
}

impl<'i> BrokenLink<'i> {
    /// Returns the Wwise event the way starts at.
    #[must_use]
    pub const fn event(&self) -> &'i WwiseEvent {
        self.event
    }

    /// Returns the soundbank whose `HIRC` section was followed, or `None` when no soundbank
    /// holds the event.
    #[must_use]
    pub const fn soundbank(&self) -> Option<&'i IndexedSoundbank> {
        self.soundbank
    }

    /// Returns the container path from the Event object to the object the way breaks at.
    #[must_use]
    pub fn path(&self) -> &[PathStep] {
        &self.path
    }

    /// Returns the object the way breaks at, the last step of the path.
    #[must_use]
    pub fn object(&self) -> Option<PathStep> {
        self.path.last().copied()
    }

    /// Returns why the way breaks.
    #[must_use]
    pub const fn reason(&self) -> &BreakReason {
        &self.reason
    }
}
