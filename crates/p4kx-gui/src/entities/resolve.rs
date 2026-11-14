// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_assembly::{ArchiveFiles, ChangeError, ChangedLoadout, LoadoutChanges, LoadoutResolver};
use p4kx_extraction::{DataForge, Guid};

use crate::forge_cache::{ForgeSource, ReadFailure};
use crate::model::describe;
use crate::{EntryIndex, NoEntries};

/// Why a Loadout cannot be resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolveFailure {
    /// The Archive changed on disk before or while the Loadout was resolved.
    ArchiveChanged,
    /// The DataForge has no Record with this GUID.
    NoRecord(Guid),
    /// Resolving was stopped, or ended without a result.
    Cancelled,
    /// The worker that resolves the Loadout ended unexpectedly.
    Lost,
    /// The Loadout cannot be resolved, for the reason given.
    Failed(String),
}

impl From<NoEntries> for ResolveFailure {
    fn from(error: NoEntries) -> Self {
        Self::Failed(error.to_string())
    }
}

/// Resolves the default Loadout of the Entity `guid` of `index` with `changes` applied, stopping
/// at docked vehicles like `entity glb` does, and gives up once `stop` returns `true`.
///
/// # Errors
///
/// Returns why the DataForge cannot be read, the Entity is none, the changes do not apply, or
/// that `stop` ended the resolution.
pub(crate) fn resolve(
    index: &EntryIndex,
    guid: Guid,
    changes: &LoadoutChanges,
    stop: &(dyn Fn() -> bool + Sync),
) -> Result<ChangedLoadout, ResolveFailure> {
    let source = read_source(index)?;
    let forge = open_forge(&source)?;
    let files = ArchiveFiles::new(index.entries_archive()?);
    let mut resolver = LoadoutResolver::new(&forge)
        .with_forge_entry(&source.entry)
        .with_files(&files)
        .with_stop(stop)
        .stopping_at_docked_vehicles();
    resolve_in(index, &mut resolver, &forge, (guid, changes))
}

/// Resolves the Entity `guid` with `changes` using the resolver of a session.
pub(super) fn resolve_in<'f>(
    index: &EntryIndex,
    resolver: &mut LoadoutResolver<'f>,
    forge: &'f DataForge<'f>,
    (guid, changes): (Guid, &LoadoutChanges),
) -> Result<ChangedLoadout, ResolveFailure> {
    if index.watch().check().is_err() {
        return Err(ResolveFailure::ArchiveChanged);
    }
    let record = forge
        .record_by_guid(guid)
        .ok_or(ResolveFailure::NoRecord(guid))?;
    let applied = resolver
        .resolve_changed(&record, changes)
        .map_err(|error| match error {
            ChangeError::Cancelled => ResolveFailure::Cancelled,
            other => ResolveFailure::Failed(describe(&other)),
        })?;
    if index.watch().check().is_err() {
        return Err(ResolveFailure::ArchiveChanged);
    }
    Ok(applied)
}

/// Returns the DataForge of `index`, reading it unless it was read before.
pub(super) fn read_source(index: &EntryIndex) -> Result<Arc<ForgeSource>, ResolveFailure> {
    if index.watch().check().is_err() {
        return Err(ResolveFailure::ArchiveChanged);
    }
    index
        .forge()
        .read(index, || {})
        .map_err(|failure| match failure {
            ReadFailure::ArchiveChanged(_) => ResolveFailure::ArchiveChanged,
            ReadFailure::Unreadable(reason) => ResolveFailure::Failed(reason),
        })
}

/// Opens the DataForge Entry read.
pub(super) fn open_forge(source: &ForgeSource) -> Result<DataForge<'_>, ResolveFailure> {
    source
        .entry
        .open()
        .map_err(|error| ResolveFailure::Failed(describe(&error)))
}
