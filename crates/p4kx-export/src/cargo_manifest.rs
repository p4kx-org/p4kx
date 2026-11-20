// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, DataForge, Guid, RecordTree, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
use super::prepared_definition::PreparedDefinition;
use crate::build::original_input::OriginalCatalogInput;
use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_number_capture::CapturedNumber;
use crate::catalog_record_reference::CatalogRecordReference;
use crate::catalog_row::CatalogRow;
use crate::catalog_table::CatalogTable;
use crate::record_observation::RecordObservation;
use crate::resource_number::ResourceNumber;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::{SourceArenaOutput, SourceRecordReference};
use crate::{CatalogError, ExportAccountingError, Kind};

pub(crate) const SOURCE_TYPE: &str = "CargoManifest";

type RecordLink<'source> = SourcedValue<Box<SourceRecordReference<'source>>>;
type OrderedSlots<T> = SourcedValue<Vec<SourcedValue<T>>>;

/// One physical `CargoManifest` definition and its original fill and spawn inputs.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct CargoManifestDefinition<'source> {
    #[serde(flatten)]
    source: CanonicalRecordHeader<'source>,
    lock_cargo_grids_on_spawn: SourcedValue<bool>,
    cargo_fill_capacity: SourcedValue<CargoFillCapacity<'source>>,
    cargo_spawn_rules: OrderedSlots<CargoSpawnRule<'source>>,
    authored_fields: SourceArenaOutput<'source>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct CargoFillCapacity<'source> {
    source_type: SourcedValue<&'source str>,
    min_capacity_range: SourcedValue<ResourceNumber>,
    max_capacity_range: SourcedValue<ResourceNumber>,
    resources: OrderedSlots<CargoResourceSlot<'source>>,
    authored_fields: SourceArenaOutput<'source>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct CargoResourceSlot<'source> {
    source_type: SourcedValue<&'source str>,
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    resource: RecordLink<'source>,
    probability: SourcedValue<ResourceNumber>,
    min_resource_allocation_percentage: SourcedValue<ResourceNumber>,
    max_resource_allocation_percentage: SourcedValue<ResourceNumber>,
    resources: SourcedValue<CargoResource<'source>>,
    authored_fields: SourceArenaOutput<'source>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct CargoResource<'source> {
    source_type: SourcedValue<&'source str>,
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    resource: RecordLink<'source>,
    probability: SourcedValue<ResourceNumber>,
    authored_fields: SourceArenaOutput<'source>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct CargoSpawnRule<'source> {
    source_type: SourcedValue<&'source str>,
    probability: SourcedValue<ResourceNumber>,
    #[schemars(with = "OrderedSlots<CatalogRecordReference>")]
    entities: SourcedValue<Vec<RecordLink<'source>>>,
    #[schemars(with = "OrderedSlots<CatalogRecordReference>")]
    resources: SourcedValue<Vec<RecordLink<'source>>>,
    quantity_algorithm_type: SourcedValue<CargoQuantityAlgorithm<'source>>,
    authored_fields: SourceArenaOutput<'source>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct CargoQuantityAlgorithm<'source> {
    source_type: SourcedValue<&'source str>,
    quantity: SourcedValue<ResourceNumber>,
    proportion: SourcedValue<ResourceNumber>,
    authored_fields: SourceArenaOutput<'source>,
}

pub(crate) type PreparedCargoManifest<'source> =
    PreparedDefinition<'source, CargoManifestDefinition<'source>>;

pub(crate) fn read<'source>(
    tree: &RecordTree<'source>,
    input: &OriginalCatalogInput<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    cancel: Option<&CancelToken>,
) -> Result<(PreparedCargoManifest<'source>, Vec<FieldError>), CatalogError> {
    if tree.type_name() != SOURCE_TYPE {
        return Err(ExportAccountingError::ConflictingSourceDatum.into());
    }
    PreparedDefinition::read_original(tree, input, admission, cancel, read_manifest)
}

pub(crate) fn table<'source>(
    forge: &DataForge<'_>,
    mut manifests: Vec<PreparedCargoManifest<'source>>,
) -> CatalogTable<CargoManifestDefinition<'source>> {
    PreparedDefinition::order(&mut manifests);
    PreparedDefinition::table(forge, SOURCE_TYPE, manifests)
}

pub(crate) fn catalog_rows<'rows>(
    rows: &'rows [PreparedCargoManifest<'_>],
) -> impl Iterator<Item = CatalogRow> + 'rows {
    PreparedDefinition::catalog_rows(rows, Kind::CargoManifests)
}

pub(crate) fn sources<'rows, 'source>(
    rows: &'rows [PreparedCargoManifest<'source>],
) -> impl Iterator<Item = (Kind, Guid, &'source str, &'rows [CapturedNumber])> {
    PreparedDefinition::sources(Kind::CargoManifests, rows)
}

fn read_manifest<'source>(
    source: CanonicalRecordHeader<'source>,
    fields: Observed<'_, '_, '_, 'source>,
    notes: &mut Vec<FieldError>,
) -> Result<CargoManifestDefinition<'source>, CatalogError> {
    let lock_cargo_grids_on_spawn = fields.boolean("lockCargoGridsOnSpawn", notes)?;
    let cargo_fill_capacity = fields.declared_owned("cargoFillCapacity", notes, read_fill)?;
    let cargo_spawn_rules = fields.declared_owned_array("cargoSpawnRules", notes, read_spawn)?;
    let authored_fields = fields.authored_members_except([
        ("lockCargoGridsOnSpawn", lock_cargo_grids_on_spawn.state()),
        ("cargoFillCapacity", cargo_fill_capacity.state()),
        ("cargoSpawnRules", cargo_spawn_rules.state()),
    ])?;
    Ok(CargoManifestDefinition {
        source,
        lock_cargo_grids_on_spawn,
        cargo_fill_capacity,
        cargo_spawn_rules,
        authored_fields,
    })
}

fn read_fill<'source>(
    fields: Observed<'_, '_, '_, 'source>,
    notes: &mut Vec<FieldError>,
) -> Result<CargoFillCapacity<'source>, CatalogError> {
    let min_capacity_range = fields.authored_number("minCapacityRange", notes)?;
    let max_capacity_range = fields.authored_number("maxCapacityRange", notes)?;
    let resources = fields.declared_owned_array("resources", notes, read_resource_slot)?;
    let authored_fields = fields.authored_members_except([
        ("minCapacityRange", min_capacity_range.state()),
        ("maxCapacityRange", max_capacity_range.state()),
        ("resources", resources.state()),
    ])?;
    Ok(CargoFillCapacity {
        source_type: fields.source_type_value()?,
        min_capacity_range,
        max_capacity_range,
        resources,
        authored_fields,
    })
}

fn read_resource_slot<'source>(
    fields: Observed<'_, '_, '_, 'source>,
    notes: &mut Vec<FieldError>,
) -> Result<CargoResourceSlot<'source>, CatalogError> {
    let resource = fields.record_link("resource", notes)?;
    let probability = fields.authored_number("probability", notes)?;
    let min_resource_allocation_percentage =
        fields.authored_number("minResourceAllocationPercentage", notes)?;
    let max_resource_allocation_percentage =
        fields.authored_number("maxResourceAllocationPercentage", notes)?;
    let resources = fields.declared_owned("resources", notes, read_cargo_resource)?;
    let authored_fields = fields.authored_members_except([
        ("resource", resource.state()),
        ("probability", probability.state()),
        (
            "minResourceAllocationPercentage",
            min_resource_allocation_percentage.state(),
        ),
        (
            "maxResourceAllocationPercentage",
            max_resource_allocation_percentage.state(),
        ),
        ("resources", resources.state()),
    ])?;
    Ok(CargoResourceSlot {
        source_type: fields.source_type_value()?,
        resource,
        probability,
        min_resource_allocation_percentage,
        max_resource_allocation_percentage,
        resources,
        authored_fields,
    })
}

fn read_cargo_resource<'source>(
    fields: Observed<'_, '_, '_, 'source>,
    notes: &mut Vec<FieldError>,
) -> Result<CargoResource<'source>, CatalogError> {
    let resource = fields.record_link("resource", notes)?;
    let probability = fields.authored_number("probability", notes)?;
    let authored_fields = fields.authored_members_except([
        ("resource", resource.state()),
        ("probability", probability.state()),
    ])?;
    Ok(CargoResource {
        source_type: fields.source_type_value()?,
        resource,
        probability,
        authored_fields,
    })
}

fn read_spawn<'source>(
    fields: Observed<'_, '_, '_, 'source>,
    notes: &mut Vec<FieldError>,
) -> Result<CargoSpawnRule<'source>, CatalogError> {
    let probability = fields.authored_number("probability", notes)?;
    let entities = fields.record_link_array("entities", notes)?;
    let resources = fields.record_link_array("resources", notes)?;
    let quantity_algorithm_type =
        fields.declared_owned("quantityAlgorithmType", notes, read_quantity_algorithm)?;
    let authored_fields = fields.authored_members_except([
        ("probability", probability.state()),
        ("entities", entities.state()),
        ("resources", resources.state()),
        ("quantityAlgorithmType", quantity_algorithm_type.state()),
    ])?;
    Ok(CargoSpawnRule {
        source_type: fields.source_type_value()?,
        probability,
        entities,
        resources,
        quantity_algorithm_type,
        authored_fields,
    })
}

fn read_quantity_algorithm<'source>(
    fields: Observed<'_, '_, '_, 'source>,
    notes: &mut Vec<FieldError>,
) -> Result<CargoQuantityAlgorithm<'source>, CatalogError> {
    let quantity = fields.authored_integer("quantity", notes)?;
    let proportion = fields.authored_number("proportion", notes)?;
    let authored_fields = fields.authored_members_except([
        ("quantity", quantity.state()),
        ("proportion", proportion.state()),
    ])?;
    Ok(CargoQuantityAlgorithm {
        source_type: fields.source_type_value()?,
        quantity,
        proportion,
        authored_fields,
    })
}
