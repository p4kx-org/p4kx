// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::planet_asset_raw_fields;
pub use super::planet_asset_raw_fields::RawPlanetAssetMember;
use super::terrain_guid_link::{self, TerrainGuidIndex};
pub use super::terrain_guid_link::{TerrainGuidCandidate, TerrainGuidLink, TerrainGuidLinkState};
use super::terrain_layers;
pub use super::terrain_layers::{GroundLayer, LayerTextureLink, LayerTextureLinks, TextureLayer};
use crate::catalog_read::entry_json_object::{self, EntryJsonObject};
use crate::catalog_read::loose_definition::{
    self, DefinitionFamily, DefinitionIssue, LooseDefinition, Selection,
};
use crate::catalog_read::original::{EntryCapture, PhysicalEntryIdentity};
use crate::catalog_read::parsed_entry_json;
use crate::catalog_table::CatalogTable;
use crate::export_ledger::ExportLedger;
use crate::guid_text::CatalogGuid;
use crate::schema_fields::every_field_required;
use crate::{CatalogError, CatalogOptions, Kind};

const KIND: Kind = Kind::Soils;
pub(super) const FAMILY: DefinitionFamily =
    DefinitionFamily::new(KIND, "data/libs/planets/soil_sets/", "soil", "Soil", "GUID");

pub(crate) type SoilTable = CatalogTable<Soil>;

/// Authored soil flags, weight and texture-set declaration.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct Soil {
    /// The original physical Entry identity.
    pub identity: PhysicalEntryIdentity,
    /// The parsed original GUID.
    pub guid: SourcedValue<CatalogGuid>,
    /// The original GUID text.
    pub guid_raw: SourcedValue<String>,
    #[serde(rename = "soilTextureSetGuid")]
    /// The authored soil texture-set GUID.
    pub texture_set_guid: SourcedValue<CatalogGuid>,
    #[serde(rename = "soilTextureSetGuidRaw")]
    /// The authored soil texture-set GUID text.
    pub texture_set_guid_raw: SourcedValue<String>,
    #[serde(rename = "soilTextureSet")]
    /// The original texture-set candidates and link outcome.
    pub texture_set: Option<TerrainGuidLink>,
    /// The authored beach flag.
    pub beach: SourcedValue<bool>,
    /// The authored stagnant-water flag.
    pub stagnant_water: SourcedValue<bool>,
    /// The authored trash flag.
    pub trash: SourcedValue<bool>,
    /// The authored acid flag.
    pub acid: SourcedValue<bool>,
    /// The authored ocean secondary color weight.
    pub ocean_secondary_colour_weight: terrain_layers::Number,
    /// Additional unprojected original fields.
    pub additional_fields: Vec<RawPlanetAssetMember>,
    /// Source issues observed in this owner selection.
    pub issues: Vec<DefinitionIssue>,
}

impl LooseDefinition for Soil {
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
) -> Result<SoilTable, CatalogError> {
    read_selected(archive, capture, ledger, options, Selection::All)
}

pub(crate) fn read_selected(
    archive: &Archive,
    capture: &EntryCapture<'_>,
    ledger: &mut ExportLedger,
    options: &CatalogOptions<'_>,
    selection: Selection<'_>,
) -> Result<SoilTable, CatalogError> {
    loose_definition::read_selected(
        archive,
        capture,
        ledger,
        options,
        FAMILY,
        selection,
        Soil::project,
    )
}

impl Soil {
    fn project(
        identity: PhysicalEntryIdentity,
        object: &EntryJsonObject<'_, '_>,
    ) -> Result<Self, CatalogError> {
        let object = &object.retaining_unprojected();
        let guid = object.field("GUID", entry_json_object::guid)?;
        let guid_raw = object.field("GUID", entry_json_object::text)?;
        let texture_set_guid = object.field("soilTextureSetGUID", entry_json_object::guid)?;
        let texture_set_guid_raw = object.field("soilTextureSetGUID", entry_json_object::text)?;
        let beach = object.field("bBeach", entry_json_object::boolean)?;
        let stagnant_water = object.field("bStagnantWater", entry_json_object::boolean)?;
        let trash = object.field("bTrash", entry_json_object::boolean)?;
        let acid = object.field("bAcid", entry_json_object::boolean)?;
        let ocean_secondary_colour_weight =
            object.field("oceanSecondaryColourWeight", entry_json_object::number)?;
        let mut issues: Vec<_> = loose_definition::identity_issue("GUID", &guid)
            .into_iter()
            .collect();
        terrain_layers::note_invalid(
            &mut issues,
            [
                ("soilTextureSetGUID", texture_set_guid.state()),
                ("bBeach", beach.state()),
                ("bStagnantWater", stagnant_water.state()),
                ("bTrash", trash.state()),
                ("bAcid", acid.state()),
                (
                    "oceanSecondaryColourWeight",
                    ocean_secondary_colour_weight.state(),
                ),
            ],
        );
        Ok(Self {
            identity,
            guid,
            guid_raw,
            texture_set_guid,
            texture_set_guid_raw,
            texture_set: None,
            beach,
            stagnant_water,
            trash,
            acid,
            ocean_secondary_colour_weight,
            additional_fields: planet_asset_raw_fields::collect(object)?,
            issues,
        })
    }

    pub(crate) fn required_guids(&self) -> impl Iterator<Item = CatalogGuid> + '_ {
        self.texture_set_guid
            .value()
            .copied()
            .into_iter()
            .filter(|guid| guid.0 != p4kx_extraction::Guid::default())
    }

    pub(crate) fn link(
        &mut self,
        index: &TerrainGuidIndex,
        capture: &EntryCapture<'_>,
        ledger: &mut ExportLedger,
    ) -> Result<(), CatalogError> {
        let link = index.resolve(Kind::SoilTextureSets, &self.texture_set_guid, capture)?;
        if let Some(code) = link
            .state
            .value()
            .copied()
            .and_then(terrain_guid_link::link_issue)
        {
            let issue = DefinitionIssue {
                field: "soilTextureSetGUID",
                code,
            };
            issue.record(ledger, KIND);
            parsed_entry_json::push(&mut self.issues, issue)?;
        }
        self.texture_set = Some(link);
        Ok(())
    }
}
