// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::planet_asset_raw_fields::{self, RawPlanetAssetMember};
use super::terrain_layers::{
    self, GroundLayer, GroundLayerFieldNames, GroundLayers, LayerResolver, LayerTextureLinks,
    Number, NumericSlots, TextureLayer, TextureLayers,
};
use crate::catalog_read::entry_json_object::{self, EntryJsonObject};
use crate::catalog_read::loose_archive_path;
use crate::catalog_read::loose_definition::{
    self, DefinitionFamily, DefinitionIssue, LooseDefinition, Selection,
};
use crate::catalog_read::original::{EntryCapture, PhysicalEntryIdentity};
use crate::catalog_table::CatalogTable;
use crate::export_ledger::ExportLedger;
use crate::guid_text::CatalogGuid;
use crate::schema_fields::every_field_required;
use crate::{CatalogError, CatalogOptions, Kind};
const MEDIUM_FIELDS: terrain_layers::TextureLayerFieldNames =
    terrain_layers::TextureLayerFieldNames {
        array: "mediumLayers",
        texture: "mediumLayers[].textureName",
    };

const GROUND_FIELDS: GroundLayerFieldNames = GroundLayerFieldNames {
    texture: "groundLayers[].textureName",
    surface_type: "groundLayers[].surfaceTypeName",
    surface_group: "groundLayers[].surfaceGroupName",
    gloss: "groundLayers[].gloss",
    bedrock: "groundLayers[].bIsBedrock",
};
pub(super) const FAMILY: DefinitionFamily = DefinitionFamily::new(
    Kind::SoilTextureSets,
    "data/libs/planets/soil_sets/",
    "soiltextureset",
    "SoilTextureSet",
    "GUID",
);

pub(crate) type SoilTextureSetTable = CatalogTable<SoilTextureSet>;

/// Original soil texture layers and terrain material settings.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct SoilTextureSet {
    /// The original physical Entry identity.
    pub identity: PhysicalEntryIdentity,
    /// The parsed original GUID.
    pub guid: SourcedValue<CatalogGuid>,
    /// The original GUID text.
    pub guid_raw: SourcedValue<String>,
    /// The original ground layer slots.
    pub ground_layers: GroundLayers,
    /// The original medium layer slots.
    pub medium_layers: TextureLayers,
    /// Directory links for authored ground texture paths.
    pub ground_textures: Option<LayerTextureLinks>,
    /// Directory links for authored medium texture paths.
    pub medium_textures: Option<LayerTextureLinks>,
    /// The original medium layer weights.
    pub medium_layer_weights: NumericSlots,
    /// The original medium layer height scales.
    pub medium_layer_height_scales: NumericSlots,
    /// The original scree content slots.
    pub scree_content_per_layer: NumericSlots,
    /// The authored sand flag.
    pub sand: SourcedValue<bool>,
    /// The authored minimum roughness.
    pub min_roughness: Number,
    /// The authored maximum roughness.
    pub max_roughness: Number,
    /// Additional unprojected original fields.
    pub additional_fields: Vec<RawPlanetAssetMember>,
    /// Source issues observed in this owner selection.
    pub issues: Vec<DefinitionIssue>,
}

impl LooseDefinition for SoilTextureSet {
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
) -> Result<SoilTextureSetTable, CatalogError> {
    read_selected(archive, capture, ledger, options, paths, Selection::All)
}

pub(crate) fn read_selected(
    archive: &Archive,
    capture: &EntryCapture<'_>,
    ledger: &mut ExportLedger,
    options: &CatalogOptions<'_>,
    paths: &mut loose_archive_path::Resolver<'_, '_, '_>,
    selection: Selection<'_>,
) -> Result<SoilTextureSetTable, CatalogError> {
    let mut resolver = LayerResolver::new(paths, options.cancel);
    loose_definition::read_selected(
        archive,
        capture,
        ledger,
        options,
        FAMILY,
        selection,
        |identity, object| SoilTextureSet::project(identity, object, &mut resolver),
    )
}

impl SoilTextureSet {
    fn project(
        identity: PhysicalEntryIdentity,
        object: &EntryJsonObject<'_, '_>,
        resolver: &mut LayerResolver<'_, '_, '_, '_>,
    ) -> Result<Self, CatalogError> {
        let object = &object.retaining_unprojected();
        let guid = object.field("GUID", entry_json_object::guid)?;
        let guid_raw = object.field("GUID", entry_json_object::text)?;
        let ground_layers = object.object_array("groundLayers", GroundLayer::read)?;
        let medium_layers = object.object_array("mediumLayers", TextureLayer::read)?;
        let mut issues: Vec<_> = loose_definition::identity_issue("GUID", &guid)
            .into_iter()
            .collect();
        let sand =
            terrain_layers::read_field(object, "bSand", entry_json_object::boolean, &mut issues)?;
        let min_roughness = terrain_layers::read_field(
            object,
            "fMinRoughness",
            entry_json_object::number,
            &mut issues,
        )?;
        let max_roughness = terrain_layers::read_field(
            object,
            "fMaxRoughness",
            entry_json_object::number,
            &mut issues,
        )?;
        let parameters = LayerParameters::read(object, &mut issues)?;
        let layer_paths = resolver.layers(
            &ground_layers,
            &medium_layers,
            GROUND_FIELDS,
            MEDIUM_FIELDS,
            &mut issues,
        )?;
        Ok(Self {
            identity,
            guid,
            guid_raw,
            ground_layers,
            medium_layers,
            ground_textures: layer_paths.ground,
            medium_textures: layer_paths.additional,
            medium_layer_weights: parameters.medium_layer_weights,
            medium_layer_height_scales: parameters.medium_layer_height_scales,
            scree_content_per_layer: parameters.scree_content_per_layer,
            sand,
            min_roughness,
            max_roughness,
            additional_fields: planet_asset_raw_fields::collect(object)?,
            issues,
        })
    }
}

struct LayerParameters {
    medium_layer_weights: NumericSlots,
    medium_layer_height_scales: NumericSlots,
    scree_content_per_layer: NumericSlots,
}

impl LayerParameters {
    fn read(
        object: &EntryJsonObject<'_, '_>,
        issues: &mut Vec<DefinitionIssue>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            medium_layer_weights: read_numeric_slots(object, "mediumLayerParameters", issues)?,
            medium_layer_height_scales: read_numeric_slots(
                object,
                "mediumLayerHeightScales",
                issues,
            )?,
            scree_content_per_layer: read_numeric_slots(object, "fScreeContentPerLayer", issues)?,
        })
    }
}

fn read_numeric_slots(
    object: &EntryJsonObject<'_, '_>,
    field: &'static str,
    issues: &mut Vec<DefinitionIssue>,
) -> Result<NumericSlots, CatalogError> {
    let values = object.array(field, entry_json_object::number)?;
    terrain_layers::note_invalid_array(issues, field, &values);
    Ok(values)
}
