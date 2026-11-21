// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The inputs of the variant keys in a real Archive: how many ships name one vehicle definition in
//! several spellings, and whether any GUID repeats; runs with `--ignored` when `P4KX_DATA_P4K`
//! names an Archive.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::error::Error;
use std::time::SystemTime;

use p4kx_export::{CatalogOptions, build_catalog};
use p4kx_extraction::Archive;
use p4kx_testkit::real_archive_path;
use serde_json::Value;

type TestResult = Result<(), Box<dyn Error>>;

#[expect(
    clippy::print_stdout,
    reason = "the diagnostics are the result of the run"
)]
fn print_line(line: &str) {
    println!("{line}");
}

fn normalised(path: &str) -> String {
    path.trim_start_matches(['/', '\\'])
        .chars()
        .map(|c| {
            if c == '\\' {
                '/'
            } else {
                c.to_ascii_lowercase()
            }
        })
        .collect()
}

fn entries<'c>(catalog: &'c Value, kind: &str) -> &'c [Value] {
    catalog[kind].as_array().map_or(&[][..], Vec::as_slice)
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn the_vehicle_definition_spellings_and_guids_of_a_real_archive() -> TestResult {
    let archive =
        Archive::open_with_key(real_archive_path()?, p4kx_testkit::real_archive_key().ok())?;
    let built = build_catalog(&archive, &CatalogOptions::new(SystemTime::now()))?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    let summary = built.summary().variants();
    print_line(&format!(
        "summary ships {:?}, items {:?}",
        summary.ships, summary.items
    ));

    let mut spellings: BTreeMap<String, BTreeSet<&str>> = BTreeMap::new();
    for ship in entries(&catalog, "ships") {
        if let Some(definition) = ship["vehicleDefinition"]["value"].as_str() {
            spellings
                .entry(normalised(definition))
                .or_default()
                .insert(definition);
        }
    }
    let split: Vec<_> = spellings.iter().filter(|(_, raw)| raw.len() > 1).collect();
    print_line(&format!(
        "{} distinct normalised definitions, {} with several spellings",
        spellings.len(),
        split.len()
    ));
    for (key, raw) in &split {
        print_line(&format!("  {key}: {raw:?}"));
    }

    let mut repeated = 0_usize;
    for kind in ["ships", "items"] {
        let mut seen: HashMap<&str, usize> = HashMap::new();
        for entry in entries(&catalog, kind) {
            if let Some(guid) = entry["guid"]["value"].as_str() {
                *seen.entry(guid).or_default() += 1;
            }
        }
        let duplicates: Vec<_> = seen.iter().filter(|(_, count)| **count > 1).collect();
        print_line(&format!("{kind}: {} repeated GUIDs", duplicates.len()));
        repeated += duplicates.len();
    }
    assert_eq!(repeated, 0, "no GUID repeats in a well-formed Archive");
    Ok(())
}
