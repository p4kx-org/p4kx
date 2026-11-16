// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Extracts the resource wire regions through the production readers.

use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::SystemTime;

use p4kx_assembly::{ArchiveFiles, Entities};
use p4kx_extraction::{Archive, DataForge, DataForgeEntry, Guid};
use serde::Serialize;
use serde_json::{Value, json};

use crate::archive_key_use::ArchiveKeyUse;
use crate::build::{original_input::OriginalCatalogInput, parts, source};
use crate::catalog_crafting;
use crate::catalog_origins::CatalogOrigins;
use crate::catalog_projection::{CatalogProjection, CatalogTextRegistry};
use crate::catalog_table::CatalogTable;
use crate::catalog_v3::resource_group_catalog;
use crate::collected::Collected;
use crate::contract::SCHEMA_VERSION;
use crate::crafting::ledger::{self, ReadMode};
use crate::scan::{Scan, ScanDomain};
use crate::starmap_containers::Placements;
use crate::texts::{TextSources, Texts};
use crate::{
    CatalogError, CatalogOptions, CatalogProvenance, CatalogPublicationMode, ExportHealth, Kind,
};

/// Canonical resource inventories checked by the installed oracle.
pub(super) const COVERED: [Kind; 3] =
    [Kind::Commodities, Kind::ResourceTypes, Kind::ResourceGroups];
const DEPENDENCIES: [Kind; 3] = [
    Kind::Blueprints,
    Kind::QualityDistributions,
    Kind::QualityQuantizations,
];

/// Assembles the private resource view of the archive.
pub(super) fn read(
    archive: &Archive,
    entry: &DataForgeEntry,
    exported_at: SystemTime,
) -> Result<(Value, ExportHealth), CatalogError> {
    let options = CatalogOptions {
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..CatalogOptions::new(exported_at)
    };
    let forge = entry.open().map_err(CatalogError::DataForge)?;
    let sources = TextSources::read(archive)?;
    let texts = Texts::parse(&sources)?;
    let origins = CatalogOrigins::new(archive, entry, &sources)?;
    let input = OriginalCatalogInput::new(&forge, &origins);
    let text_registry = CatalogTextRegistry::default();
    let projection = CatalogProjection::new(&texts, &input, &text_registry);
    let crafting = catalog_crafting::kinds(
        ledger::read_catalog(
            &forge,
            options.filter,
            options.cancel,
            options.progress,
            ReadMode::resource_oracle(&projection),
        )?,
        &origins,
        options.filter,
    )?;
    let files = ArchiveFiles::new(archive);
    let entities = Arc::new(Entities::new(&forge));
    let placements = Placements::default();
    let locations = quality_location_targets(&crafting.ledger)?;
    let domain = ScanDomain::resources(&placements, &locations);
    let mut collected = Scan::run(
        &input,
        &files,
        entities,
        &options,
        &projection,
        crafting,
        domain,
    )?;
    collected.finish_selection();

    let health = resource_health(&mut collected, &input, &options)?;
    let metadata = source(
        archive,
        exported_at,
        options.public_patches,
        ArchiveKeyUse::of(archive),
    );
    let texts = json_value(&projection.finish()?.texts)?;
    let provenance = CatalogProvenance::new(origins.uses.freeze()?);
    let catalog = json!({
        "schemaVersion": SCHEMA_VERSION,
        "source": json_value(&metadata)?,
        "texts": texts,
        "provenance": json_value(&provenance)?,
    });
    Ok((tables(catalog, collected, &forge)?, health))
}

fn quality_location_targets(
    ledger: &crate::export_ledger::ExportLedger,
) -> Result<BTreeSet<Guid>, CatalogError> {
    ledger
        .field_inspections
        .iter()
        .flat_map(|inspection| &inspection.catalog_references)
        .filter(|reference| reference.target_kind == Kind::StarmapObjects)
        .map(|reference| Ok(reference.target()?.guid))
        .collect()
}

fn resource_health(
    rows: &mut Collected<'_>,
    input: &OriginalCatalogInput<'_, '_>,
    options: &CatalogOptions<'_>,
) -> Result<ExportHealth, CatalogError> {
    let mut kinds = rows.summary(false);
    let covered = COVERED.into_iter().chain(DEPENDENCIES);
    for kind in &mut kinds {
        kind.complete = covered.clone().any(|wanted| wanted == kind.kind);
    }
    parts::capture_rows(rows, None, Vec::new(), Vec::new());
    crate::export_health_compile::prepare_report_for(
        std::mem::take(&mut rows.ledger),
        &kinds,
        rows.failures.iter().map(crate::CatalogReadFailure::guid),
        &rows.warnings,
        options.publication_mode,
        covered,
    )?
    .finish(input, &rows.failures, options.cancel)
}

fn tables(
    mut catalog: Value,
    rows: Collected<'_>,
    forge: &DataForge<'_>,
) -> Result<Value, CatalogError> {
    let commodities = crate::commodity::tables(forge, rows.commodities);
    let groups =
        resource_group_catalog::tables(forge, rows.resource_groups, rows.resource_group_members);
    let resources = CatalogTable::new(forge, &["ResourceType"], rows.resource_types);
    catalog["definitions"] = json!({
        "commodities": json_value(&commodities.definitions)?,
        "resourceGroups": json_value(&groups.definitions)?,
        "resourceTypes": json_value(&resources)?,
    });
    catalog["declarations"] = json!({
        "resourceGroupMembers": json_value(&groups.members)?,
    });
    catalog["projections"] = json!({
        "commodityPresentation": json_value(&commodities.presentation)?,
        "resourceGroupParents": json_value(&groups.parents)?,
    });
    Ok(catalog)
}

fn json_value(value: &impl Serialize) -> Result<Value, CatalogError> {
    let bytes = serde_json::to_vec(value).map_err(CatalogError::Json)?;
    serde_json::from_slice(&bytes).map_err(CatalogError::Json)
}
