// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;

use p4kx_extraction::{Archive, DataForge, Guid, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::planet_asset_raw_fields::{self, RawPlanetAssetMember};
use super::planet_climate_properties::{ClimateAppearance, ClimateEnvelope, ClimateGrowth};
use super::planet_climate_subasset::ClimateSubAsset;
use super::planet_resource;
use super::terrain_guid_link::{self, TerrainGuidIndex, TerrainTargetFamily};
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_read::catalog_json_number::CatalogJsonNumber;
use crate::catalog_read::entry_json_object;
use crate::catalog_read::loose_definition::{
    self, DefinitionFamily, DefinitionIssue, LooseDefinition,
};
use crate::catalog_read::original::{EntryCapture, PhysicalEntryIdentity};
use crate::catalog_read::parsed_entry_json;
use crate::catalog_table::CatalogTable;
use crate::export_ledger::ExportLedger;
use crate::guid_text::CatalogGuid;
use crate::schema_fields::every_field_required;
use crate::{CatalogError, CatalogOptions, Kind};

pub(super) const FAMILY: DefinitionFamily = DefinitionFamily::new(
    Kind::ClimateAssets,
    "data/libs/planets/climate_assets/",
    "climasset",
    "PlanetClimateAsset",
    "GUID",
);

/// Original physical climate-asset definitions and ordered declarations.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct PlanetClimateAsset {
    identity: PhysicalEntryIdentity,
    guid: SourcedValue<CatalogGuid>,
    guid_raw: SourcedValue<String>,
    appearance: ClimateAppearance,
    envelope: ClimateEnvelope,
    growth: ClimateGrowth,
    sub_assets: SourcedValue<Vec<SourcedValue<ClimateSubAsset>>>,
    audio_offset: SourcedValue<Vec<SourcedValue<CatalogJsonNumber>>>,
    #[schemars(extend("x-gameText" = true))]
    procedural_audio: SourcedValue<String>,
    additional_fields: Vec<RawPlanetAssetMember>,
    issues: Vec<DefinitionIssue>,
}

impl PlanetClimateAsset {
    pub(super) fn required_shading_guids(&self) -> impl Iterator<Item = CatalogGuid> + '_ {
        self.appearance.shading_guids()
    }

    pub(super) fn link_shading(
        &mut self,
        index: &TerrainGuidIndex,
        capture: &EntryCapture<'_>,
        ledger: &mut ExportLedger,
    ) -> Result<(), CatalogError> {
        let links = self.appearance.attach_shading_links(index, capture)?;
        for (field, link) in [
            ("shadingClassGUID", &links.default),
            ("shadingClassCloseGUID", &links.close),
            ("shadingClassMidGUID", &links.mid),
            ("shadingClassFarGUID", &links.far),
        ] {
            if let Some(code) = link
                .state
                .value()
                .copied()
                .and_then(terrain_guid_link::link_issue)
            {
                let issue = DefinitionIssue { field, code };
                issue.record(ledger, Kind::ClimateAssets);
                parsed_entry_json::push(&mut self.issues, issue)?;
            }
        }
        Ok(())
    }
}

impl LooseDefinition for PlanetClimateAsset {
    fn guid(&self) -> &SourcedValue<CatalogGuid> {
        &self.guid
    }
    fn issues(&mut self) -> &mut Vec<DefinitionIssue> {
        &mut self.issues
    }
}

pub(crate) fn read(
    archive: &Archive,
    forge: &DataForge<'_>,
    original: &OriginalCatalogInput<'_, '_>,
    capture: &EntryCapture<'_>,
    ledger: &mut ExportLedger,
    options: &CatalogOptions<'_>,
) -> Result<CatalogTable<PlanetClimateAsset>, CatalogError> {
    let table = loose_definition::read(
        archive,
        capture,
        ledger,
        options,
        FAMILY,
        |identity, input| {
            let o = &input.retaining_unprojected();
            let guid = o.field("GUID", entry_json_object::guid)?;
            let issues = loose_definition::identity_issue("GUID", &guid)
                .into_iter()
                .collect();
            Ok(PlanetClimateAsset {
                identity,
                guid,
                guid_raw: o.field("GUID", entry_json_object::text)?,
                appearance: ClimateAppearance::read(o)?,
                envelope: ClimateEnvelope::read(o)?,
                growth: ClimateGrowth::read(o)?,
                sub_assets: o.object_array("subAssets", |asset| {
                    ClimateSubAsset::read(asset, forge, original, options)
                })?,
                audio_offset: o.array("vAudioOffset", entry_json_object::number)?,
                procedural_audio: o.field("sProceduralAudio", entry_json_object::text)?,
                additional_fields: planet_asset_raw_fields::collect(o)?,
                issues,
            })
        },
    )?;
    link_resources(table, archive, capture, ledger, options)
}

fn link_resources(
    mut table: CatalogTable<PlanetClimateAsset>,
    archive: &Archive,
    capture: &EntryCapture<'_>,
    ledger: &mut ExportLedger,
    options: &CatalogOptions<'_>,
) -> Result<CatalogTable<PlanetClimateAsset>, CatalogError> {
    let mut requested = HashSet::new();
    for asset in table
        .rows
        .iter()
        .flat_map(|row| row.sub_assets.value().into_iter().flatten())
    {
        let Some(guid) = asset
            .value()
            .and_then(|asset| asset.resource_guid().value())
        else {
            continue;
        };
        if guid.0 != Guid::default() {
            requested
                .try_reserve(1)
                .map_err(CatalogError::JsonSourceAllocation)?;
            requested.insert(*guid);
        }
    }
    let family = TerrainTargetFamily {
        kind: Kind::PlanetResources,
        selector: planet_resource::FAMILY,
        guid_field: "guid",
    };
    let index = TerrainGuidIndex::read(
        archive,
        capture,
        options.cancel,
        options.filter,
        vec![family],
        &requested,
    )?;
    for row in &mut table.rows {
        let placeholder =
            capture
                .source_uses()
                .value(p4kx_extraction::SourceValue::Absent, &[], false);
        let assets = std::mem::replace(&mut row.sub_assets, placeholder);
        row.sub_assets = assets.try_map(|elements| {
            let mut linked = Vec::new();
            linked
                .try_reserve_exact(elements.len())
                .map_err(CatalogError::JsonSourceAllocation)?;
            for element in elements {
                parsed_entry_json::check_cancel(options.cancel)?;
                linked.push(element.try_map(|mut asset| {
                    asset.link_resource(&index, capture, ledger)?;
                    Ok::<_, CatalogError>(asset)
                })?);
            }
            Ok::<_, CatalogError>(linked)
        })?;
    }
    Ok(table)
}
