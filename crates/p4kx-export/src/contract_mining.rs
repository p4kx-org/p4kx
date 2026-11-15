// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use crate::catalog_text_contract::GameTextRef;
use crate::contract::every_field_required;
use crate::guid_text::CatalogGuid;

/// A mineable element: a `MineableElement` Record under `mining/mineableelements`. Several
/// elements may link to the same `ResourceType`, so none is picked for a commodity.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct MineableElement<'a> {
    /// The GUID of the Record.
    pub(crate) guid: CatalogGuid,
    /// The Record name without its type, e.g. `agricium_ore`.
    pub(crate) class_name: &'a str,
    /// The record path, without `.xml`.
    pub(crate) record_path: &'a str,
    /// The GUID of the `ResourceType` Record the Reference `resourceType` links to. It equals a
    /// commodity's `subtypeGuid` only when the catalog has that commodity (20 of the 41 linked
    /// `ResourceType`s on LIVE). `null` for a null or dangling Reference.
    pub(crate) resource_type_guid: Option<CatalogGuid>,
    /// `elementInstability`, from 0 to 1000 on LIVE; `null` when the Record states none.
    pub(crate) instability: Option<f32>,
    /// `elementResistance`, from -1 to 1 on LIVE.
    pub(crate) resistance: Option<f32>,
    /// `elementOptimalWindowMidpoint`, from 0.3 to 1 on LIVE.
    pub(crate) optimal_window_midpoint: Option<f32>,
    /// `elementOptimalWindowMidpointRandomness`, from 0 to 0.3 on LIVE.
    pub(crate) optimal_window_midpoint_randomness: Option<f32>,
    /// `elementOptimalWindowThinness`, from -1 to 10 on LIVE.
    pub(crate) optimal_window_thinness: Option<f32>,
    /// `elementExplosionMultiplier`, from -1000 to 260 on LIVE.
    pub(crate) explosion_multiplier: Option<f32>,
    /// `elementClusterFactor`, from 0 to 1 on LIVE.
    pub(crate) cluster_factor: Option<f32>,
}

/// A refining process: a `RefiningProcess` Record under `refiningprocess`, one method of a
/// refinery. The DataForge holds no yields.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct RefiningProcess<'a> {
    /// The GUID of the Record.
    pub(crate) guid: CatalogGuid,
    /// The Record name without its type, e.g. `processingtype-fastcareful`.
    pub(crate) class_name: &'a str,
    /// The record path, without `.xml`.
    pub(crate) record_path: &'a str,
    /// The name of the method, from `processName`. The localisation key and the texts are game
    /// texts.
    #[schemars(extend("x-gameText" = true))]
    pub(crate) process_name: SourcedValue<GameTextRef>,
    /// `refiningSpeed`, one of `Slow`, `Normal` and `Fast` on LIVE; `null` when the Record states
    /// none. The schema keeps a string, as a new value is additive.
    pub(crate) refining_speed: Option<&'a str>,
    /// `refiningQuality`, one of `Wasteful`, `Normal` and `Careful` on LIVE; `null` when the Record
    /// states none. The schema keeps a string, as a new value is additive.
    pub(crate) refining_quality: Option<&'a str>,
}

/// A mineable composition: a `MineableComposition` Record under `mining/rockcompositionpresets`,
/// the elements one rock or deposit is made of.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct MineableComposition<'a> {
    /// The GUID of the Record.
    pub(crate) guid: CatalogGuid,
    /// The Record name without its type, e.g. `Asteroid_CType_Iron`.
    pub(crate) class_name: &'a str,
    /// The record path, without `.xml`.
    pub(crate) record_path: &'a str,
    /// The HUD label of the deposit, from `depositName`. The localisation key and the texts are
    /// game texts.
    #[schemars(extend("x-gameText" = true))]
    pub(crate) deposit_name: SourcedValue<GameTextRef>,
    /// `minimumDistinctElements`, from 1 to 4 on LIVE; `null` when the Record states none.
    pub(crate) minimum_distinct_elements: Option<i64>,
    /// The parts, from `compositionArray` in stored order.
    pub(crate) parts: Vec<CompositionPart>,
}

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

/// One part of a mineable composition: an element with the range of its share.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct CompositionPart {
    /// The GUID of the `MineableElement` Record the Reference `mineableElement` links to; `null`
    /// for a null Reference. The catalog lists the element only when it is kept.
    pub(crate) mineable_element_guid: Option<CatalogGuid>,
    /// `minPercentage`, from 1 to 100 on LIVE.
    pub(crate) min_percentage: Option<f32>,
    /// `maxPercentage`, from 5 to 100 on LIVE.
    pub(crate) max_percentage: Option<f32>,
    /// `probability` that the part occurs, from 0.02 to 1 on LIVE.
    pub(crate) probability: Option<f32>,
    /// `curveExponent`, from 1 to 5 on LIVE.
    pub(crate) curve_exponent: Option<f32>,
    /// `qualityScale`, from 0.49 to 1 on LIVE.
    pub(crate) quality_scale: Option<f32>,
}
