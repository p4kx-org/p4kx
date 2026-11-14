// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;
use std::time::UNIX_EPOCH;

use p4kx_extraction::{Archive, ArchiveWatch, DataForgeEntry};
use serde::Serialize;
use serde_json::Value;

use super::super::resource_projection;
use super::{Failure, Result, build, cardinality, groups, values};
use crate::ExportHealth;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Report {
    channel: &'static str,
    build_sha256: String,
    exact_cardinality_pin: build::Pin,
    commodities: cardinality::Counts,
    resources: values::Coverage,
    resource_groups: usize,
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
    let catalog = production_catalog(&archive, &entry)?;
    identity.check_catalog(&catalog)?;
    let counts = cardinality::check(&catalog)?;
    let pin = build::check(&identity, &counts)?;
    let resources = values::check(&forge, entry.origin(), &catalog, &watch)?;
    let resource_groups = groups::check(&forge, entry.origin(), &catalog, &watch)?;
    watch
        .check()
        .map_err(|error| Failure::source("archive-source-changed", error))?;
    report(&Report {
        channel,
        build_sha256: identity.tag()?,
        exact_cardinality_pin: pin,
        commodities: counts,
        resources,
        resource_groups,
    })
}

fn production_catalog(archive: &Archive, entry: &DataForgeEntry) -> Result<Value> {
    let (catalog, health) = resource_projection::read(archive, entry, UNIX_EPOCH)
        .map_err(|error| Failure::source("resource-projection", error))?;
    complete_sources(&health)?;
    Ok(catalog)
}

pub(super) fn complete_sources(health: &ExportHealth) -> Result<()> {
    for kind in resource_projection::COVERED {
        let health = health
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
