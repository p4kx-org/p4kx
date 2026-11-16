// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_archive::{Archive, ObservedEntry, ObservedEntryBinding};
use p4kx_audio::BankError;
use p4kx_extraction::{
    FieldOrigin, InputScope, SourceCoordinateOmission, SourceEvidenceError, SourceInput,
    SourceUseRole, SourceUses,
};
use thiserror::Error;

use super::hirc_path_source::HircInboundSource;
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
    /// A localised sound has no WEM in the language asked for or in the fallback language.
    #[error("{}", no_localisation(.language, .fallback))]
    NoLocalisation {
        /// The language asked for, empty when none was.
        language: Box<str>,
        /// The fallback language, empty when there is none.
        fallback: Box<str>,
    },
    /// The trigger has more visits than the caller's budget allows.
    #[error("the trigger exceeds the visit budget of {limit}, leaving references unfollowed")]
    TooManyObjects {
        /// The limit.
        limit: usize,
    },
}

impl BreakReason {
    /// Returns whether a traversal limit left authored routes unvisited.
    #[must_use]
    pub const fn truncated_by_limit(&self) -> bool {
        matches!(self, Self::TooManyObjects { .. })
    }

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
    pub(super) bank_source: Option<ObservedEntryBinding>,
    pub(crate) path: Box<[PathStep]>,
    pub(super) path_sources: Box<[super::HircPathSource]>,
    pub(super) failed_edge: Option<HircInboundSource>,
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

    /// Returns the original HIRC sources in path order when requested.
    #[must_use]
    pub fn path_sources(&self) -> &[super::HircPathSource] {
        &self.path_sources
    }

    /// Returns the object the way breaks at, the last step of the path.
    #[must_use]
    pub fn object(&self) -> Option<PathStep> {
        self.path.last().copied()
    }

    /// Returns the exact observed soundbank Entry associated with this break.
    #[must_use]
    pub fn entry<'a>(&self, archive: &'a Archive) -> Option<ObservedEntry<'a>> {
        let binding = self.bank_source.as_ref()?;
        let entry =
            archive.observed_entry(archive.entries().get(self.soundbank?.entry_position())?)?;
        binding.is_same_entry(&entry.binding()).then_some(entry)
    }

    /// Registers the authored edge selector and any actual unreadable bank attempt.
    ///
    /// The returned field origin is incomplete when the authored edge was not captured.
    ///
    /// # Errors
    /// Returns an error if the provenance registry cannot retain the observations.
    pub fn failed_edge_origin(
        &self,
        archive: &Archive,
        uses: &SourceUses,
    ) -> Result<FieldOrigin, SourceEvidenceError> {
        if matches!(self.reason, BreakReason::Unreadable(_))
            && let Some(entry) = self.entry(archive)
        {
            let scope = uses.observe(SourceInput::ArchiveView(entry), InputScope::Payload)?;
            scope.at_opaque(
                SourceUseRole::Attempted,
                SourceCoordinateOmission::Unavailable,
            )?;
        }
        self.failed_edge.as_ref().map_or_else(
            || Ok(uses.origin(&[], false)),
            |edge| edge.origin(archive, uses),
        )
    }

    /// Returns why the way breaks.
    #[must_use]
    pub const fn reason(&self) -> &BreakReason {
        &self.reason
    }
}
