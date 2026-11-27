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
const BIPLANAR_FIELDS: terrain_layers::TextureLayerFieldNames =
    terrain_layers::TextureLayerFieldNames {
        array: "biplanarLayers",
        texture: "biplanarLayers[].textureName",
    };

const GROUND_FIELDS: GroundLayerFieldNames = GroundLayerFieldNames {
    texture: "groundLayers[].textureName",
    surface_type: "groundLayers[].surfaceTypeName",
    surface_group: "groundLayers[].surfaceGroupName",
    gloss: "groundLayers[].gloss",
    bedrock: "groundLayers[].bIsBedrock",
};
pub(super) const FAMILY: DefinitionFamily = DefinitionFamily::new(
    Kind::GeologyTextureSets,
    "data/libs/planets/geology_sets/",
    "geotextureset",
    "GeologyTextureSet",
    "GUID",
);

pub(crate) type GeologyTextureSetTable = CatalogTable<GeologyTextureSet>;

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
/// Original material layers, texture selectors and geology surface parameters.
#[non_exhaustive]
pub struct GeologyTextureSet {
    /// The physical Entry that owns this definition.
    pub identity: PhysicalEntryIdentity,
    /// The parsed owner GUID.
    pub guid: SourcedValue<CatalogGuid>,
    /// The original owner GUID text.
    pub guid_raw: SourcedValue<String>,
    /// The authored ground material layers in source slot order.
    pub ground_layers: GroundLayers,
    /// The authored biplanar layers in source slot order.
    pub biplanar_layers: TextureLayers,
    /// Directory lookup receipts for the ground textures.
    pub ground_textures: Option<LayerTextureLinks>,
    /// Directory lookup receipts for the biplanar textures.
    pub biplanar_textures: Option<LayerTextureLinks>,
    /// The authored scree content by layer slot.
    pub scree_content_per_layer: NumericSlots,
    /// The authored minimum roughness.
    pub min_roughness: Number,
    /// The authored maximum roughness.
    pub max_roughness: Number,
    /// The authored aggregate bleed.
    pub aggregate_bleed: Number,
    /// The authored aggregate substrate spread.
    pub aggregate_substrate_spread: Number,
    /// The authored minimum scree roughness.
    pub scree_min_roughness: Number,
    /// The authored maximum scree roughness.
    pub scree_max_roughness: Number,
    /// The authored scree spread.
    pub scree_spread: Number,
    /// The authored gradient midpoint.
    pub gradient_mid_point: Number,
    /// The authored gradient noise range.
    pub gradient_noise_range: Number,
    /// Additional unprojected original fields.
    pub additional_fields: Vec<RawPlanetAssetMember>,
    /// Source issues observed in this owner selection.
    pub issues: Vec<DefinitionIssue>,
}

impl LooseDefinition for GeologyTextureSet {
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
) -> Result<GeologyTextureSetTable, CatalogError> {
    read_selected(archive, capture, ledger, options, paths, Selection::All)
}

pub(crate) fn read_selected(
    archive: &Archive,
    capture: &EntryCapture<'_>,
    ledger: &mut ExportLedger,
    options: &CatalogOptions<'_>,
    paths: &mut loose_archive_path::Resolver<'_, '_, '_>,
    selection: Selection<'_>,
) -> Result<GeologyTextureSetTable, CatalogError> {
    let mut resolver = LayerResolver::new(paths, options.cancel);
    loose_definition::read_selected(
        archive,
        capture,
        ledger,
        options,
        FAMILY,
        selection,
        |identity, object| GeologyTextureSet::project(identity, object, &mut resolver),
    )
}

impl GeologyTextureSet {
    fn project(
        identity: PhysicalEntryIdentity,
        object: &EntryJsonObject<'_, '_>,
        resolver: &mut LayerResolver<'_, '_, '_, '_>,
    ) -> Result<Self, CatalogError> {
        let object = &object.retaining_unprojected();
        let guid = object.field("GUID", entry_json_object::guid)?;
        let guid_raw = object.field("GUID", entry_json_object::text)?;
        let ground_layers = object.object_array("groundLayers", GroundLayer::read)?;
        let biplanar_layers = object.object_array("biplanarLayers", TextureLayer::read)?;
        let scree_content_per_layer =
            object.array("fScreeContentPerLayer", entry_json_object::number)?;
        let mut issues: Vec<_> = loose_definition::identity_issue("GUID", &guid)
            .into_iter()
            .collect();
        let measurements = Measurements::read(object, &mut issues)?;
        let layer_paths = resolver.layers(
            &ground_layers,
            &biplanar_layers,
            GROUND_FIELDS,
            BIPLANAR_FIELDS,
            &mut issues,
        )?;
        terrain_layers::note_invalid_array(
            &mut issues,
            "fScreeContentPerLayer",
            &scree_content_per_layer,
        );
        Ok(Self {
            identity,
            guid,
            guid_raw,
            ground_layers,
            biplanar_layers,
            ground_textures: layer_paths.ground,
            biplanar_textures: layer_paths.additional,
            scree_content_per_layer,
            min_roughness: measurements.min_roughness,
            max_roughness: measurements.max_roughness,
            aggregate_bleed: measurements.aggregate_bleed,
            aggregate_substrate_spread: measurements.aggregate_substrate_spread,
            scree_min_roughness: measurements.scree_min_roughness,
            scree_max_roughness: measurements.scree_max_roughness,
            scree_spread: measurements.scree_spread,
            gradient_mid_point: measurements.gradient_mid_point,
            gradient_noise_range: measurements.gradient_noise_range,
            additional_fields: planet_asset_raw_fields::collect(object)?,
            issues,
        })
    }
}

struct Measurements {
    min_roughness: Number,
    max_roughness: Number,
    aggregate_bleed: Number,
    aggregate_substrate_spread: Number,
    scree_min_roughness: Number,
    scree_max_roughness: Number,
    scree_spread: Number,
    gradient_mid_point: Number,
    gradient_noise_range: Number,
}

impl Measurements {
    fn read(
        object: &EntryJsonObject<'_, '_>,
        issues: &mut Vec<DefinitionIssue>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            min_roughness: terrain_layers::read_field(
                object,
                "fMinRoughness",
                entry_json_object::number,
                issues,
            )?,
            max_roughness: terrain_layers::read_field(
                object,
                "fMaxRoughness",
                entry_json_object::number,
                issues,
            )?,
            aggregate_bleed: terrain_layers::read_field(
                object,
                "fGeologyAggregateBleed",
                entry_json_object::number,
                issues,
            )?,
            aggregate_substrate_spread: terrain_layers::read_field(
                object,
                "fGeologyAggregateSubstrateSpread",
                entry_json_object::number,
                issues,
            )?,
            scree_min_roughness: terrain_layers::read_field(
                object,
                "fScreeMinRoughness",
                entry_json_object::number,
                issues,
            )?,
            scree_max_roughness: terrain_layers::read_field(
                object,
                "fScreeMaxRoughness",
                entry_json_object::number,
                issues,
            )?,
            scree_spread: terrain_layers::read_field(
                object,
                "fScreeSpread",
                entry_json_object::number,
                issues,
            )?,
            gradient_mid_point: terrain_layers::read_field(
                object,
                "fGradientMidPoint",
                entry_json_object::number,
                issues,
            )?,
            gradient_noise_range: terrain_layers::read_field(
                object,
                "fGradientNoiseRange",
                entry_json_object::number,
                issues,
            )?,
        })
    }
}
