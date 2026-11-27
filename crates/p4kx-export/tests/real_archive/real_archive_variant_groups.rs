// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The variant groups read without a catalog build on a real Archive agree with the `variantOf` of
//! its catalog; runs with `--ignored` when `P4KX_DATA_P4K` names one. Run with `--nocapture` to
//! see the counts and the time of the read.

use std::error::Error;
use std::time::{Instant, SystemTime};

use p4kx_export::{CatalogOptions, Guid, Kind, build_catalog, read_variant_groups};
use p4kx_extraction::{Archive, DataForgeEntry};
use p4kx_testkit::real_archive_path;
use serde_json::Value;

use crate::catalog_v3_consumer;

type TestResult = Result<(), Box<dyn Error>>;

#[expect(
    clippy::print_stdout,
    reason = "the diagnostics are the result of the run"
)]
fn print_line(line: &str) {
    println!("{line}");
}

fn guid(value: &Value) -> Result<Guid, Box<dyn Error>> {
    Ok(value.as_str().ok_or("not a GUID")?.parse()?)
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn the_groups_read_without_a_catalog_are_the_variant_of_of_the_catalog() -> TestResult {
    let archive =
        Archive::open_with_key(real_archive_path()?, p4kx_testkit::real_archive_key().ok())?;
    let built = build_catalog(
        &archive,
        CatalogOptions::new(SystemTime::now()).with_full_provenance(),
    )?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;

    let started = Instant::now();
    let groups = read_variant_groups(&archive, &forge, &|| false).ok_or("stopped")?;
    let took = started.elapsed();

    let mut wrong = Vec::new();
    for kind in [Kind::Ships, Kind::Items] {
        for entity in catalog_v3_consumer::rows(&catalog, kind)? {
            let expected = crate::catalog_variant_value::variant_target(entity, kind)?
                .map(|target| target.guid.parse())
                .transpose()?;
            if groups.base_of(guid(&entity["guid"]["value"])?) != expected {
                wrong.push(
                    entity["className"]["value"]
                        .as_str()
                        .unwrap_or_default()
                        .to_owned(),
                );
            }
        }
    }
    let summary = groups.summary();
    print_line(&format!(
        "variant groups read in {took:?}: ships {} bases, {} variants; items {} bases, {} variants",
        summary.ships.groups, summary.ships.variants, summary.items.groups, summary.items.variants
    ));
    assert!(wrong.is_empty(), "{} differ: {wrong:?}", wrong.len());
    assert_eq!(summary, built.summary().variants(), "the counts");
    Ok(())
}
