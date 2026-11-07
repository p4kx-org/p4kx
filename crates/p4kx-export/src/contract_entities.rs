// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

use crate::contract::{LocalisedText, every_field_required};
use crate::contract_insurance::ShipInsurance;
use crate::contract_stats::ItemStats;
use crate::guid_text::CatalogGuid;
use crate::item_variant::ItemKey;
use crate::{ItemClass, MassBasis, Playability, VehicleKind};

/// An item: an `EntityClassDefinition` Record under `entities/scitem` or `entities/legacyitems`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct Item<'a> {
    /// The original physical Record position within the source Build.
    #[serde(skip)]
    #[schemars(skip)]
    pub(crate) source_record_index: u32,
    /// The GUID of the Record.
    pub(crate) guid: CatalogGuid,
    /// The Record name without its type, e.g. `POWR_ACAS_S01_Lumen`.
    pub(crate) class_name: &'a str,
    /// The record path, without `.xml`.
    pub(crate) record_path: &'a str,
    /// The name, from `AttachDef.Localization.Name`.
    pub(crate) name: LocalisedText<'a>,
    /// The description, from `AttachDef.Localization.Description`.
    #[schemars(extend("x-gameText" = true))]
    pub(crate) description: LocalisedText<'a>,
    /// The item type of `AttachDef.Type`, e.g. `PowerPlant`, `null` without one.
    pub(crate) r#type: Option<&'a str>,
    /// The item subtype of `AttachDef.SubType`, e.g. `Power`, `null` without one.
    pub(crate) sub_type: Option<&'a str>,
    /// The size of `AttachDef.Size`, `null` without one.
    pub(crate) size: Option<i64>,
    /// The grade of `AttachDef.Grade`, `null` without one.
    pub(crate) grade: Option<i64>,
    /// The mass in kilograms of the physics controller, `null` without one.
    pub(crate) mass: Option<f32>,
    /// The GUID of the manufacturer the Reference `AttachDef.Manufacturer` links to, `null` for
    /// a null Reference.
    pub(crate) manufacturer_guid: Option<CatalogGuid>,
    /// The words of `AttachDef.Tags`, in stored order.
    pub(crate) tags: Vec<&'a str>,
    /// The display type of the purchasable parameters, e.g. `@item_DisplayType_PowerPlant`.
    #[schemars(extend("x-gameText" = true))]
    pub(crate) display_type: LocalisedText<'a>,
    /// The item class from the `Class:` line of the English description, `null` for another
    /// value or without that line. Today one of `Civilian`, `Competition`, `Industrial`,
    /// `Military` and `Stealth`; the schema keeps a string, as a new value is additive.
    #[serde(rename = "itemClass")]
    #[schemars(with = "Option<String>")]
    pub(crate) class: Option<ItemClass>,
    /// Who the item is for, `null` when no rule applies. Today one of `player`, `mission` and
    /// `special`; the schema keeps a string, as a new value is additive.
    #[schemars(with = "Option<String>")]
    pub(crate) playability: Option<Playability>,
    /// The GUID of the base of the variant group the item belongs to, `null` for a base and for an
    /// item that is alone in its group. It is the `guid` of an entry of `items`.
    pub(crate) variant_of: Option<CatalogGuid>,
    /// The typed stat blocks of a weapon, shield, quantum drive, power plant, cooler or missile,
    /// raw facts without derived values (ADR-0065); `null` for any other item.
    pub(crate) stats: Option<Box<ItemStats<'a>>>,
    #[serde(skip)]
    #[schemars(skip)]
    pub(crate) variant_key: Option<ItemKey<'a>>,
    #[serde(skip)]
    #[schemars(skip)]
    pub(crate) required_tags: Vec<&'a str>,
}

/// A paint: an item of the type `Paints` and the ships it fits.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct Paint {
    /// The `guid` of the paint's entry in `items`, which holds its name and record.
    pub(crate) item_guid: CatalogGuid,
    /// The GUIDs of the ships the paint fits, ordered by GUID; empty when it fits none. Each
    /// is the `guid` of an entry of `ships`.
    pub(crate) ship_guids: Vec<CatalogGuid>,
}

/// A ship or ground vehicle: an `EntityClassDefinition` Record under `entities/spaceships` or
/// `entities/groundvehicles`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct Ship<'a> {
    /// The original physical Record position within the source Build.
    #[serde(skip)]
    #[schemars(skip)]
    pub(crate) source_record_index: u32,
    /// The GUID of the Record.
    pub(crate) guid: CatalogGuid,
    /// The Record name without its type, e.g. `AEGS_Avenger_Titan`.
    pub(crate) class_name: &'a str,
    /// The record path, without `.xml`.
    pub(crate) record_path: &'a str,
    /// The name, from `vehicleName` of the vehicle parameters.
    pub(crate) name: LocalisedText<'a>,
    /// The description, from `vehicleDescription` of the vehicle parameters.
    #[schemars(extend("x-gameText" = true))]
    pub(crate) description: LocalisedText<'a>,
    /// The role, from `vehicleRole`, e.g. `@vehicle_class_lightfreight`.
    pub(crate) role: LocalisedText<'a>,
    /// The career, from `vehicleCareer`, e.g. `@vehicle_focus_transporter`.
    pub(crate) career: LocalisedText<'a>,
    /// The crew size, `null` without one.
    pub(crate) crew_size: Option<i64>,
    /// The GUID of the manufacturer the Reference `manufacturer` links to, `null` for a null
    /// Reference.
    pub(crate) manufacturer_guid: Option<CatalogGuid>,
    /// The path of the vehicle definition XML, `null` without one.
    pub(crate) vehicle_definition: Option<&'a str>,
    /// Spaceship or ground vehicle, `null` when the record path and the vehicle implementation
    /// do not settle it. Today one of `spaceship` and `groundVehicle`; the schema keeps a string,
    /// as a new value is additive.
    #[schemars(with = "Option<String>")]
    pub(crate) vehicle_kind: Option<VehicleKind>,
    /// The item size of `AttachDef.Size`, `null` without one.
    pub(crate) size: Option<i64>,
    /// The hull mass in kilograms, without the Loadout, the docked vehicles and the landing gear:
    /// the sum of the `mass` of the parts of the vehicle implementation after the Modification
    /// the ship selects, rounded to four decimals. `null` when neither the parts nor the physics
    /// controller state one, never 0.
    pub(crate) mass: Option<f64>,
    /// What `mass` is the sum of. Today one of `parts` and `physics`; the schema keeps a string,
    /// as a new value is additive. `null` exactly when `mass` is.
    #[schemars(with = "Option<String>")]
    pub(crate) mass_basis: Option<MassBasis>,
    /// The hull hit points: the sum of the positive `damageMax` of the structural parts of the
    /// vehicle implementation, rounded to four decimals. `null` without a vehicle implementation
    /// or when no part states a `damageMax`.
    pub(crate) hull_health: Option<f64>,
    /// The cargo capacity in standard cargo units (SCU) of the cargo grids the default Loadout
    /// reaches, rounded to four decimals: 0 for a ship without a cargo grid, `null` when it has
    /// grids and none can be read. With `cargoGridsWithIssue` above 0 the value is a lower bound.
    pub(crate) cargo_scu: Option<f64>,
    /// How many cargo grids of the default Loadout have no readable capacity, e.g. because their
    /// container Record does not exist.
    pub(crate) cargo_grids_with_issue: usize,
    /// Who the ship is for, `null` when no rule applies. Today one of `player`, `ai`, `mission`
    /// and `special`; the schema keeps a string, as a new value is additive.
    #[schemars(with = "Option<String>")]
    pub(crate) playability: Option<Playability>,
    /// The GUID of the base of the variant group the ship belongs to: the ships that share the
    /// vehicle definition and the vehicle implementation after the Modification. `null` for a base
    /// and for a ship that is alone in its group. It is the `guid` of an entry of `ships`.
    pub(crate) variant_of: Option<CatalogGuid>,
    /// The claim and expedite values of the ship's `SEntityInsuranceProperties`; `null` when the
    /// Record has none.
    pub(crate) insurance: Option<ShipInsurance<'a>>,
}
