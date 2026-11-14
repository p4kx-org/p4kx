// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, DataForge};
use schemars::JsonSchema;
use serde::Serialize;

use super::planet_climate_asset::{self, PlanetClimateAsset};
use super::planet_creature::{self, PlanetCreatureTable};
use super::planet_geology_asset::{self, PlanetGeologyAsset};
use super::planet_resource::{self, PlanetResourceTable};
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_read::original::EntryCapture;
use crate::catalog_table::{CatalogTable, DatasetAvailability};
use crate::export_ledger::ExportLedger;
use crate::schema_fields::every_field_required;
use crate::{CatalogError, CatalogOptions};

/// Original loose planet-tech families under the canonical definitions namespace.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct PlanetTechDefinitions {
    pub(crate) planet_resources: PlanetResourceTable,
    pub(crate) creatures: PlanetCreatureTable,
    pub(crate) climate_assets: CatalogTable<PlanetClimateAsset>,
    pub(crate) geology_assets: CatalogTable<PlanetGeologyAsset>,
}

impl Default for PlanetTechDefinitions {
    fn default() -> Self {
        Self {
            planet_resources: PlanetResourceTable::empty(DatasetAvailability::Absent),
            creatures: PlanetCreatureTable::empty(DatasetAvailability::Absent),
            climate_assets: CatalogTable::empty(DatasetAvailability::Absent),
            geology_assets: CatalogTable::empty(DatasetAvailability::Absent),
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
    Ok(PlanetTechDefinitions {
        planet_resources: planet_resource::read(
            archive, forge, original, capture, ledger, options,
        )?,
        creatures: planet_creature::read(archive, forge, original, capture, ledger, options)?,
        climate_assets: planet_climate_asset::read(
            archive, forge, original, capture, ledger, options,
        )?,
        geology_assets: planet_geology_asset::read(
            archive, forge, original, capture, ledger, options,
        )?,
    })
}
