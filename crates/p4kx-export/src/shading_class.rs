// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::planet_asset_raw_fields::{self, RawPlanetAssetMember};
use super::terrain_layers::{self, GroundLayer, GroundLayerFieldNames, LayerResolver, Number};
use crate::catalog_read::entry_json_object::{self, EntryJsonObject};
use crate::catalog_read::loose_archive_path::{self, LooseArchivePath};
use crate::catalog_read::loose_definition::{
    self, DefinitionFamily, DefinitionIssue, LooseDefinition,
};
use crate::catalog_read::original::{EntryCapture, PhysicalEntryIdentity};
use crate::catalog_table::CatalogTable;
use crate::export_ledger::ExportLedger;
use crate::guid_text::CatalogGuid;
use crate::schema_fields::every_field_required;
use crate::{CatalogError, CatalogOptions, Kind};

const GROUND_FIELDS: GroundLayerFieldNames = GroundLayerFieldNames {
    texture: "groundLayer.textureName",
    surface_type: "groundLayer.surfaceTypeName",
    surface_group: "groundLayer.surfaceGroupName",
    gloss: "groundLayer.gloss",
    bedrock: "groundLayer.bIsBedrock",
};
pub(super) const FAMILY: DefinitionFamily = DefinitionFamily::new(
    Kind::ShadingClasses,
    "data/libs/planets/shading_classes/",
    "shadingclass",
    "ShadingClass",
    "GUID",
);

pub(crate) type ShadingClassTable = CatalogTable<ShadingClass>;

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct ShadingClass {
    identity: PhysicalEntryIdentity,
    guid: SourcedValue<CatalogGuid>,
    guid_raw: SourcedValue<String>,
    ground_layer: SourcedValue<GroundLayer>,
    ground_texture: Option<LooseArchivePath>,
    lighting_model: Number,
    roughness: Number,
    ambient_occlusion_amount: Number,
    distant_vegetation_uv_scale: Number,
    view_angle_masking: Number,
    normal_strength: Number,
    parallax_occlusion_height: Number,
    additional_fields: Vec<RawPlanetAssetMember>,
    issues: Vec<DefinitionIssue>,
}

impl LooseDefinition for ShadingClass {
    fn guid(&self) -> &SourcedValue<CatalogGuid> {
        &self.guid
    }

    fn issues(&mut self) -> &mut Vec<DefinitionIssue> {
        &mut self.issues
    }
}

pub(crate) fn read(
    archive: &Archive,
    capture: &EntryCapture<'_>,
    ledger: &mut ExportLedger,
    options: &CatalogOptions<'_>,
    paths: &mut loose_archive_path::Resolver<'_, '_, '_>,
) -> Result<ShadingClassTable, CatalogError> {
    let mut resolver = LayerResolver::new(paths, options.cancel);
    loose_definition::read(
        archive,
        capture,
        ledger,
        options,
        FAMILY,
        |identity, object| ShadingClass::project(identity, object, &mut resolver),
    )
}

impl ShadingClass {
    fn project(
        identity: PhysicalEntryIdentity,
        object: &EntryJsonObject<'_, '_>,
        resolver: &mut LayerResolver<'_, '_, '_, '_>,
    ) -> Result<Self, CatalogError> {
        let object = &object.retaining_unprojected();
        let guid = object.field("GUID", entry_json_object::guid)?;
        let guid_raw = object.field("GUID", entry_json_object::text)?;
        let ground_layer = object.object("groundLayer", GroundLayer::read)?;
        let lighting_model = object.field("lightingModel", terrain_layers::integer)?;
        let roughness = object.field("fRoughness", entry_json_object::number)?;
        let ambient_occlusion_amount = object.field("fAOAmount", entry_json_object::number)?;
        let distant_vegetation_uv_scale =
            object.field("fDistantVegUVScale", entry_json_object::number)?;
        let view_angle_masking = object.field("fViewAngleMasking", entry_json_object::number)?;
        let normal_strength = object.field("fNormalStrength", entry_json_object::number)?;
        let parallax_occlusion_height = object.field("fPomHeight", entry_json_object::number)?;
        let mut issues: Vec<_> = loose_definition::identity_issue("GUID", &guid)
            .into_iter()
            .collect();
        terrain_layers::note_invalid(
            &mut issues,
            [
                ("groundLayer", ground_layer.state()),
                ("lightingModel", lighting_model.state()),
                ("fRoughness", roughness.state()),
                ("fAOAmount", ambient_occlusion_amount.state()),
                ("fDistantVegUVScale", distant_vegetation_uv_scale.state()),
                ("fViewAngleMasking", view_angle_masking.state()),
                ("fNormalStrength", normal_strength.state()),
                ("fPomHeight", parallax_occlusion_height.state()),
            ],
        );
        let ground_texture = if let Some(ground) = ground_layer.value() {
            ground.field_issues(GROUND_FIELDS, &mut issues);
            Some(resolver.resolve(ground.texture(), GROUND_FIELDS.texture, &mut issues)?)
        } else {
            None
        };
        Ok(Self {
            identity,
            guid,
            guid_raw,
            ground_layer,
            ground_texture,
            lighting_model,
            roughness,
            ambient_occlusion_amount,
            distant_vegetation_uv_scale,
            view_angle_masking,
            normal_strength,
            parallax_occlusion_height,
            additional_fields: planet_asset_raw_fields::collect(object)?,
            issues,
        })
    }
}
