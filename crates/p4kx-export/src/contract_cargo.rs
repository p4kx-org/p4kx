// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

use crate::contract::every_field_required;
use crate::guid_text::CatalogGuid;

/// A cargo manifest: a `CargoManifest` Record under `cargomanifest`, which tells the game how to
/// fill the cargo grid of a ship it spawns.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct CargoManifest<'a> {
    /// The GUID of the Record.
    pub(crate) guid: CatalogGuid,
    /// The Record name without its type, e.g. `LegalCargo_TSG`.
    pub(crate) class_name: &'a str,
    /// The record path, without `.xml`.
    pub(crate) record_path: &'a str,
    /// How the capacity is shared among `resources`: the type of `cargoFillCapacity`.
    pub(crate) fill_type: Option<CargoFillType>,
    /// The lowest share of the cargo capacity that is filled (`minCapacityRange`, 0 to 1);
    /// `null` for a `custom` fill, which states shares per resource.
    pub(crate) min_capacity_range: Option<f32>,
    /// The highest share of the cargo capacity that is filled (`maxCapacityRange`, 0 to 1);
    /// `null` for a `custom` fill.
    pub(crate) max_capacity_range: Option<f32>,
    /// The `lockCargoGridsOnSpawn` flag, `null` when absent.
    pub(crate) lock_cargo_grids_on_spawn: Option<bool>,
    /// The resources of `cargoFillCapacity`, in stored order.
    pub(crate) resources: Vec<CargoManifestResource>,
    /// The `cargoSpawnRules`, in stored order.
    pub(crate) spawn_rules: Vec<CargoSpawnRule<'a>>,
}

/// How a cargo manifest shares the capacity among its resources.
#[derive(Debug, Clone, Copy, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = crate::schema_vocabulary::open_string_vocabulary)]
pub(crate) enum CargoFillType {
    /// `CargoFillCapacityValue_Even`: the capacity is split evenly.
    Even,
    /// `CargoFillCapacityValue_Random`: the split is random.
    Random,
    /// `CargoFillCapacityValue_Custom`: every resource states its own share.
    Custom,
}

/// One resource of a cargo manifest.
#[derive(Debug, Clone, Copy, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct CargoManifestResource {
    /// The GUID of the `ResourceType` Record, the `subtypeGuid` of a commodity; `null` when the
    /// resource is absent. A GUID that no commodity of the catalog carries is kept.
    pub(crate) resource_guid: Option<CatalogGuid>,
    /// The `probability` that the resource is picked, as stored.
    pub(crate) probability: Option<f32>,
    /// The lowest share of the capacity of a `custom` fill
    /// (`minResourceAllocationPercentage`, as stored); `null` for the other fills.
    pub(crate) min_allocation: Option<f32>,
    /// The highest share of the capacity of a `custom` fill
    /// (`maxResourceAllocationPercentage`, as stored); `null` for the other fills.
    pub(crate) max_allocation: Option<f32>,
}

/// A spawn rule of a cargo manifest: a set of entities or resources the game adds to the cargo.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct CargoSpawnRule<'a> {
    /// Whether the rule lists entities or resources.
    pub(crate) kind: CargoSpawnKind,
    /// The `probability` of the rule, as stored.
    pub(crate) probability: Option<f32>,
    /// The `EntityClassDefinition` Records of an `entities` rule, in stored order.
    pub(crate) entity_guids: Vec<CatalogGuid>,
    /// The `ResourceType` Records of a `resources` rule, in stored order.
    pub(crate) resource_guids: Vec<CatalogGuid>,
    /// The struct type of `quantityAlgorithmType`, e.g. `CargoManifestFixedQuantityDef`.
    pub(crate) quantity_algorithm: Option<&'a str>,
    /// The `quantity` of a fixed quantity; `null` for another algorithm.
    pub(crate) quantity: Option<i64>,
    /// The `proportion` of a proportion quantity; `null` for another algorithm.
    pub(crate) proportion: Option<f32>,
}

/// What a spawn rule of a cargo manifest lists.
#[derive(Debug, Clone, Copy, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = crate::schema_vocabulary::open_string_vocabulary)]
pub(crate) enum CargoSpawnKind {
    /// `CargoSpawnEntityDef`: entities.
    Entities,
    /// `CargoSpawnResourceDef`: resources.
    Resources,
}
