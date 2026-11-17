// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Typed local projection of mineable data and logical harvestable locations.

use std::collections::BTreeMap;

use serde::Serialize;

pub use crate::build::mining_locations::{read_mining_locations, read_mining_locations_from};
pub use crate::canonical_record_header::CatalogRecordSource as MiningRecordSource;
pub use crate::catalog_queries::CatalogPublicName;
pub use crate::catalog_read::original::PhysicalEntryRef;
pub use crate::catalog_record_reference::CatalogRecordReference;
pub use crate::catalog_text_contract::{GameTextRef, PublicNameRef};
pub use crate::catalog_v3::catalog_harvest_modules::harvest_dependency_selection::EntitySelection;
pub use crate::catalog_v3::catalog_harvest_modules::harvest_provider::{
    AreaModifier, HarvestableArea, HarvestableAreaType, HarvestableElement, HarvestableGeometry,
    HarvestableGroup, HarvestableProviderPreset,
};
pub use crate::catalog_v3::catalog_harvest_modules::harvest_provider_dependencies::{
    HarvestEntity, HarvestMiningParams, SpawnSelection,
};
pub use crate::catalog_v3::catalog_harvest_modules::harvest_provider_override::{
    ModifierInstance, ModifierTarget, ProviderSlot,
};
pub use crate::catalog_v3::mineable_composition_definition::{
    CompositionPart, MineableComposition,
};
pub use crate::catalog_v3::mineable_element_definition::MineableElement;
pub use crate::definition_reference::DefinitionStatus;
pub use crate::resource_float::{ResourceFloat32, ResourceFloat64};
pub use crate::resource_integer::ResourceInt64;
pub use crate::resource_number::ResourceNumber;
pub use crate::starmap_containers::instance::ContainerInstanceKey;
use p4kx_extraction::{Build, CancelToken, Guid, Language, SourceInventory, SourcedValue};

use crate::StarmapTraversalBudget;

/// Selects a language, cancellation and optional container traversal allowances.
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub struct MiningLocationOptions<'a> {
    /// The only language inspected for protected display text.
    pub language: Language,
    /// The caller's cancellation signal.
    pub cancel: Option<&'a CancelToken>,
    /// Optional physical-source and per-system placement work allowances.
    pub starmap_budget: StarmapTraversalBudget,
}

impl<'a> MiningLocationOptions<'a> {
    /// Selects the language and the optional cancellation signal for one read.
    #[must_use]
    pub const fn new(language: Language, cancel: Option<&'a CancelToken>) -> Self {
        Self {
            language,
            cancel,
            starmap_budget: StarmapTraversalBudget::new(),
        }
    }
}

/// Owned typed rows from the selected mining and placement readers.
///
/// Record identity, mineable elements, compositions, protected text and selected placements
/// carry original evidence. Logical location aggregates use the Catalog's convenience semantics.
#[derive(Debug)]
#[non_exhaustive]
pub struct MiningLocationSnapshot {
    /// The build identified by the installed Archive, when a manifest is available.
    pub build: Option<Build>,
    /// The requested language.
    pub language: Language,
    /// The mineable element definitions.
    pub mineable_elements: Vec<MineableElement>,
    /// The mineable compositions, including their authored shares.
    pub mineable_compositions: Vec<MineableComposition>,
    /// The harvestable provider presets and their resolved mining links.
    pub harvestable_provider_presets: Vec<HarvestableProviderPreset>,
    /// The logical locations that use a known provider preset.
    pub harvestable_locations: Vec<HarvestableLocation>,
    /// The selected physical harvest occurrences, with their original routes where reached.
    pub harvestable_occurrences: Vec<HarvestableOccurrence>,
    /// The Starmap object definitions and their selected placements.
    pub starmap_objects: Vec<MiningStarmapObject>,
    /// Resource types physically referenced by the elements and their first refined products.
    pub resource_types: Vec<MiningResourceType>,
    /// Canonical public names referenced by the selected resource types.
    pub public_names: Vec<CatalogPublicName>,
    /// Canonical protected text associated with the selected records.
    pub game_texts: Vec<MiningGameText>,
    /// The physical sources and uses captured by this read.
    pub source_inventory: SourceInventory,
    /// Observed read gaps and diagnostics.
    pub coverage: MiningLocationCoverage,
}

/// Observed availability of the original inputs for this projection.
#[derive(Debug, Default)]
#[non_exhaustive]
pub struct MiningLocationCoverage {
    /// True when the requested localization entry was loaded.
    pub language_available: bool,
    /// True when the Archive directory reported a known inventory problem.
    pub archive_inventory_incomplete: bool,
    /// Records whose path or selected header could not be read.
    pub unreadable_record_headers: usize,
    /// Selected Record bodies that could not be read.
    pub unreadable_record_bodies: usize,
    /// Original text fields that could not be projected.
    pub unreadable_text_fields: usize,
    /// Authored Starmap relationship fields with projection diagnostics.
    pub unreadable_starmap_relationships: usize,
    /// Original mining definition fields with projection diagnostics.
    pub unreadable_mining_fields: usize,
    /// Observed element resource and ore refinement links with no compatible physical target.
    pub unresolved_resource_links: usize,
    /// Distinct physically linked `ResourceType` Records whose bodies could not be read.
    pub unreadable_resource_types: usize,
    /// Distinct provider links that could not be resolved.
    pub unreadable_provider_links: usize,
    /// True when the world placement traversal reported incomplete inventory.
    pub placement_inventory_incomplete: bool,
    /// Unreadable, missing or malformed container occurrences reported by the world reader.
    pub unavailable_placement_occurrences: usize,
    /// Distinct container preset scans that failed.
    pub unreadable_harvest_containers: usize,
    /// Locations for which a malformed or incomplete parent chain prevented a known link.
    pub locations_with_unresolved_parent: usize,
}

/// A directly linked resource and its original name and refinement fields.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct MiningResourceType {
    /// The resource's physical Record identity and header fields.
    #[serde(flatten)]
    pub source: MiningRecordSource,
    /// The unmodified reference into the public-name rows of the mining result.
    pub name: SourcedValue<PublicNameRef>,
    /// The original refined `ResourceType` reference and its source state.
    pub refined_version: SourcedValue<CatalogRecordReference>,
}

/// The Starmap object used by the logical location join.
#[derive(Debug)]
#[non_exhaustive]
pub struct MiningStarmapObject {
    /// The authored Record identity.
    pub source: MiningRecordSource,
    /// The canonical protected-text ID for the Starmap name.
    pub name: SourcedValue<GameTextRef>,
    /// The name of the linked Starmap type, when available.
    pub type_name: Option<String>,
    /// The selected system folder.
    pub system: SourcedValue<String>,
    /// The entity name of the selected placement.
    pub entity_name: SourcedValue<String>,
    /// The original DataForge parent reference, including unresolved targets.
    pub parent: SourcedValue<CatalogRecordReference>,
    /// The nearest enclosing Starmap object in the physical container route.
    pub placed_under_guid: SourcedValue<Guid>,
    /// The selected world position in metres, to millimetre precision.
    pub position: SourcedValue<[f64; 3]>,
    /// True when the object is a mining claim marker.
    pub mining_claim: bool,
}

/// One protected canonical text row used by the projection.
#[derive(Debug)]
#[non_exhaustive]
pub struct MiningGameText {
    /// The canonical protected text identifier.
    pub id: SourcedValue<GameTextRef>,
    /// The original localization key.
    pub key: SourcedValue<String>,
    /// Translation forms for the requested language, keyed by language code.
    pub translations: SourcedValue<BTreeMap<String, MiningTextForms>>,
}

/// The exact decoded localization value and the rendered text form.
#[derive(Debug)]
#[non_exhaustive]
pub struct MiningTextForms {
    /// The source text as decoded from the localization entry.
    pub source_text: SourcedValue<String>,
    /// The display text from the same source.
    pub display_text: SourcedValue<String>,
}

/// One logical Catalog location for a provider preset.
#[derive(Debug)]
#[non_exhaustive]
pub struct HarvestableLocation {
    /// The provider GUID.
    pub preset_guid: Guid,
    /// The associated non-claim Starmap object, when one is known.
    pub starmap_object_guid: Option<Guid>,
    /// The owning system folder name.
    pub system: String,
    /// The archive path of the selected physical container.
    pub container: String,
}

/// One confirmed physical use of an admitted provider preset.
#[derive(Debug)]
#[non_exhaustive]
pub struct HarvestableOccurrence {
    /// The first provider GUID in the verified original SOC member.
    pub preset_guid: SourcedValue<Guid>,
    /// The physical container Entry.
    pub container: SourcedValue<PhysicalEntryRef>,
    /// The same authored container path carried by the logical location.
    pub container_path: SourcedValue<String>,
    /// The owning system folder.
    pub system: String,
    /// The nearest resolved non-claim object in the logical join.
    pub starmap_object_guid: Option<Guid>,
    /// The observed traversal classification and reached identity.
    pub route: HarvestableOccurrenceRoute,
}

/// Whether the original world walk reached the container it inspected.
#[derive(Debug)]
#[non_exhaustive]
pub enum HarvestableOccurrenceRoute {
    /// A route reached this occurrence.
    Reached(ReachedHarvestableOccurrence),
    /// The occurrence came from directory inventory outside the routes reached by this walk.
    Unreached,
}

/// The original route and parent of a reached physical occurrence.
#[derive(Debug)]
#[non_exhaustive]
pub struct ReachedHarvestableOccurrence {
    /// The exact system root Entry.
    pub root: SourcedValue<PhysicalEntryRef>,
    /// The root and ordered Child coordinates of this occurrence.
    pub id: SourcedValue<ContainerInstanceKey>,
    /// The original immediate containing occurrence, when present.
    pub parent: SourcedValue<ContainerInstanceKey>,
    /// The nearest original record selector along the route, including mining claim markers.
    pub containing_record_guid: SourcedValue<Guid>,
}
