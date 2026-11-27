// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Selective access to authored NPC spawn definitions.

use p4kx_extraction::{Archive, Build, CancelToken, DataForge, DataForgeEntry, SourceInventory};

use super::unlocalized_record_query::plan::Plan;
use super::{CatalogLanguageSelection, CatalogRecordCoverage, CatalogRecordSelection};
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_query_session::{
    CatalogQuerySession, CatalogQuerySource, ParsedCatalogQueries, checkpoint,
};
use crate::catalog_table::DatasetAvailability;
use crate::catalog_v3::npc_spawn_definitions::{NpcSpawnDefinitions, NpcSpawnDefinitionsPick};
use crate::record_observation::RecordObservation;
use crate::{CatalogError, Kind};

pub use crate::catalog_v3::npc_spawn_definitions::{
    AssignmentKind, AutoSpawnClass, AutoSpawnData, AutoSpawnDefinition, AutoSpawnSet,
    AutoSpawnSetEntry, ClassSelector, EncounterIndex, EncounterIndexEntry, NpcArchetypeDefinition,
    NpcArchetypeWeight, NpcClassPoolDefinition, NpcClassPoolEntry, NpcIdentityBranch,
    NpcSpawnDefinitionRows, SetEntryData, SpawnAssignment, SpawnInventory, SpawnLimits, SpawnTags,
};

/// Selects one canonical authored NPC spawn definition family.
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub enum NpcSpawnDefinitionFamily {
    /// Selects `definitions.npcArchetypes`.
    Archetypes,
    /// Selects `definitions.npcClassPools`.
    ClassPools,
    /// Selects `definitions.autoSpawns`.
    Spawns,
    /// Selects `definitions.autoSpawnSets`.
    Sets,
    /// Selects `definitions.encounterIndices`.
    Encounters,
}

impl NpcSpawnDefinitionFamily {
    const fn kind(self) -> Kind {
        match self {
            Self::Archetypes => Kind::NpcArchetypes,
            Self::ClassPools => Kind::NpcClassPools,
            Self::Spawns => Kind::AutoSpawns,
            Self::Sets => Kind::AutoSpawnSets,
            Self::Encounters => Kind::EncounterIndices,
        }
    }
}

/// Selected original NPC spawn rows and their physical source evidence.
#[derive(Debug)]
#[non_exhaustive]
#[must_use]
pub struct NpcSpawnDefinitionSnapshot<'source> {
    /// The installed Archive Build, when known.
    pub build: Option<Build>,
    /// Availability of the selected source family.
    pub availability: DatasetAvailability,
    /// Canonical rows ordered by GUID then physical Record index.
    pub rows: NpcSpawnDefinitionRows<'source>,
    /// Original sources and uses backing the returned values.
    pub source_inventory: SourceInventory,
    /// Unreadable, incomplete and skipped Records within the selected scope.
    pub coverage: CatalogRecordCoverage,
}

/// Supplies one original NPC spawn family without following referenced Record bodies.
/// Publish consumer-owned results after this function succeeds.
///
/// # Errors
/// Returns Archive, accounting, source-evidence or cancellation errors.
pub fn with_npc_spawn_definitions<T>(
    archive: &Archive,
    family: NpcSpawnDefinitionFamily,
    selection: CatalogRecordSelection,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(NpcSpawnDefinitionSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    CatalogQuerySession::open(archive, CatalogLanguageSelection::Selected(&[]), cancel)?
        .with_npc_spawn_definitions(family, selection, cancel, inspect)
}

/// Supplies one original NPC spawn family from an already decoded Archive Entry.
///
/// # Errors
/// Returns query errors or a source-origin mismatch for a foreign Entry.
pub fn with_npc_spawn_definitions_from<T>(
    archive: &Archive,
    entry: &DataForgeEntry,
    family: NpcSpawnDefinitionFamily,
    selection: CatalogRecordSelection,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(NpcSpawnDefinitionSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    CatalogQuerySession::from_entry(
        archive,
        entry,
        CatalogLanguageSelection::Selected(&[]),
        cancel,
    )?
    .with_npc_spawn_definitions(family, selection, cancel, inspect)
}

impl CatalogQuerySession<'_> {
    /// Supplies one original NPC spawn family through this Archive session.
    ///
    /// # Errors
    /// Returns the errors of [`with_npc_spawn_definitions`].
    pub fn with_npc_spawn_definitions<T>(
        &self,
        family: NpcSpawnDefinitionFamily,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(NpcSpawnDefinitionSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.parsed(cancel)?
            .with_npc_spawn_definitions(family, selection, cancel, inspect)
    }
}

impl ParsedCatalogQueries<'_> {
    /// Supplies one original NPC spawn family from the parsed DataForge.
    ///
    /// # Errors
    /// Returns the errors of [`with_npc_spawn_definitions`].
    pub fn with_npc_spawn_definitions<T>(
        &self,
        family: NpcSpawnDefinitionFamily,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(NpcSpawnDefinitionSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.project(cancel, |archive, entry, forge| {
            project(
                archive,
                entry,
                forge,
                family.kind(),
                selection,
                cancel,
                inspect,
            )
        })
    }
}

fn project<T>(
    archive: &Archive,
    entry: CatalogQuerySource<'_>,
    forge: &DataForge<'_>,
    kind: Kind,
    selection: CatalogRecordSelection,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(NpcSpawnDefinitionSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    let Plan {
        owners,
        mut coverage,
    } = Plan::read(forge, kind, selection, cancel, None)?;
    let origins = entry.records(archive)?;
    let input = OriginalCatalogInput::new(forge, &origins);
    let mut rows = NpcSpawnDefinitions::default();
    let availability = if owners.is_empty() {
        DatasetAvailability::from_source_declarations(forge, |name| {
            name == kind.records().type_name
        })
    } else {
        DatasetAvailability::Available
    };
    for owner in owners {
        checkpoint(cancel)?;
        let admission = RecordObservation::capture(&input, &owner.record)?;
        let (row, notes) =
            NpcSpawnDefinitionsPick::read(kind, &owner.tree, &input, &admission, cancel)?;
        if notes
            .iter()
            .any(|note| note.description().issue.blocks_projection())
        {
            coverage.incomplete_fields.push(owner.record.index());
        }
        rows.push(row);
    }
    rows.order();
    let rows = rows.into_query_rows(kind)?;
    let source_inventory = origins.uses.finish_with(|| checkpoint(cancel))?;
    checkpoint(cancel)?;
    Ok(inspect(NpcSpawnDefinitionSnapshot {
        build: archive.build().cloned(),
        availability,
        rows,
        source_inventory,
        coverage,
    }))
}
