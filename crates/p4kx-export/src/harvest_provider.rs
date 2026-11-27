// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, DataForge, RecordTree, SourceUses, SourcedValue, Value};
use schemars::JsonSchema;
use serde::Serialize;

use super::super::authored_fields::Observed;
use super::super::prepared_definition::PreparedDefinition;
use super::harvest_provider_dependencies::{ProviderDependencies, SpawnSelection};
use super::harvest_provider_override::{ModifierTarget, read_modifier_target};
use crate::CatalogError;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_record_reference::CatalogRecordReference;
use crate::mining_locations::MiningRecordSource;
use crate::record_observation::RecordObservation;
use crate::resource_number::ResourceNumber;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;

type Slots<T> = SourcedValue<Vec<SourcedValue<T>>>;
type RecordLink = SourcedValue<Box<CatalogRecordReference>>;

/// One provider preset and its original ordered group and area declarations.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct HarvestableProviderPreset {
    /// Sourced identity of the physical provider Record.
    #[serde(flatten)]
    pub source: MiningRecordSource,
    /// Original group slots from the provider.
    pub groups: Slots<HarvestableGroup>,
    /// Original area slots from the provider.
    pub areas: Slots<HarvestableArea>,
}

impl HarvestableProviderPreset {
    pub(crate) fn order(rows: &mut [Self]) {
        rows.sort_by_cached_key(|row| row.source.order_key());
    }
}

/// One authored provider group.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct HarvestableGroup {
    /// Authored group name.
    pub group_name: SourcedValue<String>,
    /// Authored probability and its storage representation.
    pub group_probability: SourcedValue<ResourceNumber>,
    /// Original ordered element slots.
    pub harvestables: Slots<HarvestableElement>,
}

/// One original element of a provider group.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct HarvestableElement {
    /// Original reference to the harvestable preset.
    pub harvestable: RecordLink,
    /// Original explicit entity selection.
    pub harvestable_entity_class: RecordLink,
    /// Original setup reference.
    pub harvestable_setup: RecordLink,
    /// Original clustering reference.
    pub clustering: RecordLink,
    /// Authored relative probability.
    pub relative_probability: SourcedValue<ResourceNumber>,
    /// Original ordered geometry slots.
    pub geometries: Slots<HarvestableGeometry>,
    /// Exact dependency selection used to obtain the chosen mining parameters.
    pub spawn: SpawnSelection,
}

/// One original geometry restriction of an element or area modifier.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct HarvestableGeometry {
    /// The original geometry Tag reference.
    pub tag: RecordLink,
}

/// One original area declaration.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct HarvestableArea {
    /// Authored debug group name.
    pub name: SourcedValue<String>,
    /// Authored global modifier.
    pub global_modifier: SourcedValue<ResourceNumber>,
    /// Original owned area type.
    pub area_type: SourcedValue<HarvestableAreaType>,
    /// Original ordered modifier slots.
    pub modifiers: Slots<AreaModifier>,
}

/// One original area type and its object preset selectors.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct HarvestableAreaType {
    /// Exact concrete type observed in the owned area value.
    pub source_type: SourcedValue<String>,
    /// Original ordered object preset paths.
    pub object_preset_paths: Slots<String>,
}

/// One area modifier and its original weak target.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct AreaModifier {
    /// The original pointer target and optional exact provider slot.
    pub harvestable_element: SourcedValue<Box<ModifierTarget>>,
    /// Authored modifier number.
    pub modifier: SourcedValue<ResourceNumber>,
    /// Original ordered geometry slots.
    pub geometries: Slots<HarvestableGeometry>,
}

pub(crate) fn read<'source>(
    tree: &RecordTree<'source>,
    forge: &'source DataForge<'source>,
    input: &OriginalCatalogInput<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    cancel: Option<&CancelToken>,
    dependencies: &ProviderDependencies,
) -> Result<(HarvestableProviderPreset, Vec<FieldError>), CatalogError> {
    let (prepared, notes) = PreparedDefinition::read_original(
        tree,
        input,
        admission,
        cancel,
        |source, fields, notes| {
            let groups =
                fields.declared_class_array("harvestableGroups", notes, |group, notes| {
                    read_group(group, forge, input, cancel, dependencies, notes)
                })?;
            let areas = fields.declared_class_array("areas", notes, |area, notes| {
                read_area(area, &source.record_index, &input.origins().uses, notes)
            })?;
            Ok(HarvestableProviderPreset {
                source: source.into(),
                groups,
                areas,
            })
        },
    )?;
    Ok((prepared.into_definition(), notes))
}

fn read_group<'source>(
    group: Observed<'_, '_, '_, 'source>,
    forge: &'source DataForge<'source>,
    input: &OriginalCatalogInput<'_, 'source>,
    cancel: Option<&CancelToken>,
    dependencies: &ProviderDependencies,
    notes: &mut Vec<FieldError>,
) -> Result<HarvestableGroup, CatalogError> {
    Ok(HarvestableGroup {
        group_name: group.harvest_text("groupName", notes)?,
        group_probability: group.authored_number("groupProbability", notes)?,
        harvestables: group.declared_class_array("harvestables", notes, |element, notes| {
            let (harvestable, harvestable_entity_class, spawn) =
                dependencies.select(element, forge, input, cancel, notes)?;
            Ok(HarvestableElement {
                harvestable,
                harvestable_entity_class,
                harvestable_setup: record_link(element, "harvestableSetup", notes)?,
                clustering: record_link(element, "clustering", notes)?,
                relative_probability: element.authored_number("relativeProbability", notes)?,
                geometries: geometries(element, notes)?,
                spawn,
            })
        })?,
    })
}

fn read_area(
    area: Observed<'_, '_, '_, '_>,
    owner: &SourcedValue<u32>,
    sources: &SourceUses,
    notes: &mut Vec<FieldError>,
) -> Result<HarvestableArea, CatalogError> {
    let concrete = match area.structure()?.field("areaType") {
        Some(Value::Struct(value)) => Some(value.type_name()),
        _ => None,
    };
    let kinds = concrete.as_slice();
    Ok(HarvestableArea {
        name: area.harvest_text("debugGroupName", notes)?,
        global_modifier: area.authored_number("globalModifier", notes)?,
        area_type: area.owned("areaType", kinds, notes, |area_type, notes| {
            Ok(HarvestableAreaType {
                source_type: area_type.source_type_value()?.map(str::to_owned),
                object_preset_paths: area_type.harvest_paths("objectPresetPaths", notes)?,
            })
        })?,
        modifiers: area.declared_class_array("modifiers", notes, |modifier, notes| {
            Ok(AreaModifier {
                harvestable_element: read_modifier_target(modifier, owner, sources, notes)?,
                modifier: modifier.authored_number("harvestableModifier", notes)?,
                geometries: geometries(modifier, notes)?,
            })
        })?,
    })
}

fn geometries(
    owner: Observed<'_, '_, '_, '_>,
    notes: &mut Vec<FieldError>,
) -> Result<Slots<HarvestableGeometry>, CatalogError> {
    owner.declared_class_array("geometries", notes, |geometry, notes| {
        Ok(HarvestableGeometry {
            tag: record_link(geometry, "tag", notes)?,
        })
    })
}

pub(super) fn record_link(
    owner: Observed<'_, '_, '_, '_>,
    name: &str,
    notes: &mut Vec<FieldError>,
) -> Result<RecordLink, CatalogError> {
    Ok(owner
        .record_link(name, notes)?
        .map(|link| Box::new((*link).into_catalog())))
}
