// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

use crate::contract::every_field_required;
use crate::guid_text::CatalogGuid;

/// A mineable entity: an `EntityClassDefinition` Record under `entities/mineable`, one kind of
/// rock or deposit. Many entities share a composition; a provider preset reaches an entity through
/// the `entityGuid` of its harvestables. Entities are selected by record path and type alone, so
/// one without the components below has `null` and empty fields.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct MineableEntity<'a> {
    /// The GUID of the Record.
    pub(crate) guid: CatalogGuid,
    /// The Record name without its type, e.g. `MineableRock_AsteroidLegendary_Quantainium`.
    pub(crate) class_name: &'a str,
    /// The record path, without `.xml`.
    pub(crate) record_path: &'a str,
    /// The GUID of the `MineableComposition` Record `Components[MineableParams]` links to.
    pub(crate) mineable_composition_guid: Option<CatalogGuid>,
    /// The GUID of the `MiningGlobalParams` Record `Components[MineableParams]` links to.
    pub(crate) mining_global_params_guid: Option<CatalogGuid>,
    /// The scan signature the mining scanner shows, from 3000 to 4900 on LIVE: the slot of
    /// `signatures` that `signatureTypeParams` of the `RadarSystemGlobalParams` Record names
    /// `@hud_scanning_info_rs_signature`. `null` when the Record states no signatures, all of them
    /// are zero, `signatures` has not one number per signature type, or the Archive names no
    /// such slot (a catalog warning says so).
    pub(crate) scan_signature: Option<f32>,
    /// `radarProperties.baseSignatureParams.signatures` of `Components[SSCSignatureSystemParams]`
    /// in stored order, one number per signature type; a value that is not a finite 32-bit float
    /// is exported as 0. Empty when the Record states none.
    pub(crate) signatures: Vec<f32>,
    /// The GUID of the `RadarContactTypeEntry` Record `radarProperties.contactType` links to.
    pub(crate) radar_contact_type_guid: Option<CatalogGuid>,
}
