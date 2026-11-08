// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::Cell;

use p4kx_extraction::{Entry, TextureMemberCandidate};

use crate::directory_index::{self, DirectoryIndex};
use crate::directory_inputs::DirectoryInputs;
use crate::lookup_work::LookupWork;
use crate::source;
use crate::source_selection::{EntrySelection, SourceSelection};
use crate::{
    CandidateInventory, CandidateState, ContainerIdentity, Digest, EntryInput, Identity,
    InputLookup, MediaError, MediaKind, MediaWriter, NotSelectedReason, OriginalIdentifier,
    Reference, ReferenceCandidate, ReferenceResolution, ReferenceTarget, ReferenceUnavailable,
    Source, Verification,
};

/// Source-bound candidate identities without opening or decoding target Entries.
pub struct DirectoryLookup<'a> {
    pub(crate) index: &'a DirectoryIndex<'a>,
    pub(crate) root: &'a Digest,
    pub(crate) chain: &'a [ContainerIdentity],
    pub(crate) selection: SourceSelection<'a>,
    pub(crate) work: &'a Cell<u64>,
    pub(crate) source_allowance: u64,
    inputs: Option<&'a DirectoryInputs<'a>>,
    pub(crate) shaders: Option<&'a crate::shader_lookup::ShaderLookup<'a>>,
}

impl<'a> DirectoryLookup<'a> {
    #[cfg(test)]
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
            inputs: None,
            shaders: None,
        }
    }
    pub(crate) const fn new_with_inputs(
        index: &'a DirectoryIndex<'a>,
        root: &'a Digest,
        chain: &'a [ContainerIdentity],
        selection: SourceSelection<'a>,
        inputs: &'a DirectoryInputs<'a>,
    ) -> Self {
        Self {
            index,
            root,
            chain,
            selection,
            work: &inputs.budget.work,
            source_allowance: 0,
            inputs: Some(inputs),
            shaders: None,
        }
    }

    pub(crate) const fn with_shaders(
        mut self,
        shaders: Option<&'a crate::shader_lookup::ShaderLookup<'a>>,
    ) -> Self {
        self.shaders = shaders;
        self
    }
    pub(crate) fn input_state(&self) -> Result<&DirectoryInputs<'_>, MediaError> {
        self.inputs
            .ok_or_else(|| MediaError::Contract("required input access is unavailable".into()))
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
        Ok(self.lookup_input(original, path, kind, writer)?.reference)
    }

    /// Classifies an exact path and lends a unique physical input without reading it.
    ///
    /// # Errors
    /// Returns original lookup, work, cancellation or source-change failures.
    pub fn lookup_input(
        &self,
        original: OriginalIdentifier,
        path: &str,
        kind: Option<MediaKind>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<InputLookup<'_>, MediaError> {
        let source_allowance = self.allowance(writer.limits().sources);
        let work = LookupWork {
            checkpoint: writer.checkpoint,
            limits: writer.limits(),
            current: self.work,
            source_allowance,
        };
        let index = self.index.index(&work)?;
        work.charge(1)?;
        let requested = kind.is_none_or(|kind| writer.has_kind(kind));
        let mut position = None;
        let mut observed = 0_u64;
        let candidates = index
            .matching_entries_with(path, |count| work.charge(count))
            .map_err(directory_index::error)?
            .map(|entry| {
                let entry = entry.map_err(directory_index::error)?;
                observed += 1;
                position = Some(entry.index());
                Ok(self.candidate(entry, kind, requested))
            });
        let reference =
            writer.try_resolve_reference(original, candidates, self.index.inventory())?;
        let usable = reference.candidates_complete
            && observed == 1
            && matches!(
                &reference.resolution,
                ReferenceResolution::Resolved { .. } | ReferenceResolution::NotSelected { .. }
            );
        let input = if usable {
            position.map(|position| EntryInput {
                root: self.root,
                chain: self.chain,
                position,
            })
        } else {
            None
        };
        Ok(InputLookup { reference, input })
    }

    /// Visits all physical texture-family candidates without reading or resolving any target.
    ///
    /// Candidate states retain selection and raw-path proof; the returned inventory bounds absence.
    ///
    /// # Errors
    /// Returns original lookup/work/checkpoint, physical-index or visitor failures.
    pub fn visit_texture_family(
        &self,
        header_path: &str,
        writer: &mut MediaWriter<'_>,
        mut visit: impl FnMut(
            TextureMemberCandidate<'_>,
            ReferenceCandidate,
            &mut MediaWriter<'_>,
        ) -> Result<(), MediaError>,
    ) -> Result<CandidateInventory, MediaError> {
        let work = LookupWork {
            checkpoint: writer.checkpoint,
            limits: writer.limits(),
            current: self.work,
            source_allowance: self.allowance(writer.limits().sources),
        };
        let index = self.index.texture_members(&work)?;
        work.charge(1)?;
        let requested = writer.has_kind(MediaKind::Texture);
        for candidate in index
            .family_candidates_with(header_path, |count| work.charge(count))
            .map_err(directory_index::error)?
        {
            let candidate = candidate.map_err(directory_index::error)?;
            let reference = self.candidate(candidate.entry, Some(MediaKind::Texture), requested);
            visit(candidate, reference, writer)?;
        }
        work.charge(0)?;
        Ok(self.index.inventory())
    }

    fn allowance(&self, limit: u64) -> u64 {
        self.inputs.map_or(self.source_allowance, |inputs| {
            inputs.source_allowance(limit)
        })
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

#[cfg(test)]
#[path = "directory_texture_lookup_tests.rs"]
mod texture_tests;
