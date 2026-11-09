// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The quality quantizations of the Basetool catalog of a real Archive; runs with `--ignored` when
//! `P4KX_DATA_P4K` names one.

use std::collections::HashSet;
use std::error::Error;
use std::time::SystemTime;

use p4kx_export::{CatalogOptions, build_catalog};
use p4kx_extraction::{Archive, DataForgeEntry, Guid};
use p4kx_testkit::real_archive_path;
use serde_json::Value;

use crate::catalog_v3_consumer;

type TestResult = Result<(), Box<dyn Error>>;

const MIN_RECORDS: usize = 60;

#[expect(
    clippy::print_stdout,
    reason = "the measurements are the result of the verification"
)]
fn print_line(line: &str) {
    println!("{line}");
}

fn list<'c>(catalog: &'c Value, kind: &str) -> Result<&'c Vec<Value>, String> {
    catalog_v3_consumer::named_rows(catalog, kind).map_err(|error| error.to_string())
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the measurements print before the assertions"
)]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn the_quality_quantizations_of_a_real_archive_link_to_resource_types() -> TestResult {
    let archive = Archive::open_with_key(
        real_archive_path()?,
        Some(p4kx_testkit::real_archive_key()?),
    )?;
    let built = build_catalog(&archive, &CatalogOptions::new(SystemTime::now()))?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    let entries = list(&catalog, "qualityQuantizations")?;
    assert!(entries.len() >= MIN_RECORDS, "{} Records", entries.len());
    assert!(
        built
            .summary()
            .failures()
            .iter()
            .all(|failure| !failure.record_path.contains("qualityquantization")),
        "every Record is readable"
    );

    let mut commodities = HashSet::new();
    for choice in catalog_v3_consumer::presentations(&catalog)? {
        if let Some(guid) = catalog_v3_consumer::nullable_guid(
            choice
                .get("subtypeGuid")
                .ok_or("Commodity subtype carrier")?,
        )? {
            commodities.insert(guid);
        }
    }
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let mut unlinked = Vec::new();
    let mut linked: HashSet<String> = HashSet::new();
    let (mut links, mut exported) = (0_usize, 0_usize);
    for quantization in entries {
        let name = quantization["className"].as_str().ok_or("a className")?;
        let bands = quantization["bands"].as_array().ok_or("bands")?;
        assert!(!bands.is_empty(), "{name} has bands");
        let mut next = 0_i64;
        for band in bands {
            let (start, end) = (
                band["start"].as_i64().ok_or("start")?,
                band["end"].as_i64().ok_or("end")?,
            );
            assert_eq!(start, next, "{name}: the bands follow each other");
            assert!(start <= end, "{name}: a band ends after it starts");
            assert!(
                band["mappedValue"].as_i64().is_some(),
                "{name}: mappedValue"
            );
            next = end + 1;
        }
        assert_eq!(next, 1001, "{name}: the bands cover 0 to 1000");
        let types = quantization["resourceTypeGuids"]
            .as_array()
            .ok_or("resourceTypeGuids")?;
        if types.is_empty() {
            unlinked.push(name);
        }
        for text in types.iter().filter_map(Value::as_str) {
            let record = text
                .parse::<Guid>()
                .ok()
                .and_then(|guid| forge.record_by_guid(guid))
                .ok_or_else(|| format!("{name}: {text} is no Record"))?;
            assert_eq!(record.type_name()?, "ResourceType", "{name}: {text}");
            links += 1;
            linked.insert(text.to_owned());
            exported += usize::from(commodities.contains(text));
        }
    }
    print_line(&format!(
        "{} quantizations, {links} ResourceType links ({exported} to a commodity of the catalog), {} unlinked: {unlinked:?}",
        entries.len(),
        unlinked.len()
    ));
    assert!(links >= 40, "the ResourceTypes link: {links}");
    assert_eq!(exported, links, "every linked ResourceType is a commodity");
    assert!(
        built
            .summary()
            .failures()
            .iter()
            .all(|failure| !linked.contains(&failure.guid.to_string())),
        "no linked ResourceType is a failure"
    );
    let summary = built.summary().quantizations();
    assert_eq!(summary.unused, unlinked.len(), "the unused Records");
    assert_eq!(
        summary.unresolved_links, 0,
        "every link of a ResourceType reaches a quantization of the catalog"
    );
    Ok(())
}
