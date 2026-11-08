// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::{Cell, OnceCell};
use std::io;

use p4kx_extraction::{Archive, Entry, EntryOrigin, EntryOriginStep};

use crate::directory_lookup::DirectoryLookup;
use crate::shader_flags::ShaderRead;
use crate::shader_lookup::ShaderLookup;
use crate::source_selection::EntrySelection;
use crate::traversal::Traversal;
use crate::traversal_visit::TraversalVisit;
use crate::{
    CandidateInventory, CandidateState, MediaError, MediaKind, MediaWriter, NotSelectedReason,
    ReferenceCandidate, ReferenceTarget, ReferenceUnavailable, Source, Tally,
};
use crate::{shader_source, source};

pub(crate) struct ShaderTarget {
    pub(crate) name: String,
    pub(crate) source: Source,
    pub(crate) origin: EntryOrigin,
    pub(crate) state: CandidateState,
    pub(crate) read: OnceCell<ShaderRead>,
}

pub(crate) struct ShaderIndex {
    pub(crate) targets: Vec<ShaderTarget>,
    pub(crate) inventory: CandidateInventory,
}

impl ShaderIndex {
    pub(crate) fn build(
        lookup: &ShaderLookup<'_>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<Self, MediaError> {
        let mut collector = Collector {
            index: Self {
                targets: Vec::new(),
                inventory: CandidateInventory::Complete,
            },
            requested: writer.has_kind(MediaKind::Shader),
        };
        let keys = Cell::new(0);
        let mut traversal = Traversal {
            root: lookup.root,
            providers: &[],
            include: lookup.selection.include,
            exclude: lookup.selection.exclude,
            entries: Tally::default(),
            budget: lookup.budget,
            index_keys: &keys,
            shaders: None,
        };
        traversal.visit(lookup.archive, &mut Vec::new(), writer, &mut collector)?;
        Ok(collector.index)
    }

    pub(crate) fn candidates(
        &self,
        positions: &[usize],
        writer: &MediaWriter<'_>,
    ) -> Result<Vec<ReferenceCandidate>, MediaError> {
        let mut candidates = Vec::new();
        candidates
            .try_reserve_exact(positions.len())
            .map_err(|cause| allocation(writer, cause))?;
        for position in positions {
            let target = self.targets.get(*position).ok_or_else(|| {
                MediaError::Contract("shader candidate is outside its physical index".into())
            })?;
            candidates.push(target.candidate());
        }
        Ok(candidates)
    }
    pub(crate) fn matching(
        &self,
        name: &str,
        lookup: &ShaderLookup<'_>,
        writer: &MediaWriter<'_>,
    ) -> Result<Vec<usize>, MediaError> {
        let mut matches = Vec::new();
        for (index, target) in self.targets.iter().enumerate() {
            lookup.work(target.name.len().min(name.len()).saturating_add(1), writer)?;
            if !target.name.eq_ignore_ascii_case(name) {
                continue;
            }
            matches
                .try_reserve(1)
                .map_err(|cause| allocation(writer, cause))?;
            matches.push(index);
            if crate::shader_values::count(matches.len())?
                > u64::from(writer.limits().reference_candidates)
            {
                break;
            }
        }
        Ok(matches)
    }
}

impl ShaderTarget {
    pub(crate) fn candidate(&self) -> ReferenceCandidate {
        let mut target = ReferenceTarget::new_with_source(&self.source);
        target.kind = Some(MediaKind::Shader);
        ReferenceCandidate::new(target, self.state)
    }
}

struct Collector {
    index: ShaderIndex,
    requested: bool,
}

impl TraversalVisit for Collector {
    fn counts_sources(&self) -> bool {
        false
    }

    fn directory(
        &mut self,
        archive: &Archive,
        lookup: &DirectoryLookup<'_>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        for (_, issue) in archive.issues() {
            lookup
                .input_state()?
                .budget
                .metadata(0, 1, writer.limits(), writer.checkpoint)?;
            if issue.is_archive_wide() {
                self.index.inventory =
                    CandidateInventory::Unavailable(ReferenceUnavailable::IncompleteInventory);
            }
        }
        Ok(())
    }

    fn entry(
        &mut self,
        entry: &Entry,
        selection: EntrySelection,
        lookup: &DirectoryLookup<'_>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        if lookup.index.archive.raw_path(entry.index()).is_some() {
            self.index.inventory =
                CandidateInventory::Unavailable(ReferenceUnavailable::IncompleteInventory);
        }
        let file = shader_source::file_name(entry.path());
        let Some((name, extension)) = file.rsplit_once('.') else {
            return Ok(());
        };
        if !extension.eq_ignore_ascii_case("ext") {
            return Ok(());
        }
        lookup.input_state()?.budget.metadata(
            0,
            identity_work(name, entry, lookup)?,
            writer.limits(),
            writer.checkpoint,
        )?;
        self.index
            .targets
            .try_reserve_exact(1)
            .map_err(|cause| allocation(writer, cause))?;
        let origin = lookup.index.archive.entry_origin(entry).ok_or_else(|| {
            MediaError::Contract("shader inventory Entry has no owned origin".into())
        })?;
        let state = if selection != EntrySelection::Included {
            CandidateState::NotSelected {
                reason: NotSelectedReason::Filter,
            }
        } else if !self.requested {
            CandidateState::NotSelected {
                reason: NotSelectedReason::KindNotRequested,
            }
        } else {
            CandidateState::Selected
        };
        self.index.targets.push(ShaderTarget {
            name: name.to_owned(),
            source: source::entry(lookup.root, lookup.index.archive, entry, lookup.chain)?,
            origin,
            state,
            read: OnceCell::new(),
        });
        Ok(())
    }

    fn excluded_container(&mut self) {
        self.index.inventory =
            CandidateInventory::Unavailable(ReferenceUnavailable::IncompleteInventory);
    }
}

fn identity_work(
    name: &str,
    entry: &Entry,
    lookup: &DirectoryLookup<'_>,
) -> Result<u64, MediaError> {
    let mut work = crate::shader_values::count(size_of::<ShaderTarget>())?;
    let mut add = |length: usize, copies: u64| -> Result<(), MediaError> {
        let bytes = crate::shader_values::count(length)?
            .checked_mul(copies)
            .ok_or_else(|| MediaError::Contract("shader identity storage size overflow".into()))?;
        work = work
            .checked_add(bytes)
            .ok_or_else(|| MediaError::Contract("shader identity storage size overflow".into()))?;
        Ok(())
    };
    add(size_of::<EntryOriginStep>(), 1)?;
    add(
        size_of_val(entry.metadata().encryption_declarations().iter().as_slice()),
        1,
    )?;
    add(name.len(), 1)?;
    add(entry.path().len(), 2)?;
    add(lookup.root.hex.len(), 1)?;
    if let Some(raw) = lookup.index.archive.raw_path(entry.index()) {
        add(raw.len(), 2)?;
    }
    for ancestor in lookup.chain {
        add(ancestor.path.len(), 1)?;
        if let Some(raw) = &ancestor.raw_path_hex {
            add(raw.len(), 1)?;
        }
    }
    Ok(work)
}
fn allocation(writer: &MediaWriter<'_>, cause: std::collections::TryReserveError) -> MediaError {
    MediaError::Io {
        path: writer.root.clone(),
        source: io::Error::new(io::ErrorKind::OutOfMemory, cause),
    }
}
