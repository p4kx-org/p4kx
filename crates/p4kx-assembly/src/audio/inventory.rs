// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_archive::{Archive, ArchiveInputOrigin, Entry, ObservedEntry};

use super::{AudioFileError, AudioIndex};

/// An independently searched family of audio index inputs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum AudioSurveyKind {
    /// Root ATL definition Entries in the index's precedence order.
    AtlDefinitions,
    /// Root soundbank Entries in the index's path order.
    Soundbanks,
}

/// The index's final treatment of one consulted Entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum AudioCandidateState {
    /// The parser and index admitted the candidate.
    Admitted,
    /// The candidate could not be read or parsed.
    Unavailable,
    /// A configured limit prevented full admission of the candidate.
    CapacityLimited,
}

/// An actual audio index candidate in its owning Archive.
#[derive(Debug, Clone, Copy)]
pub struct AudioCandidate<'archive> {
    entry: ObservedEntry<'archive>,
    state: AudioCandidateState,
    decoded_bytes: Option<u64>,
    retained_input: bool,
}

impl<'archive> AudioCandidate<'archive> {
    /// Returns the original candidate Entry.
    #[must_use]
    pub const fn entry(&self) -> ObservedEntry<'archive> {
        self.entry
    }

    /// Returns the actual outcome of consulting the candidate.
    #[must_use]
    pub const fn state(&self) -> AudioCandidateState {
        self.state
    }

    /// Returns whether the index used the read input or retained part of it.
    #[must_use]
    pub const fn has_retained_input(&self) -> bool {
        self.retained_input
    }

    /// Returns the decoded extent supplied to the parser after a completed read.
    #[must_use]
    pub const fn decoded_bytes(&self) -> Option<u64> {
        self.decoded_bytes
    }
}

/// An audio candidate inventory bound to the source root Archive.
#[derive(Debug, Clone, Copy)]
pub struct AudioSurvey<'index, 'archive> {
    archive: &'archive Archive,
    candidates: &'index [Candidate],
    directory_complete: bool,
}

impl<'index, 'archive> AudioSurvey<'index, 'archive> {
    /// Returns whether the original root directory and candidate admissions are complete.
    #[must_use]
    pub fn complete(&self) -> bool {
        self.directory_complete
            && self.candidates.iter().all(|candidate| {
                candidate.state == AudioCandidateState::Admitted
                    && candidate.decoded_bytes.is_some()
            })
    }

    /// Returns the physical candidates in the index's search order.
    pub fn candidates(
        &self,
    ) -> impl Iterator<Item = AudioCandidate<'archive>> + use<'index, 'archive> {
        let archive = self.archive;
        self.candidates.iter().filter_map(move |candidate| {
            let entry = archive.observed_entry(archive.entries().get(candidate.position)?)?;
            Some(AudioCandidate {
                entry,
                state: candidate.state,
                decoded_bytes: candidate.decoded_bytes,
                retained_input: candidate.retained_input,
            })
        })
    }
}

#[derive(Debug)]
pub(super) struct Candidate {
    position: usize,
    state: AudioCandidateState,
    decoded_bytes: Option<u64>,
    retained_input: bool,
}

impl Candidate {
    pub(super) const fn of(
        entry: &Entry,
        error: Option<&AudioFileError>,
        decoded_bytes: Option<u64>,
    ) -> Self {
        let state = match error {
            None => AudioCandidateState::Admitted,
            Some(error) if error.is_capacity_limit() => AudioCandidateState::CapacityLimited,
            Some(_) => AudioCandidateState::Unavailable,
        };
        Self {
            position: entry.index(),
            state,
            decoded_bytes,
            retained_input: error.is_none(),
        }
    }

    pub(super) const fn retain_prefix(&mut self) {
        self.retained_input = true;
    }
}

#[derive(Debug, Default)]
pub(super) struct AudioSurveys {
    owner: Option<ArchiveInputOrigin>,
    directory_complete: bool,
    pub(super) soundbanks: Vec<Candidate>,
}

impl AudioSurveys {
    pub(super) fn new(archive: &Archive) -> Self {
        Self {
            owner: archive
                .root_directory_entry_count()
                .map(|_| archive.input_origin().clone()),
            directory_complete: !archive
                .issues()
                .iter()
                .any(|(_, issue)| issue.is_archive_wide()),
            soundbanks: Vec::new(),
        }
    }
}

impl AudioIndex {
    /// Returns the index's actual survey when supplied its original root Archive.
    #[must_use]
    pub fn survey<'index, 'archive>(
        &'index self,
        archive: &'archive Archive,
        kind: AudioSurveyKind,
    ) -> Option<AudioSurvey<'index, 'archive>> {
        archive.root_directory_entry_count()?;
        if !self
            .surveys
            .owner
            .as_ref()?
            .is_same_input(archive.input_origin())
        {
            return None;
        }
        let candidates = match kind {
            AudioSurveyKind::AtlDefinitions => self.definitions.candidates(),
            AudioSurveyKind::Soundbanks => &self.surveys.soundbanks,
        };
        Some(AudioSurvey {
            archive,
            candidates,
            directory_complete: self.surveys.directory_complete,
        })
    }
}

#[cfg(test)]
#[path = "inventory_tests.rs"]
mod tests;
