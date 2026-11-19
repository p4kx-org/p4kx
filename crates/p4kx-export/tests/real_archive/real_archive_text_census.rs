// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_export::{CatalogOptions, CatalogSummary, Compression, Kind, SCHEMA_VERSION};
use p4kx_extraction::Archive;
use serde_json::Value;

use crate::catalog_capture::CatalogCapture;
use crate::catalog_v3_consumer;
use crate::real_archive_catalog_size::artifact;

#[path = "real_archive_item_text_capture.rs"]
mod item_capture;
#[path = "real_archive_text_pipe.rs"]
mod pipe;

#[cfg(test)]
#[path = "real_archive_text_census_tests.rs"]
mod tests;

pub(super) type TestResult<T> = Result<T, Box<dyn std::error::Error>>;

pub(super) fn write_catalog(
    archive: &Archive,
    options: &CatalogOptions<'_>,
    before_write: impl FnOnce(),
) -> TestResult<(CatalogSummary, Value)> {
    let paths = catalog_paths();
    let (summary, capture) = pipe::transfer(&paths, |sink| {
        p4kx_export::write_catalog(archive, options, Compression::None, || {
            before_write();
            Ok(sink)
        })
    })?;
    if capture.bytes != summary.size() {
        return Err("text census byte count differs from the catalog writer".into());
    }
    Ok((summary, schema_v3(capture.document)?))
}

pub(super) fn read_completed_catalog(directory: &Path) -> TestResult<Value> {
    let paths = catalog_paths();
    let projection = artifact::read_completed(directory, |decoded| Ok(paths.read(decoded)?))?;
    schema_v3(projection)
}

fn schema_v3(document: Value) -> TestResult<Value> {
    if document.get("schemaVersion").and_then(Value::as_u64) != Some(u64::from(SCHEMA_VERSION)) {
        return Err("the text census requires catalog schema v3".into());
    }
    Ok(document)
}

type TextPaths = CatalogCapture<item_capture::Projection>;

fn catalog_paths() -> TextPaths {
    let mut paths = TextPaths::default();
    for kind in Kind::ALL {
        paths.keep(&catalog_v3_consumer::row_location(kind));
    }
    for path in [
        "/schemaVersion",
        "/source",
        "/completeness",
        "/exportHealth/status",
        "/exportHealth/read",
        "/exportHealth/projection",
        "/exportHealth/publicationMode",
        "/exportHealth/kinds",
        "/texts",
        "/configurations/contractTemplates",
        "/configurations/missionBrokerEntries",
        "/configurations/blueprintTiers",
        "/projections/blueprintPresentation",
        "/projections/commodityPresentation",
    ] {
        paths.keep(path);
    }
    paths.project("/definitions/items", item_capture::ITEM_DEFINITIONS);
    paths.project(
        "/provenance/sources",
        item_capture::Projection::ArchiveEntrySources,
    );
    paths
}
pub(in crate::real_archive_texts) fn read_fixture_through_pipe(input: &[u8]) -> TestResult<Value> {
    use std::io::Write as _;

    let ((), capture) = pipe::transfer(&catalog_paths(), |mut sink| {
        sink.write_all(input)
            .map_err(p4kx_export::CatalogError::Write)?;
        Ok(((), sink))
    })?;
    Ok(capture.document)
}
