// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataForge, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::planet_asset_raw_fields::{self, RawPlanetAssetMember};
use super::terrain_guid_link::{self, TerrainGuidIndex, TerrainGuidLink};
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_read::catalog_json_number::CatalogJsonNumber;
use crate::catalog_read::entry_json_object::{self, EntryJsonObject};
use crate::catalog_read::loose_definition::{self, DefinitionIssue};
use crate::catalog_read::loose_record_link::{self, LooseRecordLink};
use crate::catalog_read::original::EntryCapture;
use crate::catalog_read::parsed_entry_json;
use crate::guid_text::CatalogGuid;
use crate::schema_fields::every_field_required;
use crate::{CatalogError, CatalogOptions, Kind};

type Number = SourcedValue<CatalogJsonNumber>;
type Vector = SourcedValue<Vec<Number>>;

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct ClimateSubAsset {
    asset_size: Number,
    asset_size_y: Number,
    gravity_aligned: SourcedValue<bool>,
    ground_cover: SourcedValue<bool>,
    biome_tag: Number,
    spawn_ocean_surface: SourcedValue<bool>,
    use_ground_cover_coverage: SourcedValue<bool>,
    fallback_promotion_type: SourcedValue<String>,
    entity_class: SourcedValue<CatalogGuid>,
    entity_class_raw: SourcedValue<String>,
    entity: LooseRecordLink,
    promotion_type: SourcedValue<String>,
    random_z_offset_max: Number,
    random_z_offset_min: Number,
    slope_alignment_amount: Number,
    geometry_path: SourcedValue<String>,
    harvestable_resource: SourcedValue<CatalogGuid>,
    harvestable_resource_raw: SourcedValue<String>,
    harvestable_resource_link: Option<TerrainGuidLink>,
    harvestable_setup_record: SourcedValue<CatalogGuid>,
    harvestable_setup_record_raw: SourcedValue<String>,
    harvestable_setup: LooseRecordLink,
    lod_ratio: Number,
    material_path: SourcedValue<String>,
    growth_stage_bucket: Vector,
    opacity: Number,
    sun_shadow_mode: SourcedValue<String>,
    variant_proportion: Number,
    random_rotation_max: Vector,
    random_rotation_min: Vector,
    random_rotation_quantization: Vector,
    random_scale: Vector,
    width_from_base_to_top_multiplier: Vector,
    wheeled_collision_mode: SourcedValue<String>,
    additional_fields: Vec<RawPlanetAssetMember>,
    issues: Vec<DefinitionIssue>,
}

struct RecordLinks {
    entity_class: SourcedValue<CatalogGuid>,
    entity_class_raw: SourcedValue<String>,
    entity: LooseRecordLink,
    harvestable_setup_record: SourcedValue<CatalogGuid>,
    harvestable_setup_record_raw: SourcedValue<String>,
    harvestable_setup: LooseRecordLink,
    issues: Vec<DefinitionIssue>,
}

impl RecordLinks {
    fn read(
        o: &EntryJsonObject<'_, '_>,
        forge: &DataForge<'_>,
        original: &OriginalCatalogInput<'_, '_>,
        options: &CatalogOptions<'_>,
    ) -> Result<Self, CatalogError> {
        let entity_class = o.field("entityClass", entry_json_object::guid)?;
        let entity_class_raw = o.field("entityClass", entry_json_object::text)?;
        let harvestable_setup_record =
            o.field("harvestableSetupRecord", entry_json_object::guid)?;
        let harvestable_setup_record_raw =
            o.field("harvestableSetupRecord", entry_json_object::text)?;
        let entity = loose_record_link::by_guid(
            forge,
            original,
            options,
            &entity_class,
            &entity_class_raw,
            "EntityClassDefinition",
        )?;
        let harvestable_setup = loose_record_link::by_guid(
            forge,
            original,
            options,
            &harvestable_setup_record,
            &harvestable_setup_record_raw,
            "HarvestableSetup",
        )?;
        let mut issues = Vec::new();
        for (field, state) in [
            ("entityClass", entity.state.value()),
            ("harvestableSetupRecord", harvestable_setup.state.value()),
        ] {
            if let Some(code) = state.copied().and_then(loose_definition::reference_issue) {
                parsed_entry_json::push(&mut issues, DefinitionIssue { field, code })?;
            }
        }
        Ok(Self {
            entity_class,
            entity_class_raw,
            entity,
            harvestable_setup_record,
            harvestable_setup_record_raw,
            harvestable_setup,
            issues,
        })
    }
}

impl ClimateSubAsset {
    pub(super) const fn resource_guid(&self) -> &SourcedValue<CatalogGuid> {
        &self.harvestable_resource
    }

    pub(super) fn link_resource(
        &mut self,
        resources: &TerrainGuidIndex,
        capture: &EntryCapture<'_>,
        ledger: &mut crate::export_ledger::ExportLedger,
    ) -> Result<(), CatalogError> {
        let link = resources.resolve(Kind::PlanetResources, &self.harvestable_resource, capture)?;
        if let Some(code) = link
            .state
            .value()
            .copied()
            .and_then(terrain_guid_link::link_issue)
        {
            let issue = DefinitionIssue {
                field: "harvestableResource",
                code,
            };
            parsed_entry_json::push(&mut self.issues, issue)?;
        }
        for issue in &self.issues {
            issue.record(ledger, Kind::ClimateAssets);
        }
        self.harvestable_resource_link = Some(link);
        Ok(())
    }

    pub(crate) fn read(
        o: &EntryJsonObject<'_, '_>,
        forge: &DataForge<'_>,
        original: &OriginalCatalogInput<'_, '_>,
        options: &CatalogOptions<'_>,
    ) -> Result<Self, CatalogError> {
        let links = RecordLinks::read(o, forge, original, options)?;
        Ok(Self {
            asset_size: o.field("assetSize", entry_json_object::number)?,
            asset_size_y: o.field("assetSizeY", entry_json_object::number)?,
            gravity_aligned: o.field("bGravityAligned", entry_json_object::boolean)?,
            ground_cover: o.field("bGroundCover", entry_json_object::boolean)?,
            biome_tag: o.field("biomeTag", entry_json_object::number)?,
            spawn_ocean_surface: o.field("bSpawnOceanSurface", entry_json_object::boolean)?,
            use_ground_cover_coverage: o
                .field("bUseGroundCoverCoverage", entry_json_object::boolean)?,
            fallback_promotion_type: o.field("eFallbackPromotionType", entry_json_object::text)?,
            entity_class: links.entity_class,
            entity_class_raw: links.entity_class_raw,
            entity: links.entity,
            promotion_type: o.field("ePromotionType", entry_json_object::text)?,
            random_z_offset_max: o.field("fRandomZOffsetMax", entry_json_object::number)?,
            random_z_offset_min: o.field("fRandomZOffsetMin", entry_json_object::number)?,
            slope_alignment_amount: o.field("fSlopeAlignmentAmount", entry_json_object::number)?,
            geometry_path: o.field("geomPath", entry_json_object::text)?,
            harvestable_resource: o.field("harvestableResource", entry_json_object::guid)?,
            harvestable_resource_raw: o.field("harvestableResource", entry_json_object::text)?,
            harvestable_resource_link: None,
            harvestable_setup_record: links.harvestable_setup_record,
            harvestable_setup_record_raw: links.harvestable_setup_record_raw,
            harvestable_setup: links.harvestable_setup,
            lod_ratio: o.field("LODRatio", entry_json_object::number)?,
            material_path: o.field("matPath", entry_json_object::text)?,
            growth_stage_bucket: o.array("nGrowthStageBucket", entry_json_object::number)?,
            opacity: o.field("opacity", entry_json_object::number)?,
            sun_shadow_mode: o.field("SunShadowMode", entry_json_object::text)?,
            variant_proportion: o.field("VariantProportion", entry_json_object::number)?,
            random_rotation_max: o.array("vRandomRotationMax", entry_json_object::number)?,
            random_rotation_min: o.array("vRandomRotationMin", entry_json_object::number)?,
            random_rotation_quantization: o
                .array("vRandomRotationQuantization", entry_json_object::number)?,
            random_scale: o.array("vRandomScale", entry_json_object::number)?,
            width_from_base_to_top_multiplier: o
                .array("vWidthFromBaseToTopMultiplier", entry_json_object::number)?,
            wheeled_collision_mode: o.field("WheeledCollisionMode", entry_json_object::text)?,
            additional_fields: planet_asset_raw_fields::collect(o)?,
            issues: links.issues,
        })
    }
}
