// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_extraction::{
    Archive, ArchiveWatch, Build, CancelToken, FieldOrigin, ProvenanceMode, SourceInventory,
    SourceUse, SourceValue, SourcedValue,
};

use super::ContainerRootStatus;
use crate::catalog_query_session::checkpoint;
use crate::catalog_read::original::{EntryCapture, PhysicalEntryRef, with_entry_capture_mode};
use crate::entry_source::EntrySource;
use crate::starmap_index::{Index, SystemRootCandidates};
use crate::{CatalogError, ExportAccountingError, HealthText, export_archive_health};

/// Canonical system-root candidates and their original directory evidence.
#[derive(Debug)]
#[non_exhaustive]
#[must_use]
pub struct ContainerSystemRootInventory {
    /// Build metadata present in the watched Archive.
    pub build: Option<Build>,
    /// Observed winning-path groups in the same order as the Normal placement writer.
    pub roots: Vec<ContainerSystemRoot>,
    /// Original directory path range used to enumerate the winning groups.
    pub directory_origin: FieldOrigin,
    /// Original directory and candidate Entry sources and uses.
    pub source_inventory: SourceInventory,
}

impl ContainerSystemRootInventory {
    /// Whether the original directory establishes the scope of the root survey.
    #[must_use]
    pub fn directory_complete(&self) -> bool {
        self.directory_origin.source_set_complete()
    }

    /// Whether the complete directory establishes one physical root for each system.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.directory_complete()
            && self.roots.iter().all(|root| {
                root.status.origin().source_set_complete() && root.selected_entry().is_some()
            })
    }
}

/// The winning directory path for one system with its observed physical candidates.
#[derive(Debug)]
#[non_exhaustive]
#[must_use]
pub struct ContainerSystemRoot {
    /// Normalized system folder name.
    pub system: SourcedValue<HealthText>,
    /// Whether the observed candidates identify one root or an ambiguous path.
    pub status: SourcedValue<ContainerRootStatus>,
    /// All physical candidates at the winning path, in Archive directory order.
    pub candidates: Vec<SourcedValue<PhysicalEntryRef>>,
}

impl ContainerSystemRoot {
    /// Borrows the unique observed candidate, if the winning path has one physical Entry.
    #[must_use]
    pub const fn selected_entry(&self) -> Option<&PhysicalEntryRef> {
        let [candidate] = self.candidates.as_slice() else {
            return None;
        };
        candidate.value()
    }
}

/// Reads the original root directory choices of the Normal container placement writer.
/// A covered candidate identifies an Entry whose payload has yet to be inspected.
///
/// # Errors
/// Returns input evidence, Archive mutation or cancellation errors.
pub fn read_container_system_roots(
    archive: &Archive,
    cancel: Option<&CancelToken>,
) -> Result<ContainerSystemRootInventory, CatalogError> {
    read_container_system_roots_with_provenance_mode(archive, ProvenanceMode::MarkersOnly, cancel)
}

/// Reads the container system roots with the selected source detail.
///
/// # Errors
/// Returns the errors of `read_container_system_roots`.
pub fn read_container_system_roots_with_provenance_mode(
    archive: &Archive,
    provenance_mode: ProvenanceMode,
    cancel: Option<&CancelToken>,
) -> Result<ContainerSystemRootInventory, CatalogError> {
    checkpoint(cancel)?;
    let watch = ArchiveWatch::new(archive);
    let index = Index::new(archive);
    let choices = choose_roots(&index, cancel)?;
    let inventory = capture_roots(archive, &index, &choices, provenance_mode, cancel)?;
    watch.check()?;
    checkpoint(cancel)?;
    Ok(inventory)
}

pub(super) struct RootChoice<'index> {
    pub(super) system: Arc<str>,
    pub(super) candidates: SystemRootCandidates<'index>,
}

pub(super) fn choose_roots<'index>(
    index: &'index Index<'_>,
    cancel: Option<&CancelToken>,
) -> Result<Vec<RootChoice<'index>>, CatalogError> {
    index
        .systems()
        .into_iter()
        .map(|(system, position)| {
            checkpoint(cancel)?;
            let candidates = index
                .system_root_candidates(position)
                .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
            Ok(RootChoice { system, candidates })
        })
        .collect()
}

pub(super) fn capture_roots(
    archive: &Archive,
    index: &Index<'_>,
    choices: &[RootChoice<'_>],
    provenance_mode: ProvenanceMode,
    cancel: Option<&CancelToken>,
) -> Result<ContainerSystemRootInventory, CatalogError> {
    with_entry_capture_mode(archive, provenance_mode, |capture| {
        let sources = capture.source_uses();
        let directory = sources.observe_root_directory_paths(archive)?;
        let directory_complete =
            directory.is_some() && export_archive_health::inventory(archive).is_none();
        let directory_origin = sources.origin(directory.as_slice(), directory_complete);
        let mut roots = Vec::with_capacity(choices.len());
        for choice in choices {
            checkpoint(cancel)?;
            roots.push(read_root(
                capture,
                archive,
                index,
                choice,
                (directory.as_slice(), directory_complete),
                cancel,
            )?);
        }
        let source_inventory = sources.finish_with(|| checkpoint(cancel))?;
        Ok(ContainerSystemRootInventory {
            build: archive.build().cloned(),
            roots,
            directory_origin,
            source_inventory,
        })
    })
}

fn read_root(
    capture: &EntryCapture<'_>,
    archive: &Archive,
    index: &Index<'_>,
    choice: &RootChoice<'_>,
    (directory, mut complete): (&[SourceUse], bool),
    cancel: Option<&CancelToken>,
) -> Result<ContainerSystemRoot, CatalogError> {
    let (status, slots) = match &choice.candidates {
        SystemRootCandidates::Unique(slot) => {
            (ContainerRootStatus::Selected, std::slice::from_ref(*slot))
        }
        SystemRootCandidates::Ambiguous(slots) => (ContainerRootStatus::Ambiguous, *slots),
    };
    let sources = capture.source_uses();
    let mut candidates = Vec::with_capacity(slots.len());
    let mut uses = directory.to_vec();
    for &slot in slots {
        checkpoint(cancel)?;
        let entry = index
            .entries
            .get(slot)
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        let reference = EntrySource::outer(capture, archive, entry)?
            .receipt()
            .physical_reference(capture)?;
        let (inputs, known) = reference.contributions();
        uses.extend_from_slice(inputs);
        complete &= known;
        candidates.push(reference.into_cell(sources));
    }
    let shared = sources.shared_origin(&uses, complete);
    Ok(ContainerSystemRoot {
        system: sources.value_with_shared_origin(
            SourceValue::Present(HealthText::new(&choice.system)),
            &shared,
            &[],
            true,
        )?,
        status: sources.value_with_shared_origin(
            SourceValue::Present(status),
            &shared,
            &[],
            true,
        )?,
        candidates,
    })
}
