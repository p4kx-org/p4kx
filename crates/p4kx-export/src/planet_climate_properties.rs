// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use super::planet_asset_raw_fields::{self, RawPlanetAssetMember};
use super::terrain_guid_link::{TerrainGuidIndex, TerrainGuidLink};
use crate::CatalogError;
use crate::Kind;
use crate::catalog_read::catalog_json_number::CatalogJsonNumber;
use crate::catalog_read::entry_json_object::{self, EntryJsonObject};
use crate::catalog_read::original::EntryCapture;
use crate::guid_text::CatalogGuid;
use crate::schema_fields::every_field_required;

type Number = SourcedValue<CatalogJsonNumber>;
type Vector = SourcedValue<Vec<Number>>;
type GuidField = SourcedValue<CatalogGuid>;

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct ClimateEnvelope {
    min_elevation: Number,
    max_elevation: Number,
    elevation_curve_impact: Number,
    elevation_high_curve_impact: Number,
    min_annual_rainfall: Number,
    max_annual_rainfall: Number,
    humidity_curve_impact: Number,
    min_temperature: Number,
    max_temperature: Number,
    temperature_curve_impact: Number,
    optimal_aggregate_size: Number,
    aggregate_size_range: Number,
    bedrock_curve_impact: Number,
    optimal_luminosity: Number,
    luminosity_range: Number,
    luminosity_curve_impact: Number,
    soil_depth_min: Number,
    soil_depth_max: Number,
    soil_depth_curve: Number,
    biome_mask: Number,
    soil_exclusion_mask: Number,
    seed_exclusion_mask: Number,
    slope_range_deg: Vector,
}

impl ClimateEnvelope {
    pub(crate) fn read(o: &EntryJsonObject<'_, '_>) -> Result<Self, CatalogError> {
        Ok(Self {
            min_elevation: o.field("minElevation", entry_json_object::number)?,
            max_elevation: o.field("maxElevation", entry_json_object::number)?,
            elevation_curve_impact: o.field("elevationCurveImpact", entry_json_object::number)?,
            elevation_high_curve_impact: o
                .field("elevationHighCurveImpact", entry_json_object::number)?,
            min_annual_rainfall: o.field("minAnnualRainfall", entry_json_object::number)?,
            max_annual_rainfall: o.field("maxAnnualRainfall", entry_json_object::number)?,
            humidity_curve_impact: o.field("humidityCurveImpact", entry_json_object::number)?,
            min_temperature: o.field("minTemperature", entry_json_object::number)?,
            max_temperature: o.field("maxTemperature", entry_json_object::number)?,
            temperature_curve_impact: o
                .field("temperatureCurveImpact", entry_json_object::number)?,
            optimal_aggregate_size: o.field("optimalAggregateSize", entry_json_object::number)?,
            aggregate_size_range: o.field("aggregateSizeRange", entry_json_object::number)?,
            bedrock_curve_impact: o.field("bedrockCurveImpact", entry_json_object::number)?,
            optimal_luminosity: o.field("optimalLuminosity", entry_json_object::number)?,
            luminosity_range: o.field("luminosityRange", entry_json_object::number)?,
            luminosity_curve_impact: o.field("luminosityCurveImpact", entry_json_object::number)?,
            soil_depth_min: o.field("soilDepthMin", entry_json_object::number)?,
            soil_depth_max: o.field("soilDepthMax", entry_json_object::number)?,
            soil_depth_curve: o.field("soilDepthCurve", entry_json_object::number)?,
            biome_mask: o.field("biomeMask", entry_json_object::number)?,
            soil_exclusion_mask: o.field("soilExclusionMask", entry_json_object::number)?,
            seed_exclusion_mask: o.field("seedExclusionMask", entry_json_object::number)?,
            slope_range_deg: o.array("vSlopeRangeDeg", entry_json_object::number)?,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct ClimateAppearance {
    shading_class: GuidField,
    shading_class_close: GuidField,
    shading_class_mid: GuidField,
    shading_class_far: GuidField,
    shading_links: Option<ClimateShadingLinks>,
    debris_path: SourcedValue<String>,
    far_lut_path: SourcedValue<String>,
    exclude_from_snow: SourcedValue<bool>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct ClimateShadingLinks {
    pub(super) default: TerrainGuidLink,
    pub(super) close: TerrainGuidLink,
    pub(super) mid: TerrainGuidLink,
    pub(super) far: TerrainGuidLink,
}

impl ClimateAppearance {
    pub(super) fn shading_guids(&self) -> impl Iterator<Item = CatalogGuid> + '_ {
        [
            &self.shading_class,
            &self.shading_class_close,
            &self.shading_class_mid,
            &self.shading_class_far,
        ]
        .into_iter()
        .filter_map(SourcedValue::value)
        .copied()
        .filter(|guid| guid.0 != p4kx_extraction::Guid::default())
    }

    pub(super) fn attach_shading_links(
        &mut self,
        index: &TerrainGuidIndex,
        capture: &EntryCapture<'_>,
    ) -> Result<&ClimateShadingLinks, CatalogError> {
        Ok(self.shading_links.insert(ClimateShadingLinks {
            default: index.resolve(Kind::ShadingClasses, &self.shading_class, capture)?,
            close: index.resolve(Kind::ShadingClasses, &self.shading_class_close, capture)?,
            mid: index.resolve(Kind::ShadingClasses, &self.shading_class_mid, capture)?,
            far: index.resolve(Kind::ShadingClasses, &self.shading_class_far, capture)?,
        }))
    }
    pub(crate) fn read(o: &EntryJsonObject<'_, '_>) -> Result<Self, CatalogError> {
        Ok(Self {
            shading_class: o.field("shadingClassGUID", entry_json_object::guid)?,
            shading_class_close: o.field("shadingClassCloseGUID", entry_json_object::guid)?,
            shading_class_mid: o.field("shadingClassMidGUID", entry_json_object::guid)?,
            shading_class_far: o.field("shadingClassFarGUID", entry_json_object::guid)?,
            shading_links: None,
            debris_path: o.field("debrisPath", entry_json_object::text)?,
            far_lut_path: o.field("farLUTPath", entry_json_object::text)?,
            exclude_from_snow: o.field("bExcludeFromSnow", entry_json_object::boolean)?,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct ClimateGrowthBucket {
    shade_density: Number,
    additional_fields: Vec<RawPlanetAssetMember>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct ClimateGrowth {
    dominance: Number,
    sparsity: Number,
    vitality_factor: Number,
    season_offset_range: Number,
    buckets: SourcedValue<Vec<SourcedValue<ClimateGrowthBucket>>>,
    historic_growth_factors_size0: Vector,
    historic_growth_factors_size1: Vector,
    historic_growth_factors_size2: Vector,
    historic_growth_mode_a: SourcedValue<String>,
    historic_growth_mode_b: SourcedValue<String>,
    historic_growth_mode_c: SourcedValue<String>,
}

impl ClimateGrowth {
    pub(crate) fn read(o: &EntryJsonObject<'_, '_>) -> Result<Self, CatalogError> {
        Ok(Self {
            dominance: o.field("dominance", entry_json_object::number)?,
            sparsity: o.field("sparsity", entry_json_object::number)?,
            vitality_factor: o.field("fVitalityFactor", entry_json_object::number)?,
            season_offset_range: o.field("fSeasonOffsetRange", entry_json_object::number)?,
            buckets: o.object_array("growthStageBuckets", |bucket| {
                Ok(ClimateGrowthBucket {
                    shade_density: bucket.field("shadeDensity", entry_json_object::number)?,
                    additional_fields: planet_asset_raw_fields::collect(bucket)?,
                })
            })?,
            historic_growth_factors_size0: o
                .array("historicGrowthFactorsSize0", entry_json_object::number)?,
            historic_growth_factors_size1: o
                .array("historicGrowthFactorsSize1", entry_json_object::number)?,
            historic_growth_factors_size2: o
                .array("historicGrowthFactorsSize2", entry_json_object::number)?,
            historic_growth_mode_a: o.field("historicGrowthModeA", entry_json_object::text)?,
            historic_growth_mode_b: o.field("historicGrowthModeB", entry_json_object::text)?,
            historic_growth_mode_c: o.field("historicGrowthModeC", entry_json_object::text)?,
        })
    }
}
