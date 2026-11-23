// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    Archive, Build, CancelToken, FieldOrigin, Guid, SourceInventory, SourceUseRef, SourcedValue,
};

use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_origins::CatalogOrigins;
use crate::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordCoverage, ParsedCatalogQueries,
};
use crate::contract_loot::LootContainer;
use crate::loot_containers;
use crate::{CatalogError, Filter, StarmapTraversalBudget};

/// Selects the exact physical terminal Starmap owner or the null canonical key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum LootAncestorSelection {
    /// Selects an original physical Record index in this DataForge.
    RecordIndex(u32),
    /// Selects a key with no resolved Starmap object.
    WithoutResolvedObject,
}

/// Selects one canonical Loot row by its original system scope and exact key.
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub struct LootContainerQuery<'query> {
    /// Physical outer SOC Entry identifying the containing system folder.
    pub system_entry_index: u32,
    /// The authored multi-config GUID.
    pub multi_config_guid: Guid,
    /// The exact canonical container path.
    pub container: &'query str,
    /// Requested original terminal Starmap Record or null location key.
    pub ancestor: LootAncestorSelection,
    /// Optional Catalog Record scope to reproduce.
    pub catalog_filter: Option<&'query Filter>,
    /// Optional caller-selected limits for descriptor and placement walks.
    pub placement_budget: StarmapTraversalBudget,
}

impl<'query> LootContainerQuery<'query> {
    /// Selects a key under the normal unfiltered Catalog policy.
    #[must_use]
    pub const fn new(
        system_entry_index: u32,
        multi_config_guid: Guid,
        container: &'query str,
        ancestor: LootAncestorSelection,
    ) -> Self {
        Self {
            system_entry_index,
            multi_config_guid,
            container,
            ancestor,
            catalog_filter: None,
            placement_budget: StarmapTraversalBudget::new(),
        }
    }
}

/// Outcome of selecting one physical canonical Loot key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum LootContainerStatus {
    /// A row was selected with complete competing route and owner observations.
    Found,
    /// Complete original source coverage excludes the requested key.
    NotFound,
    /// Competing original physical Starmap Records share the requested identity.
    Ambiguous,
    /// Original coverage is incomplete. An observed row may accompany the result.
    Unavailable,
}

/// Original physical positions of the selected Loot occurrence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct LootContainerOccurrence {
    /// The physical system Entry supplying the winning occurrence.
    pub system_entry_index: u32,
    /// The original seed that reaches the authored placement.
    pub seed_entry_index: u32,
    /// Outer archive Entry containing the original entity document.
    pub containing_entry_index: u32,
    /// Nested directory position of the original entity document.
    pub document_entry_index: u32,
}

/// One scoped canonical Loot row and the original inputs used for its selection.
#[derive(Debug)]
#[non_exhaustive]
pub struct LootContainerSnapshot {
    /// The Build identified by the Archive, when known.
    pub build: Option<Build>,
    /// The folder or lookup state of the requested physical system anchor.
    pub system_anchor: SourcedValue<String>,
    /// Result of the requested exact key.
    pub status: LootContainerStatus,
    /// The canonical Catalog row, when an occurrence was observed.
    pub row: Option<LootContainer>,
    /// Original author of the observed row.
    pub occurrence: Option<LootContainerOccurrence>,
    /// Original positive admission or complete searched Record path scope.
    pub admission_origin: Option<FieldOrigin>,
    /// Original header or table count used to check an explicit physical ancestor selection.
    pub ancestor_selection_origin: Option<FieldOrigin>,
    /// Shared original basis of the all-system placement survey, when run.
    pub placement_survey: Option<FieldOrigin>,
    /// Failed Record header and descriptor attempt references in the source inventory.
    pub failed_source_uses: Vec<SourceUseRef>,
    /// Original directory and verified document inputs of the reached link search.
    pub entity_link_survey: Option<FieldOrigin>,
    /// Source state of demanded physical Starmap aliases.
    pub records: CatalogRecordCoverage,
    /// All observed admitted aliases of the explicitly requested Starmap GUID.
    pub requested_aliases: Vec<u32>,
    /// Original inputs and uses backing the selected result.
    pub source_inventory: SourceInventory,
}

/// Reads at most one canonical Loot row directly from the original Archive.
///
/// # Errors
/// Returns original input, source accounting, Archive change or cancellation errors.
pub fn read_loot_container(
    archive: &Archive,
    query: LootContainerQuery<'_>,
    cancel: Option<&CancelToken>,
) -> Result<LootContainerSnapshot, CatalogError> {
    CatalogQuerySession::open(archive, CatalogLanguageSelection::Selected(&[]), cancel)?
        .read_loot_container(query, cancel)
}

impl CatalogQuerySession<'_> {
    /// Selects a canonical Loot row through this session's original Archive and Entry.
    ///
    /// # Errors
    /// Returns original input, source accounting, Archive change or cancellation errors.
    pub fn read_loot_container(
        &self,
        query: LootContainerQuery<'_>,
        cancel: Option<&CancelToken>,
    ) -> Result<LootContainerSnapshot, CatalogError> {
        self.parsed(cancel)?.read_loot_container(query, cancel)
    }
}

impl ParsedCatalogQueries<'_> {
    /// Selects a canonical Loot row with the session's reusable DataForge indexes.
    ///
    /// # Errors
    /// Returns original source accounting, Archive change or cancellation errors.
    pub fn read_loot_container(
        &self,
        query: LootContainerQuery<'_>,
        cancel: Option<&CancelToken>,
    ) -> Result<LootContainerSnapshot, CatalogError> {
        self.project(cancel, |archive, entry, forge| {
            let origins = CatalogOrigins::for_archive_records(archive, entry)?;
            let input = OriginalCatalogInput::new(forge, &origins);
            loot_containers::read_physical_container(archive, forge, &input, query, cancel)
        })
    }
}
