// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceValue, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use crate::CatalogError;
use crate::catalog_read::catalog_json_number::CatalogJsonNumber;
use crate::catalog_read::entry_json_object::{self, EntryJsonObject};
use crate::catalog_read::parsed_entry_json::{SourceNode, SourceNodeValue};
use crate::guid_text::CatalogGuid;
use crate::schema_fields::every_field_required;

type Number = SourcedValue<CatalogJsonNumber>;
type Vector = SourcedValue<Vec<Number>>;

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
/// Authored geometry, material and Entity class selectors.
#[non_exhaustive]
pub struct GeologyGeometry {
    /// The authored geometry path.
    pub geometry_path: SourcedValue<String>,
    /// The authored material path.
    pub material_path: SourcedValue<String>,
    /// The authored geometry size vector.
    pub size: Vector,
    /// The authored level-of-detail ratio.
    pub lod_ratio: Number,
    /// The authored promotion type.
    pub promotion_type: SourcedValue<String>,
    /// The authored fallback promotion type.
    pub fallback_promotion_type: SourcedValue<String>,
    /// The authored Entity class GUID.
    pub entity_class: SourcedValue<CatalogGuid>,
    /// The original Entity class GUID text.
    pub entity_class_raw: SourcedValue<String>,
    /// The original named or numeric sun shadow mode.
    pub sun_shadow_mode: SourcedValue<SunShadowMode>,
    /// The authored wheeled collision mode.
    pub wheeled_collision_mode: SourcedValue<String>,
    /// The authored ground cover flag.
    pub ground_cover: SourcedValue<bool>,
}

impl GeologyGeometry {
    pub(crate) fn read(o: &EntryJsonObject<'_, '_>) -> Result<Self, CatalogError> {
        Ok(Self {
            geometry_path: o.field("geomPath", entry_json_object::text)?,
            material_path: o.field("matPath", entry_json_object::text)?,
            size: o.array("size", entry_json_object::number)?,
            lod_ratio: o.field("LODRatio", entry_json_object::number)?,
            promotion_type: o.field("ePromotionType", entry_json_object::text)?,
            fallback_promotion_type: o.field("eFallbackPromotionType", entry_json_object::text)?,
            entity_class: o.field("entityClass", entry_json_object::guid)?,
            entity_class_raw: o.field("entityClass", entry_json_object::text)?,
            sun_shadow_mode: o.field("SunShadowMode", sun_shadow_mode)?,
            wheeled_collision_mode: o.field("WheeledCollisionMode", entry_json_object::text)?,
            ground_cover: o.field("bGroundCover", entry_json_object::boolean)?,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(tag = "type", content = "value", rename_all = "camelCase")]
/// The original named or numeric representation of a sun shadow mode.
#[non_exhaustive]
pub enum SunShadowMode {
    /// An authored name.
    Named(String),
    /// An authored numeric token.
    Numeric(CatalogJsonNumber),
}

fn sun_shadow_mode(node: &SourceNode<'_>) -> SourceValue<SunShadowMode> {
    match &node.value {
        SourceNodeValue::String(value) => value.text().map_or(SourceValue::Invalid, |value| {
            SourceValue::Present(SunShadowMode::Named(value.to_owned()))
        }),
        SourceNodeValue::Number(token) => SourceValue::Present(SunShadowMode::Numeric(
            CatalogJsonNumber((*token).to_owned()),
        )),
        _ => SourceValue::Invalid,
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
/// Authored placements and alignment for a geology asset.
#[non_exhaustive]
pub struct GeologyPlacement {
    /// The authored projection alignment flag.
    pub align_to_projection_direction: SourcedValue<bool>,
    /// The authored gravity alignment flag.
    pub gravity_aligned: SourcedValue<bool>,
    /// The authored parent-blocking override.
    pub ignores_parent_blocking: SourcedValue<bool>,
    /// The authored burying immunity flag.
    pub immune_to_burying: SourcedValue<bool>,
    /// The lower authored random Z offset.
    pub random_z_offset_min: Number,
    /// The upper authored random Z offset.
    pub random_z_offset_max: Number,
    /// The authored slope alignment amount.
    pub slope_alignment_amount: Number,
    /// The authored slope range in degrees.
    pub slope_range_deg: Vector,
    /// The authored lower random rotation.
    pub random_rotation_min: Vector,
    /// The authored upper random rotation.
    pub random_rotation_max: Vector,
    /// The authored random rotation quantization.
    pub random_rotation_quantization: Vector,
    /// The authored random scale.
    pub random_scale: Vector,
}

impl GeologyPlacement {
    pub(crate) fn read(o: &EntryJsonObject<'_, '_>) -> Result<Self, CatalogError> {
        Ok(Self {
            align_to_projection_direction: o
                .field("bAlignToProjectionDirection", entry_json_object::boolean)?,
            gravity_aligned: o.field("bGravityAligned", entry_json_object::boolean)?,
            ignores_parent_blocking: o
                .field("bIgnoresParentBlocking", entry_json_object::boolean)?,
            immune_to_burying: o.field("bImmuneToBurying", entry_json_object::boolean)?,
            random_z_offset_min: o.field("fRandomZOffsetMin", entry_json_object::number)?,
            random_z_offset_max: o.field("fRandomZOffsetMax", entry_json_object::number)?,
            slope_alignment_amount: o.field("fSlopeAlignmentAmount", entry_json_object::number)?,
            slope_range_deg: o.array("vSlopeRangeDeg", entry_json_object::number)?,
            random_rotation_min: o.array("vRandomRotationMin", entry_json_object::number)?,
            random_rotation_max: o.array("vRandomRotationMax", entry_json_object::number)?,
            random_rotation_quantization: o
                .array("vRandomRotationQuantization", entry_json_object::number)?,
            random_scale: o.array("vRandomScale", entry_json_object::number)?,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
/// Authored aggregate, bedrock and soil settings for a geology asset.
#[non_exhaustive]
pub struct GeologySubstrate {
    /// The authored aggregate size.
    pub aggregate_size: Number,
    /// The authored maximum aggregate size.
    pub aggregate_size_max: Number,
    /// The authored bedrock fall distance.
    pub bedrock_falldown_distance: Number,
    /// The authored point of maximum bedrock influence.
    pub bedrock_point_of_max_influence: Number,
    /// The authored bedrock scale ratio.
    pub bedrock_scale_ratio: Number,
    /// The authored maximum soil depth.
    pub max_soil_depth: Number,
}

impl GeologySubstrate {
    pub(crate) fn read(o: &EntryJsonObject<'_, '_>) -> Result<Self, CatalogError> {
        Ok(Self {
            aggregate_size: o.field("fAggregateSize", entry_json_object::number)?,
            aggregate_size_max: o.field("fAggregateSizeMax", entry_json_object::number)?,
            bedrock_falldown_distance: o
                .field("fBedrockFalldownDistance", entry_json_object::number)?,
            bedrock_point_of_max_influence: o
                .field("fBedrockPointOfMaxInfluence", entry_json_object::number)?,
            bedrock_scale_ratio: o.field("fBedrockScaleRatio", entry_json_object::number)?,
            max_soil_depth: o.field("fMaxSoilDepth", entry_json_object::number)?,
        })
    }
}
