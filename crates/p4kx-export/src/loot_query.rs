// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Selective original loot declarations and their weighted views.

use p4kx_extraction::{Archive, Build, CancelToken, DataForgeEntry, SourceInventory};

use crate::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordCoverage, CatalogRecordSelection,
    DatasetAvailability, ParsedCatalogQueries,
};
use crate::{CatalogError, CraftingWarning};

pub use crate::catalog_v3::loot_definition_catalog::{LootDefinition, LootDefinitionKind};
pub use crate::contract_harvest::HarvestableLink;
pub use crate::contract_harvest_config::{SubHarvestable, SubHarvestableMultiConfig, TaggedConfig};
pub use crate::contract_loot::{
    LootArchetype, LootArchetypeEntry, LootConfig, LootSelector, LootTable, LootTableEntry,
    QuantityRange, SpawnWith,
};
pub use crate::contract_loot_legacy::{
    LegacyLootArchetype, LegacyLootEntry, LegacyLootQuantity, LegacyLootTable,
    LegacyLootTableEntry, LegacySecondaryEntry, LegacySecondaryGroup, LegacySpawnWith,
};

#[path = "legacy_loot_query.rs"]
mod legacy;
pub use legacy::LegacyLootSnapshot;

#[path = "loot_query_read.rs"]
mod read;

#[path = "loot_document_query.rs"]
mod documents;
pub use crate::catalog_read::original::{PhysicalEntryRef, SourcedXmlAttribute};
pub use crate::guid_text::CatalogGuid;
pub use crate::loot_entxml::documents::{
    LootContainerLookupRepair, LootContainerLookupRepairKind, LootContainerTargetStatus,
    LootEntityConfigReference, LootEntityContainerDocument, LootEntityContainerLink,
    LootEntityLinkNode,
};
pub use documents::{
    LootDocumentAdmission, LootDocumentCoverage, LootDocumentQuery, LootDocumentReachability,
    LootDocumentSelection, LootDocumentSnapshot, read_loot_documents,
};

/// The selected physical definitions and any weighted views of those owners.
#[derive(Debug)]
#[non_exhaustive]
pub struct LootDefinitionSnapshot<'source> {
    /// The identity of the Archive build, when known.
    pub build: Option<Build>,
    /// Availability of the source declaration independently of the selector.
    pub availability: DatasetAvailability,
    /// The selected family in canonical Catalog row order.
    pub rows: Vec<LootDefinition<'source>>,
    /// The weighted table views when the selected family is a table.
    pub tables: Vec<LootTable<'source>>,
    /// The weighted archetype views when the selected family is an archetype.
    pub archetypes: Vec<LootArchetype<'source>>,
    /// The slot-preset views for the selected multi-config family, in Catalog order.
    pub sub_harvestable_multi_configs: Vec<SubHarvestableMultiConfig<'source>>,
    /// Unavailable harvestable dependencies reached by the multi-config projection.
    pub harvest_dependency_warning: Option<CraftingWarning>,
    /// The physical sources and uses backing the returned values.
    pub source_inventory: SourceInventory,
    /// Coverage of the selected physical owners.
    pub coverage: CatalogRecordCoverage,
}

/// Reads one Loot Record family without discovering world placements or media.
///
/// # Errors
/// Returns input, source-evidence, Archive change or cancellation errors.
pub fn with_loot_definitions<T>(
    archive: &Archive,
    family: LootDefinitionKind,
    selection: CatalogRecordSelection,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(LootDefinitionSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    CatalogQuerySession::open(archive, CatalogLanguageSelection::Selected(&[]), cancel)?
        .with_loot_definitions(family, selection, cancel, inspect)
}

/// Reads one Loot Record family from a decoded DataForge Entry of the same Archive.
///
/// # Errors
/// Returns input, source-origin, source-evidence, Archive change or cancellation errors.
pub fn with_loot_definitions_from<T>(
    archive: &Archive,
    entry: &DataForgeEntry,
    family: LootDefinitionKind,
    selection: CatalogRecordSelection,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(LootDefinitionSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    CatalogQuerySession::from_entry(
        archive,
        entry,
        CatalogLanguageSelection::Selected(&[]),
        cancel,
    )?
    .with_loot_definitions(family, selection, cancel, inspect)
}

impl CatalogQuerySession<'_> {
    /// Reads one Loot Record family from this session.
    ///
    /// # Errors
    /// Returns input, source-evidence, Archive change or cancellation errors.
    pub fn with_loot_definitions<T>(
        &self,
        family: LootDefinitionKind,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(LootDefinitionSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.parsed(cancel)?
            .with_loot_definitions(family, selection, cancel, inspect)
    }
}

impl ParsedCatalogQueries<'_> {
    /// Reads one Loot Record family with this view's reusable DataForge indexes.
    ///
    /// # Errors
    /// Returns input, source-evidence, Archive change or cancellation errors.
    pub fn with_loot_definitions<T>(
        &self,
        family: LootDefinitionKind,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(LootDefinitionSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.project(cancel, |archive, entry, forge| {
            read::Query {
                archive,
                entry,
                forge,
                family,
                selection,
                cancel,
            }
            .run(inspect)
        })
    }
}
