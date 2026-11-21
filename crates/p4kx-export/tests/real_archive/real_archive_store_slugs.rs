// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The curated ship store slugs against the ships of a real Archive; runs with `--ignored` when
//! `P4KX_DATA_P4K` names one.

use std::collections::HashMap;
use std::error::Error;
use std::time::SystemTime;

use p4kx_export::{CatalogOptions, build_catalog};
use p4kx_extraction::Archive;
use p4kx_testkit::{real_archive_key, real_archive_path};
use serde_json::Value;

const MIN_RESOLVED: usize = 200;

#[expect(
    clippy::print_stdout,
    reason = "the counts are the result of the verification"
)]
#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn every_mapped_store_slug_resolves_to_a_ship_of_the_archive() -> Result<(), Box<dyn Error>> {
    let archive = Archive::open_with_key(real_archive_path()?, Some(real_archive_key()?))?;
    let built = build_catalog(&archive, &CatalogOptions::new(SystemTime::now()))?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    let counts = built.summary().store_slugs();
    println!(
        "ship store slugs: {} slugs, {} mapped, {} resolved, {} retired, {} unmapped, stale {:?}",
        counts.slugs, counts.mapped, counts.resolved, counts.retired, counts.unmapped, counts.stale
    );
    assert!(
        counts.stale.is_empty(),
        "mapped slugs that no longer resolve: {:?}",
        counts.stale
    );
    assert!(
        counts.resolved >= MIN_RESOLVED,
        "{} resolved",
        counts.resolved
    );
    assert_eq!(counts.mapped, counts.resolved + counts.retired);
    assert_eq!(counts.slugs, counts.mapped + counts.unmapped);

    let classes: HashMap<&str, &str> = catalog["ships"]
        .as_array()
        .ok_or("ships")?
        .iter()
        .filter_map(|ship| {
            Some((
                ship["guid"]["value"].as_str()?,
                ship["className"]["value"].as_str()?,
            ))
        })
        .collect();
    let slugs = catalog["shipStoreSlugs"]
        .as_array()
        .ok_or("shipStoreSlugs")?;
    assert_eq!(
        slugs.len(),
        counts.exported,
        "the summary counts the entries"
    );
    assert_eq!(slugs.len(), counts.resolved + counts.retired);
    let names: Vec<&str> = slugs
        .iter()
        .filter_map(|slug| slug["slug"].as_str())
        .collect();
    assert!(names.is_sorted(), "entries are ordered by slug");
    for slug in slugs {
        let name = slug["slug"].as_str().ok_or("slug")?;
        if slug["retired"] == true {
            assert!(slug["shipGuid"].is_null(), "retired {name} has no ship");
            continue;
        }
        let guid = slug["shipGuid"].as_str().ok_or("shipGuid")?;
        assert_eq!(
            classes.get(guid).copied(),
            slug["className"].as_str(),
            "{name} maps to a ship of the catalog under its class name"
        );
    }
    Ok(())
}
