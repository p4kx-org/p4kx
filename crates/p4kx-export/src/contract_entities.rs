// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_number_capture::CapturedNumber;
use crate::catalog_text_contract::{GameTextRef, PublicNameRef};
use crate::catalog_v3::authored_fields::ship_presentation::{
    ShipAttachmentManufacturer, ShipVehicleManufacturer,
};
use crate::catalog_v3::authored_fields::ship_source_read::ShipPlayabilityInputs;
use crate::contract::every_field_required;
use crate::contract_insurance::ShipInsurance;
use crate::guid_text::CatalogGuid;
use crate::resource_number::ResourceNumber;
use crate::ship::ShipIdentity;
use crate::{MassBasis, Playability, VehicleKind};

pub(crate) use crate::item::Item;

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
    #[serde(flatten)]
    pub(crate) header: CanonicalRecordHeader<'a>,
    #[serde(skip)]
    #[schemars(skip)]
    pub(crate) identity: ShipIdentity<'a>,
    #[serde(skip)]
    #[schemars(skip)]
    pub(crate) source_numbers: Vec<CapturedNumber>,
    /// The name, from `vehicleName` of the vehicle parameters.
    pub(crate) name: SourcedValue<PublicNameRef>,
    /// The description, from `vehicleDescription` of the vehicle parameters.
    #[schemars(extend("x-gameText" = true))]
    pub(crate) description: SourcedValue<GameTextRef>,
    /// The role, from `vehicleRole`, e.g. `@vehicle_class_lightfreight`.
    pub(crate) role: SourcedValue<PublicNameRef>,
    /// The career, from `vehicleCareer`, e.g. `@vehicle_focus_transporter`.
    pub(crate) career: SourcedValue<PublicNameRef>,
    /// The original crew-size field of the selected Vehicle component.
    pub(crate) crew_size: SourcedValue<ResourceNumber>,
    /// The selected `VehicleComponentParams` occurrence and its manufacturer field.
    pub(crate) vehicle_manufacturer: ShipVehicleManufacturer<'a>,
    /// The selected `AttachDef` on the ship and its independent `Manufacturer` field.
    pub(crate) attachment_manufacturer: ShipAttachmentManufacturer<'a>,
    /// The exact authored vehicle selector, independently of normalized Archive lookup paths.
    pub(crate) vehicle_definition: SourcedValue<&'a str>,
    /// The kind derived from the original Record path and effective rolling parts.
    #[schemars(with = "SourcedValue<String>")]
    pub(crate) vehicle_kind: SourcedValue<VehicleKind>,
    /// The original `Size` of the selected `AttachDef`.
    pub(crate) size: SourcedValue<ResourceNumber>,
    /// Hull mass in kilograms from the selected Parts or Physics inputs, rounded to four decimals.
    /// A positive mass that rounds to zero remains present.
    pub(crate) mass: SourcedValue<f64>,
    /// The `parts` or `physics` basis selected for a present `mass`.
    #[schemars(with = "SourcedValue<String>")]
    pub(crate) mass_basis: SourcedValue<MassBasis>,
    /// The sum of positive structural part `damageMax` values, rounded to four decimals.
    pub(crate) hull_health: SourcedValue<f64>,
    /// The cargo capacity in standard cargo units (SCU) of the cargo grids the default Loadout
    /// reaches, rounded to four decimals: 0 for a ship without a cargo grid, `null` when it has
    /// grids and none can be read. With `cargoGridsWithIssue` above 0 the value is a lower bound.
    pub(crate) cargo_scu: Option<f64>,
    /// How many cargo grids of the default Loadout have no readable capacity, e.g. because their
    /// container Record does not exist.
    pub(crate) cargo_grids_with_issue: usize,
    /// The sourced classification selected by the original Ship rules.
    #[schemars(with = "SourcedValue<String>")]
    pub(crate) playability: SourcedValue<Playability>,
    /// The GUID of the base of the variant group the ship belongs to: the ships that share the
    /// vehicle definition and the vehicle implementation after the Modification. `null` for a base
    /// and for a ship that is alone in its group. It is the `guid` of an entry of `ships`.
    pub(crate) variant_of: Option<CatalogGuid>,
    /// The original selected static insurance declaration and parameter states.
    pub(crate) insurance: SourcedValue<Box<ShipInsurance<'a>>>,
    /// Authored crew-manifest inputs, independently of the derived playability result.
    pub(crate) playability_inputs: ShipPlayabilityInputs<'a>,
}
