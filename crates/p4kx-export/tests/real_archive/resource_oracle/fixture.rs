// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Write};
use std::path::Path;
use std::time::UNIX_EPOCH;

use p4kx_export::{
    CatalogOptions, CatalogPublicationMode, CatalogSummary, Compression, DECOMPRESSED_LIMIT, Kind,
    write_catalog_from,
};
use p4kx_extraction::{Archive, ArchiveWatch, DataForgeEntry};
use serde::Serialize;
use serde_json::Value;

use super::{Failure, Result, build, cardinality, values};

const MAX_RECORDS: usize = 2_000_000;

struct Limited(Vec<u8>);

impl Write for Limited {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let next = self
            .0
            .len()
            .checked_add(bytes.len())
            .ok_or_else(|| io::Error::other("catalog byte budget"))?;
        if u64::try_from(next).map_or(true, |next| next > DECOMPRESSED_LIMIT) {
            return Err(io::Error::other("catalog byte budget"));
        }
        self.0.try_reserve(bytes.len()).map_err(io::Error::other)?;
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Report {
    channel: &'static str,
    build_sha256: String,
    exact_cardinality_pin: build::Pin,
    commodities: cardinality::Counts,
    resources: values::Coverage,
}

pub(super) fn verify(path: &Path, channel: &'static str) -> Result<()> {
    let key =
        p4kx_testkit::real_archive_key().map_err(|error| Failure::source("archive-key", error))?;
    let archive = Archive::open_with_key(path, Some(key))
        .map_err(|error| Failure::source("archive-open", error))?;
    let watch = ArchiveWatch::new(&archive);
    watch
        .check()
        .map_err(|error| Failure::source("archive-source-changed", error))?;
    let identity = build::Identity::of(archive.build())?;
    let entry =
        DataForgeEntry::read(&archive).map_err(|error| Failure::source("dcb-read", error))?;
    watch
        .check()
        .map_err(|error| Failure::source("archive-source-changed", error))?;
    let forge = entry
        .open()
        .map_err(|error| Failure::source("dcb-open", error))?;
    if forge.records().len() > MAX_RECORDS {
        return Err(Failure::check("record-inventory-budget"));
    }
    let catalog = production_catalog(&archive, &entry)?;
    identity.check_catalog(&catalog)?;
    let counts = cardinality::check(&catalog)?;
    let pin = build::check(&identity, &counts)?;
    let resources = values::check(&forge, entry.origin(), &catalog, &watch)?;
    watch
        .check()
        .map_err(|error| Failure::source("archive-source-changed", error))?;
    report(&Report {
        channel,
        build_sha256: identity.tag()?,
        exact_cardinality_pin: pin,
        commodities: counts,
        resources,
    })
}

fn production_catalog(archive: &Archive, entry: &DataForgeEntry) -> Result<Value> {
    let options = CatalogOptions {
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..CatalogOptions::new(UNIX_EPOCH)
    };
    let (summary, written) =
        write_catalog_from(archive, entry, &options, Compression::None, || {
            Ok(Limited(Vec::new()))
        })
        .map_err(|error| Failure::source("production-catalog", error))?;
    complete_sources(&summary)?;
    serde_json::from_slice(&written.0).map_err(|error| Failure::source("catalog-json", error))
}

fn complete_sources(summary: &CatalogSummary) -> Result<()> {
    for kind in [Kind::Commodities, Kind::ResourceTypes] {
        let health = summary
            .export_health()
            .kind(kind)
            .ok_or_else(|| Failure::check("affected-kind-health"))?;
        let sources = health.source_records;
        if !health.source_inventory_complete
            || !health.field_inventory_complete
            || sources.omitted_after_read_failure != 0
            || sources.omitted_after_projection_failure != 0
            || sources.failed_selection_candidates != 0
            || sources.failed_dependencies != 0
        {
            return Err(Failure::check("affected-source-inventory-incomplete"));
        }
    }
    Ok(())
}
#[expect(
    clippy::print_stdout,
    reason = "The opt-in oracle emits only build hashes and aggregate coverage."
)]
fn report(value: &Report) -> Result<()> {
    let line =
        serde_json::to_string(value).map_err(|error| Failure::source("aggregate-report", error))?;
    println!("resource-oracle {line}");
    Ok(())
}
