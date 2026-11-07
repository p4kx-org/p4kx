// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::Cell;

use p4kx_extraction::Entry;

use crate::directory_index::{self, DirectoryIndex};
use crate::lookup_work::LookupWork;
use crate::source;
use crate::source_selection::{EntrySelection, SourceSelection};
use crate::{
    CandidateState, ContainerIdentity, Digest, Identity, MediaError, MediaKind, MediaWriter,
    NotSelectedReason, OriginalIdentifier, Reference, ReferenceCandidate, ReferenceTarget,
    ReferenceUnavailable, Source, Verification,
};

/// Source-bound candidate identities without opening or decoding target Entries.
pub struct DirectoryLookup<'a> {
    pub(crate) index: &'a DirectoryIndex<'a>,
    pub(crate) root: &'a Digest,
    pub(crate) chain: &'a [ContainerIdentity],
    pub(crate) selection: SourceSelection<'a>,
    pub(crate) work: &'a Cell<u64>,
    pub(crate) source_allowance: u64,
}

impl<'a> DirectoryLookup<'a> {
    pub(crate) const fn new(
        index: &'a DirectoryIndex<'a>,
        root: &'a Digest,
        chain: &'a [ContainerIdentity],
        selection: SourceSelection<'a>,
        work: &'a Cell<u64>,
        source_allowance: u64,
    ) -> Self {
        Self {
            index,
            root,
            chain,
            selection,
            work,
            source_allowance,
        }
    }
    /// Resolves a domain-validated Entry path in the immediately owning Archive.
    ///
    /// Resolved proves physical identity only; target readability and projection are separate.
    ///
    /// # Errors
    /// Returns the original work, cancellation, source-change or index-storage failure.
    pub fn resolve_path(
        &self,
        original: OriginalIdentifier,
        path: &str,
        kind: Option<MediaKind>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<Reference, MediaError> {
        let work = LookupWork {
            checkpoint: writer.checkpoint,
            limits: writer.limits(),
            current: self.work,
            source_allowance: self.source_allowance,
        };
        let index = self.index.index(&work)?;
        work.charge(1)?;
        let requested = kind.is_none_or(|kind| writer.has_kind(kind));
        let candidates = index
            .matching_entries_with(path, |count| work.charge(count))
            .map_err(directory_index::error)?
            .map(|entry| {
                entry
                    .map(|entry| self.candidate(entry, kind, requested))
                    .map_err(directory_index::error)
            });
        let inventory = self.index.inventory();
        writer.try_resolve_reference(original, candidates, inventory)
    }

    fn candidate(
        &self,
        entry: &Entry,
        kind: Option<MediaKind>,
        requested: bool,
    ) -> ReferenceCandidate {
        let state = if self.index.archive.raw_path(entry.index()).is_some() {
            CandidateState::Unavailable {
                reason: ReferenceUnavailable::UnsupportedLayout,
            }
        } else if self.selection.classify(self.chain, entry.path()) != EntrySelection::Included {
            CandidateState::NotSelected {
                reason: NotSelectedReason::Filter,
            }
        } else if !requested {
            CandidateState::NotSelected {
                reason: NotSelectedReason::KindNotRequested,
            }
        } else {
            CandidateState::Selected
        };
        let target = ReferenceTarget {
            root: self.root.clone(),
            source: Identity::Entry {
                containers: self.chain.to_vec(),
                entry: source::identity(self.index.archive, entry),
            },
            kind,
            subresource: None,
        };
        ReferenceCandidate::new(target, state)
    }

    pub(crate) fn owning_source(&self, size: u64) -> Source {
        let identity = match self.chain.split_last() {
            Some((entry, containers)) => Identity::Entry {
                containers: containers.to_vec(),
                entry: entry.clone(),
            },
            None => Identity::Root,
        };
        Source {
            root: self.root.clone(),
            identity,
            size,
            declared_integrity: None,
            verification: Verification::NotAvailable,
            raw: None,
        }
    }
}

#[cfg(test)]
#[path = "directory_lookup_tests.rs"]
mod tests;
