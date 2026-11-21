// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;

use p4kx_extraction::{Archive, DataForge};
use schemars::JsonSchema;
use serde::Serialize;

use super::ecosystem::{self, EcosystemTable};
use super::geology_set::{self, GeologySetTable};
use super::geology_texture_set::{self, GeologyTextureSetTable};
use super::planet_climate_asset::{self, PlanetClimateAsset};
use super::planet_creature::{self, PlanetCreatureTable};
use super::planet_geology_asset::{self, PlanetGeologyAsset};
use super::planet_resource::{self, PlanetResourceTable};
use super::shading_class::{self, ShadingClassTable};
use super::soil::{self, SoilTable};
use super::soil_texture_set::{self, SoilTextureSetTable};
pub(crate) use super::terrain_guid_link::TerrainGuidIndex;
use super::terrain_guid_link::TerrainTargetFamily;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_read::loose_archive_path::Resolver;
use crate::catalog_read::original::EntryCapture;
use crate::catalog_read::parsed_entry_json;
use crate::catalog_table::{CatalogTable, DatasetAvailability};
use crate::export_ledger::ExportLedger;
use crate::guid_text::CatalogGuid;
use crate::schema_fields::every_field_required;
use crate::{CatalogError, CatalogOptions, Kind};

/// Original loose planet-tech families under the canonical definitions namespace.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct PlanetTechDefinitions {
    pub(crate) planet_resources: PlanetResourceTable,
    pub(crate) creatures: PlanetCreatureTable,
    pub(crate) climate_assets: CatalogTable<PlanetClimateAsset>,
    pub(crate) geology_assets: CatalogTable<PlanetGeologyAsset>,
    pub(crate) ecosystems: EcosystemTable,
    pub(crate) geology_sets: GeologySetTable,
    pub(crate) geology_texture_sets: GeologyTextureSetTable,
    pub(crate) soils: SoilTable,
    pub(crate) soil_texture_sets: SoilTextureSetTable,
    pub(crate) shading_classes: ShadingClassTable,
}

impl Default for PlanetTechDefinitions {
    fn default() -> Self {
        Self {
            planet_resources: PlanetResourceTable::empty(DatasetAvailability::Absent),
            creatures: PlanetCreatureTable::empty(DatasetAvailability::Absent),
            climate_assets: CatalogTable::empty(DatasetAvailability::Absent),
            geology_assets: CatalogTable::empty(DatasetAvailability::Absent),
            ecosystems: EcosystemTable::empty(DatasetAvailability::Absent),
            geology_sets: GeologySetTable::empty(DatasetAvailability::Absent),
            geology_texture_sets: GeologyTextureSetTable::empty(DatasetAvailability::Absent),
            soils: SoilTable::empty(DatasetAvailability::Absent),
            soil_texture_sets: SoilTextureSetTable::empty(DatasetAvailability::Absent),
            shading_classes: ShadingClassTable::empty(DatasetAvailability::Absent),
        }
    }
}

pub(crate) fn read(
    archive: &Archive,
    forge: &DataForge<'_>,
    original: &OriginalCatalogInput<'_, '_>,
    capture: &EntryCapture<'_>,
    ledger: &mut ExportLedger,
    options: &CatalogOptions<'_>,
) -> Result<PlanetTechDefinitions, CatalogError> {
    let mut paths = Resolver::new(archive, capture, options.cancel);
    let mut result = PlanetTechDefinitions {
        planet_resources: planet_resource::read(
            archive, forge, original, capture, ledger, options,
        )?,
        creatures: planet_creature::read(archive, forge, original, capture, ledger, options)?,
        climate_assets: planet_climate_asset::read(archive, original, capture, ledger, options)?,
        geology_assets: planet_geology_asset::read(
            archive, forge, original, capture, ledger, options,
        )?,
        ecosystems: ecosystem::read(archive, capture, ledger, options, &mut paths)?,
        geology_sets: geology_set::read(archive, capture, ledger, options)?,
        geology_texture_sets: geology_texture_set::read(
            archive, capture, ledger, options, &mut paths,
        )?,
        soils: soil::read(archive, capture, ledger, options)?,
        soil_texture_sets: soil_texture_set::read(archive, capture, ledger, options, &mut paths)?,
        shading_classes: shading_class::read(archive, capture, ledger, options, &mut paths)?,
    };
    result.link_terrain(archive, capture, ledger, options)?;
    Ok(result)
}

impl PlanetTechDefinitions {
    fn link_terrain(
        &mut self,
        archive: &Archive,
        capture: &EntryCapture<'_>,
        ledger: &mut ExportLedger,
        options: &CatalogOptions<'_>,
    ) -> Result<(), CatalogError> {
        let wanted = requested_terrain_guids(
            self.geology_sets
                .rows
                .iter()
                .flat_map(geology_set::GeologySet::required_guids)
                .chain(self.soils.rows.iter().flat_map(soil::Soil::required_guids))
                .chain(
                    self.climate_assets
                        .rows
                        .iter()
                        .flat_map(PlanetClimateAsset::required_shading_guids),
                ),
            options.cancel,
        )?;
        let index = TerrainGuidIndex::read(
            archive,
            capture,
            options.cancel,
            options.filter,
            terrain_target_families(),
            &wanted,
        )?;
        for row in &mut self.geology_sets.rows {
            parsed_entry_json::check_cancel(options.cancel)?;
            row.link(&index, capture, ledger, options.cancel)?;
        }
        for row in &mut self.soils.rows {
            parsed_entry_json::check_cancel(options.cancel)?;
            row.link(&index, capture, ledger)?;
        }
        for row in &mut self.climate_assets.rows {
            parsed_entry_json::check_cancel(options.cancel)?;
            row.link_shading(&index, capture, ledger)?;
        }
        Ok(())
    }
}

pub(crate) fn requested_terrain_guids(
    guids: impl IntoIterator<Item = CatalogGuid>,
    cancel: Option<&p4kx_extraction::CancelToken>,
) -> Result<HashSet<CatalogGuid>, CatalogError> {
    let mut wanted = HashSet::new();
    for guid in guids {
        parsed_entry_json::check_cancel(cancel)?;
        wanted
            .try_reserve(1)
            .map_err(CatalogError::JsonSourceAllocation)?;
        wanted.insert(guid);
    }
    Ok(wanted)
}

/// Original definition families that can contribute terrain GUID candidates.
pub(crate) fn terrain_target_families() -> Vec<TerrainTargetFamily> {
    [
        (Kind::GeologyAssets, planet_geology_asset::FAMILY),
        (Kind::GeologyTextureSets, geology_texture_set::FAMILY),
        (Kind::SoilTextureSets, soil_texture_set::FAMILY),
        (Kind::ShadingClasses, shading_class::FAMILY),
    ]
    .into_iter()
    .map(|(kind, selector)| TerrainTargetFamily {
        kind,
        selector,
        guid_field: "GUID",
    })
    .collect()
}
