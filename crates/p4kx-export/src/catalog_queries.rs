// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Selective access to canonical catalog data in an opened game Archive.

#[path = "blueprint_category_query.rs"]
mod blueprint_categories;
#[path = "unlocalized_record_query.rs"]
mod unlocalized_record_query;

#[path = "contract_query.rs"]
mod contracts;
pub use contracts::{
    AutoSpawnSettings, AutoSpawns, BlueprintReward, BlueprintSource, CatalogRecordFailure,
    ClassFilter, ClassTarget, ClassTargets, ClueDetails, ClueItemDetails, ContractBase,
    ContractCooldown, ContractDependencyCoverage, ContractDiagnosticExample, ContractEntry,
    ContractFaction, ContractFacts, ContractKey, ContractOccurrence, ContractQueryCoverage,
    ContractQueryDiagnostic, ContractSnapshot, ContractWithTitles, CooldownLayers,
    DefinitionReference, DependentProperty, EntitySpawnGroup, EntitySpawnOption, EntitySpawnPool,
    EntitySpawnSlot, FactLayer, FactionFilter, FixedReward, FloatOption, FunctionFilter,
    FunctionTarget, FunctionTargets, GiverFaction, GiverSource, HaulingItem, HaulingItemKind,
    HaulingOrder, HaulingRequirement, HaulingSource, Hostility, IntegerOption, Investigation,
    InvestigationClue, InvestigationClueItem, InvestigationLocation, InvestigationName,
    InvestigationNameSource, InvestigationSuspect, ItemToken, ItemTokenOutcome, LayeredFacts,
    LocationCondition, LocationCount, LocationPrerequisite, LocationQuery, LocationQueryLink,
    MissionItemGap, MissionPropertyDeclaration, MissionPropertySet, MissionPropertyText,
    MissionReward, MissionTitles, NpcSpawnGroup, NpcSpawnName, NpcSpawnOption, NpcSpawnPool,
    OfferedAt, OptionMetadata, OtherLocationFilter, OwnedReference, OwnedReferenceStatus,
    PickupPlace, ProjectedMissionText, PropertyReference, PropertyReferenceStatus, PropertyTarget,
    PropertyTargets, PropertyValue, PublishedNpcPool, RelativeLocationFilter, RelativeTarget,
    ReputationGain, RequiredIntro, ScopedLocation, ShadowedContract, ShipSpawnGroup,
    ShipSpawnOption, ShipSpawnPool, ShipSpawnSlot, SizeFilter, SizeTarget, SizeTargets,
    SourcePointer, SourceTagList, SpawnClosetSearchOptions, SpawnInventoryItem, SpawnLevel,
    SpawnPoolLink, SpawnTagText, StandingLevel, StandingRequirement, Stated, StringOption,
    TagOption, TagPaths, TagTerm, TimeTrialSplit, with_contract, with_contract_from,
    with_contracts, with_contracts_from,
};

#[path = "contract_generator_query.rs"]
mod contract_generators;

#[path = "gameplay_property_query.rs"]
mod gameplay_properties;
#[path = "harvest_provider_query.rs"]
mod harvest_provider_presets;
#[path = "item_port_query.rs"]
mod item_ports;
#[path = "manufacturer_query.rs"]
mod manufacturers;
#[path = "mission_location_query.rs"]
mod mission_location_templates;
#[path = "reputation_query.rs"]
mod reputation;

use p4kx_extraction::{
    Archive, Build, CancelToken, DataForge, DataForgeEntry, Guid, Record, SourceInventory,
    SourcedValue,
};
use serde::Serialize;

use crate::catalog_query_session::checkpoint;
use crate::catalog_v3::blueprint_category_list_read;
use crate::{CatalogError, Kind};

pub use crate::canonical_record_header::{CanonicalRecordHeader, CatalogRecordSource};
pub use crate::catalog_query_session::{
    CatalogLanguageSelection, CatalogQuerySession, ParsedCatalogQueries,
};
pub use crate::catalog_record_reference::CatalogRecordReference;
pub use crate::catalog_table::DatasetAvailability;
pub use crate::catalog_text_contract::{CanonicalText, GameTextRef, PublicNameRef, TextForms};
pub use crate::catalog_v3::authored_fields::AuthoredRestriction;
pub use crate::catalog_v3::catalog_item_alignment::{
    ItemPort, ItemPortContainer, PortItemType, PortRestrictions,
};
pub use crate::catalog_v3::gameplay_property_catalog::{
    DisplayTransformationNodeRef, DisplayTransformationNodeSource, DisplayTransformationPayload,
    DisplayTransformationSource, GameplayProperty, GameplayPropertyFields,
    PropertyNameOverrideConditionPayload, PropertyNameOverrideConditionSource,
    PropertyNameOverrideFields, PropertyNameOverrideSource,
};
pub use crate::catalog_v3::reputation_faction_definition::FactionReputationDefinition;
pub use crate::catalog_v3::reputation_scope_definition::{ScopeDefinition, StandingLink};
pub use crate::catalog_v3::reputation_standing_definition::StandingDefinition;
pub use crate::catalog_v3::tag_catalog::OriginalTagPath;
pub use crate::definition_reference::DefinitionStatus;
pub use crate::location_template::{LocationName, MissionLocationTemplate};
pub use crate::manufacturer::{
    ManufacturerAudioTrigger, ManufacturerDefinition, ManufacturerDisplayFeatures,
    ManufacturerLocalization,
};
pub use crate::mission_location_tag::{MissionLocationTag, MissionLocationTagTerm};
pub use crate::resource_number::ResourceNumber;
pub use crate::source_value_catalog::{
    CatalogGuid, OriginalErrorPool, OriginalErrorTable, OriginalInstanceLocation,
    OriginalValueErrorFacts, SourceArenaOutput, SourceDataType, SourceDeclaration, SourceField,
    SourceNode, SourcePayload, SourceRecordReference, SourceText, SourceValueRef,
    SourceWeakReference, SourceWeakTarget,
};
pub use blueprint_categories::{
    BlueprintCategory, BlueprintCategorySnapshot, read_blueprint_categories,
    read_blueprint_categories_from,
};
pub use gameplay_properties::{
    CatalogPublicName, GameplayPropertySnapshot, read_gameplay_properties,
    read_gameplay_properties_from,
};

pub use harvest_provider_presets::{
    HarvestableProviderPresetSnapshot, read_harvestable_provider_presets,
    read_harvestable_provider_presets_from,
};

pub use item_ports::{ItemPortSnapshot, with_item_ports, with_item_ports_from};
pub use manufacturers::{ManufacturerSnapshot, with_manufacturers, with_manufacturers_from};
pub use mission_location_templates::{
    MissionLocationTemplateSnapshot, with_mission_location_templates,
    with_mission_location_templates_from,
};
pub use reputation::{
    ReputationRows, ReputationSelection, ReputationSnapshot, with_reputation, with_reputation_from,
};

pub use crate::location_condition_kind::LocationConditionKind;
pub use crate::mission_generator::GeneratorDefinition as ContractGeneratorDefinition;
pub use crate::mission_generator::completion_tag_results::ContractCompletionTagResult;
pub use crate::mission_generator::item_properties::GeneratorMissionItemProperty;
pub use crate::mission_generator::item_reward_results::{
    ContractItemRewardResult, ItemRewardTarget,
};
pub use crate::mission_property_set::MissionPropertyLevel;
pub use crate::resource_integer::ResourceInt64;
pub use crate::template_source_refs::{DeclarationBinding, SourceFieldRef};
pub use crate::template_value_meaning::{
    TemplateDatasetCondition, TemplateMissionItemSelection, TemplateSpecificItems,
};
pub use contract_generators::{
    ContractGeneratorProjectionFailure, ContractGeneratorProjectionNote, ContractGeneratorSnapshot,
    with_contract_generators, with_contract_generators_from,
};

/// Selects physical Records within the queried dataset and supplied DataForge.
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub enum CatalogRecordSelection {
    /// All Records admitted to the queried dataset by canonical catalog policy.
    All,
    /// All admitted Records with this GUID, including duplicates.
    Guid(Guid),
    /// The Record with this physical index in the supplied DataForge.
    RecordIndex(u32),
}

impl CatalogRecordSelection {
    fn for_each<'source>(
        self,
        forge: &'source DataForge<'_>,
        cancel: Option<&CancelToken>,
        mut read: impl FnMut(&Record<'source>) -> Result<(), CatalogError>,
    ) -> Result<(), CatalogError> {
        match self {
            Self::RecordIndex(index) => {
                if let Some(record) = forge.record(index) {
                    read(&record)?;
                }
            }
            Self::All | Self::Guid(_) => {
                for record in forge.records() {
                    checkpoint(cancel)?;
                    if matches!(self, Self::Guid(guid) if record.guid() != guid) {
                        continue;
                    }
                    read(&record)?;
                }
            }
        }
        Ok(())
    }
}

/// A category-list declaration with its original ordered reference slots.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct BlueprintCategoryList {
    /// The original physical Record identity and header fields.
    #[serde(flatten)]
    pub source: CatalogRecordSource,
    /// The authored slots, including duplicates, nulls and unresolved references.
    pub categories: SourcedValue<Vec<SourcedValue<CatalogRecordReference>>>,
}

/// The selected category lists and the source inventory needed to resolve their uses.
#[derive(Debug)]
#[non_exhaustive]
#[must_use]
pub struct BlueprintCategoryListSnapshot {
    /// The installed Archive build, when available.
    pub build: Option<Build>,
    /// Availability of the category-list source family.
    pub availability: DatasetAvailability,
    /// Selected rows in canonical GUID order, then by physical index for tied GUIDs.
    pub rows: Vec<BlueprintCategoryList>,
    /// Original sources and uses retained by this query.
    pub source_inventory: SourceInventory,
    /// Coverage of the headers and bodies considered by the selection.
    pub coverage: CatalogRecordCoverage,
}

/// Physical Records that could not be read fully, or were skipped by selection policy.
#[derive(Debug, Default)]
#[non_exhaustive]
pub struct CatalogRecordCoverage {
    /// Physical indices whose headers could not be classified.
    pub unreadable_headers: Vec<u32>,
    /// Selected physical indices whose bodies could not be read.
    pub unreadable_bodies: Vec<u32>,
    /// Selected physical indices with recoverable field projection issues.
    pub incomplete_fields: Vec<u32>,
    /// The number of development leftovers excluded by catalog policy.
    pub skipped_leftovers: usize,
}

/// Reads category-list declarations from the Archive's original DataForge.
///
/// # Errors
///
/// Returns input, source evidence or cancellation errors. Recoverable Record and field
/// issues are described in the returned coverage.
pub fn read_blueprint_category_lists(
    archive: &Archive,
    selection: CatalogRecordSelection,
    cancel: Option<&CancelToken>,
) -> Result<BlueprintCategoryListSnapshot, CatalogError> {
    CatalogQuerySession::open(archive, CatalogLanguageSelection::Selected(&[]), cancel)?
        .read_blueprint_category_lists(selection, cancel)
}

/// Reads category-list declarations from an already decoded entry of the same Archive.
///
/// # Errors
///
/// Returns the errors of [`read_blueprint_category_lists`], or [`CatalogError::SourceOriginMismatch`]
/// if the entry's proven origin does not match the supplied Archive.
pub fn read_blueprint_category_lists_from(
    archive: &Archive,
    entry: &DataForgeEntry,
    selection: CatalogRecordSelection,
    cancel: Option<&CancelToken>,
) -> Result<BlueprintCategoryListSnapshot, CatalogError> {
    CatalogQuerySession::from_entry(
        archive,
        entry,
        CatalogLanguageSelection::Selected(&[]),
        cancel,
    )?
    .read_blueprint_category_lists(selection, cancel)
}

impl CatalogQuerySession<'_> {
    /// Reads category-list declarations and their original ordered references.
    ///
    /// # Errors
    ///
    /// Returns input, source evidence or cancellation errors. Recoverable gaps appear in coverage.
    pub fn read_blueprint_category_lists(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
    ) -> Result<BlueprintCategoryListSnapshot, CatalogError> {
        self.parsed(cancel)?
            .read_blueprint_category_lists(selection, cancel)
    }
}

impl ParsedCatalogQueries<'_> {
    /// Reads category-list declarations and their original ordered references.
    ///
    /// # Errors
    ///
    /// Returns input, source evidence or cancellation errors. Recoverable gaps appear in coverage.
    pub fn read_blueprint_category_lists(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
    ) -> Result<BlueprintCategoryListSnapshot, CatalogError> {
        self.project(cancel, |archive, entry, forge| {
            read_archive(archive, entry, forge, selection, cancel)
        })
    }
}

fn read_archive(
    archive: &Archive,
    entry: &DataForgeEntry,
    forge: &DataForge<'_>,
    selection: CatalogRecordSelection,
    cancel: Option<&CancelToken>,
) -> Result<BlueprintCategoryListSnapshot, CatalogError> {
    let projected = unlocalized_record_query::read(
        archive,
        entry,
        forge,
        Kind::BlueprintCategoryLists,
        selection,
        cancel,
        |tree, input, admission| {
            let read = blueprint_category_list_read::read(tree, input, admission, cancel)?;
            let row = BlueprintCategoryList {
                source: read.value.declaration.source.into(),
                categories: read.value.declaration.categories,
            };
            Ok((row, read.notes))
        },
    )?;
    Ok(BlueprintCategoryListSnapshot {
        build: archive.build().cloned(),
        availability: projected.availability,
        rows: projected.rows,
        source_inventory: projected.source_inventory,
        coverage: projected.coverage,
    })
}
