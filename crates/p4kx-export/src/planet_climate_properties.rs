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
/// Authored placement ranges, masks and climate curve settings.
#[non_exhaustive]
pub struct ClimateEnvelope {
    /// The authored lower elevation bound.
    pub min_elevation: Number,
    /// The authored upper elevation bound.
    pub max_elevation: Number,
    /// The authored elevation curve impact.
    pub elevation_curve_impact: Number,
    /// The authored high-elevation curve impact.
    pub elevation_high_curve_impact: Number,
    /// The authored lower annual rainfall bound.
    pub min_annual_rainfall: Number,
    /// The authored upper annual rainfall bound.
    pub max_annual_rainfall: Number,
    /// The authored humidity curve impact.
    pub humidity_curve_impact: Number,
    /// The authored lower temperature bound.
    pub min_temperature: Number,
    /// The authored upper temperature bound.
    pub max_temperature: Number,
    /// The authored temperature curve impact.
    pub temperature_curve_impact: Number,
    /// The authored optimal aggregate size.
    pub optimal_aggregate_size: Number,
    /// The authored aggregate size range.
    pub aggregate_size_range: Number,
    /// The authored bedrock curve impact.
    pub bedrock_curve_impact: Number,
    /// The authored optimal luminosity.
    pub optimal_luminosity: Number,
    /// The authored luminosity range.
    pub luminosity_range: Number,
    /// The authored luminosity curve impact.
    pub luminosity_curve_impact: Number,
    /// The authored minimum soil depth.
    pub soil_depth_min: Number,
    /// The authored maximum soil depth.
    pub soil_depth_max: Number,
    /// The authored soil depth curve.
    pub soil_depth_curve: Number,
    /// The authored biome mask.
    pub biome_mask: Number,
    /// The authored soil exclusion mask.
    pub soil_exclusion_mask: Number,
    /// The authored seed exclusion mask.
    pub seed_exclusion_mask: Number,
    /// The authored slope range in degrees.
    pub slope_range_deg: Vector,
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
/// Authored visual selectors and shading classes.
#[non_exhaustive]
pub struct ClimateAppearance {
    /// The authored default shading-class GUID.
    pub shading_class: GuidField,
    /// The authored close shading-class GUID.
    pub shading_class_close: GuidField,
    /// The authored mid-range shading-class GUID.
    pub shading_class_mid: GuidField,
    /// The authored far shading-class GUID.
    pub shading_class_far: GuidField,
    /// Candidate evidence for each authored shading-class selector.
    pub shading_links: Option<ClimateShadingLinks>,
    /// The authored debris path.
    pub debris_path: SourcedValue<String>,
    /// The authored far lookup table path.
    pub far_lut_path: SourcedValue<String>,
    /// The authored snow exclusion flag.
    pub exclude_from_snow: SourcedValue<bool>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
/// Original resolution evidence for each shading-class distance role.
#[non_exhaustive]
pub struct ClimateShadingLinks {
    /// The default shading-class link.
    pub default: TerrainGuidLink,
    /// The close shading-class link.
    pub close: TerrainGuidLink,
    /// The mid-range shading-class link.
    pub mid: TerrainGuidLink,
    /// The far shading-class link.
    pub far: TerrainGuidLink,
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
/// An original climate growth bucket.
#[non_exhaustive]
pub struct ClimateGrowthBucket {
    /// The authored shade density.
    pub shade_density: Number,
    /// Additional unprojected original fields.
    pub additional_fields: Vec<RawPlanetAssetMember>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
/// Authored growth, density and seasonal settings.
#[non_exhaustive]
pub struct ClimateGrowth {
    /// The authored dominance setting.
    pub dominance: Number,
    /// The authored sparsity setting.
    pub sparsity: Number,
    /// The authored vitality factor.
    pub vitality_factor: Number,
    /// The authored season offset range.
    pub season_offset_range: Number,
    /// The original growth buckets in source slot order.
    pub buckets: SourcedValue<Vec<SourcedValue<ClimateGrowthBucket>>>,
    /// The authored historic growth factors for size 0.
    pub historic_growth_factors_size0: Vector,
    /// The authored historic growth factors for size 1.
    pub historic_growth_factors_size1: Vector,
    /// The authored historic growth factors for size 2.
    pub historic_growth_factors_size2: Vector,
    /// The authored historic growth mode A.
    pub historic_growth_mode_a: SourcedValue<String>,
    /// The authored historic growth mode B.
    pub historic_growth_mode_b: SourcedValue<String>,
    /// The authored historic growth mode C.
    pub historic_growth_mode_c: SourcedValue<String>,
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
