// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;
use std::path::Path;

use p4kx_extraction::{Archive, ArchiveWatch};
use serde::Serialize;

use super::ReducedError;
use super::bounded_output::{self, Limits, Sizes};
use super::plan;
use super::selected_tables::{CompleteTables, SelectedTables};
use super::selected_texts::SelectedTexts;
use super::{stage_economy, stage_equipment, stage_missions, stage_navigation, stage_weapons};
use crate::archive_key_use::ArchiveKeyUse;
use crate::build;
use crate::catalog_queries::{CatalogLanguageSelection, CatalogQuerySession};
use crate::catalog_query_session::checkpoint;
use crate::catalog_rows::spool::RowSpoolError;
use crate::contract::{SCHEMA_VERSION, Source};
use crate::ship_store_pledge::ShipStorePledges;
use crate::{CatalogError, CatalogFile, CatalogOptions};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Document<'source> {
    format: &'static str,
    format_version: u32,
    catalog: Catalog<'source>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Catalog<'source> {
    schema_version: u32,
    source: Source<'source>,
    export_health: Health,
    ship_store_pledges: ShipStorePledges,
    texts: SelectedTexts,
    #[serde(flatten)]
    tables: CompleteTables,
}

#[derive(Serialize)]
struct Health {
    status: HealthStatus,
    scope: &'static [&'static str],
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
enum HealthStatus {
    Complete,
}

impl Document<'_> {
    fn take_failure(&self) -> Option<RowSpoolError> {
        self.catalog.tables.take_spool_failure()
    }
}

fn stage<'source>(
    archive: &'source Archive,
    scratch: &Path,
    options: &CatalogOptions<'source>,
) -> Result<Document<'source>, ReducedError> {
    let demand = archive.key_demand().scope();
    let cancel = options.cancel;
    let session = CatalogQuerySession::open(
        archive,
        CatalogLanguageSelection::Selected(&plan::LOCALES),
        cancel,
    )?;
    let parsed = session.parsed(cancel)?;
    let mut tables = SelectedTables::default();
    let mut texts = SelectedTexts::default();
    let equipment = stage_equipment::stage(&parsed, scratch, cancel, &mut tables, &mut texts)?;
    stage_economy::stage(archive, &parsed, scratch, cancel, &mut tables, &mut texts)?;
    stage_navigation::stage(
        &parsed,
        scratch,
        cancel,
        options.starmap_budget,
        &mut tables,
        &mut texts,
    )?;
    stage_missions::stage(&parsed, scratch, cancel, &mut tables, &mut texts)?;
    stage_weapons::stage(
        &parsed,
        scratch,
        cancel,
        &equipment.item_targets,
        &mut tables,
    )?;
    let tables = tables.finish()?;
    let key_use = ArchiveKeyUse::scoped(archive, &demand);
    let source = build::source(
        archive,
        options.exported_at,
        options.public_patches,
        key_use,
    );
    Ok(Document {
        format: plan::FORMAT,
        format_version: plan::FORMAT_VERSION,
        catalog: Catalog {
            schema_version: SCHEMA_VERSION,
            source,
            export_health: Health {
                status: HealthStatus::Complete,
                scope: &plan::SELECTION,
            },
            ship_store_pledges: equipment.ship_store_pledges,
            texts,
            tables,
        },
    })
}

pub(super) fn prepare<'output>(
    archive: &Archive,
    scratch: &Path,
    options: &CatalogOptions<'_>,
    create: impl FnOnce() -> io::Result<CatalogFile<'output>>,
) -> Result<(CatalogFile<'output>, Sizes), ReducedError> {
    let watch = ArchiveWatch::new(archive);
    watch.check().map_err(CatalogError::from)?;
    let document = stage(archive, scratch, options)?;
    let cancel = options.cancel;
    let limits = Limits::default();
    let decoded = bounded_output::measure(&document, limits.decoded, cancel)
        .map_err(|error| document.take_failure().map_or(error, ReducedError::from))?;
    watch.check().map_err(CatalogError::from)?;
    checkpoint(cancel)?;
    let mut file = create().map_err(CatalogError::Write)?;
    match write(&document, &mut file, decoded, limits, options, &watch) {
        Ok(sizes) => Ok((file, sizes)),
        Err(primary) => match file.discard() {
            Ok(()) => Err(primary),
            Err(cleanup) => Err(ReducedError::Cleanup {
                primary: Box::new(primary),
                cleanup,
            }),
        },
    }
}

fn write(
    document: &Document<'_>,
    file: &mut CatalogFile<'_>,
    decoded: u64,
    limits: Limits,
    options: &CatalogOptions<'_>,
    watch: &ArchiveWatch,
) -> Result<Sizes, ReducedError> {
    let cancel = options.cancel;
    let sizes = bounded_output::write(document, file, decoded, limits, cancel)
        .map_err(|error| document.take_failure().map_or(error, ReducedError::from))?;
    watch.check().map_err(CatalogError::from)?;
    checkpoint(cancel)?;
    Ok(sizes)
}
