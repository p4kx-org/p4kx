// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Mineable elements and refining processes in the P4KX catalog, through the public API.

use crate::mining_fixture::{
    BEXALITE, DANGLING, FAST_CAREFUL, OTHER_RESOURCE, SHARED_RESOURCE, SLOW_WASTEFUL, UNLINKED,
    write_archive,
};

use std::error::Error;
use std::time::{Duration, UNIX_EPOCH};

use p4kx_export::{BuiltCatalog, CatalogOptions, Filter, Kind, build_catalog};
use p4kx_extraction::Archive;
use serde_json::{Value, json};

type TestResult = Result<(), Box<dyn Error>>;

fn build(filter: Option<&Filter>) -> Result<(BuiltCatalog, Value), Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let archive =
        Archive::open_with_key(write_archive(dir.path())?, Some(p4kx_testkit::test_key()))?;
    let mut options = CatalogOptions::new(UNIX_EPOCH + Duration::from_secs(1_792_000_000));
    options.filter = filter;
    let built = build_catalog(&archive, &options)?;
    let json = serde_json::from_slice(built.json())?;
    Ok((built, json))
}

fn guid(value: u128) -> String {
    let text = format!("{value:032x}");
    format!(
        "{}-{}-{}-{}-{}",
        &text[0..8],
        &text[8..12],
        &text[12..16],
        &text[16..20],
        &text[20..32]
    )
}

fn class_names<'a>(catalog: &'a Value, kind: &str) -> Vec<&'a str> {
    catalog[kind]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|entry| entry["className"].as_str())
        .collect()
}

#[test]
fn every_element_is_exported_even_when_several_share_a_resource_type() -> TestResult {
    let (built, catalog) = build(None)?;
    assert_eq!(
        class_names(&catalog, "mineableElements"),
        [
            "agricium_deposit",
            "bexalite_ore",
            "agricium_ore",
            "unlinked_ore",
            "dangling_ore"
        ],
        "ordered by GUID, the leftover counted as skipped"
    );
    let shared = guid(SHARED_RESOURCE);
    let linked: Vec<_> = catalog["mineableElements"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|entry| entry["resourceTypeGuid"] == shared.as_str())
        .map(|entry| (entry["className"].as_str(), entry["instability"].as_f64()))
        .collect();
    assert_eq!(
        linked,
        [
            (Some("agricium_deposit"), Some(425.0)),
            (Some("agricium_ore"), Some(350.0))
        ],
        "both elements keep their own instability; none is picked"
    );
    let kind = built.summary().kind(Kind::MineableElements);
    assert_eq!(
        kind.map(|kind| (kind.count, kind.skipped, kind.complete)),
        Some((5, 1, true)),
        "counts"
    );
    Ok(())
}

#[test]
fn a_null_or_dangling_resource_type_gives_a_null_guid_and_keeps_the_element() -> TestResult {
    let (_, catalog) = build(None)?;
    for (record, class, instability) in [
        (UNLINKED, "unlinked_ore", 12.0),
        (DANGLING, "dangling_ore", 13.0),
    ] {
        let entry = catalog["mineableElements"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|entry| entry["guid"] == guid(record).as_str())
            .ok_or("the element")?;
        assert_eq!(entry["className"], class);
        assert_eq!(entry["resourceTypeGuid"], Value::Null, "{class}");
        assert_eq!(entry["instability"], instability, "{class} keeps its facts");
    }
    Ok(())
}

#[test]
fn a_mineable_element_carries_the_plain_facts_of_its_record() -> TestResult {
    let (_, catalog) = build(None)?;
    let entry = catalog["mineableElements"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|entry| entry["guid"] == guid(BEXALITE).as_str())
        .ok_or("the element")?;
    assert_eq!(
        *entry,
        json!({
            "guid": guid(BEXALITE),
            "className": "bexalite_ore",
            "recordPath": "libs/foundry/records/mining/mineableelements/bexalite_ore",
            "resourceTypeGuid": guid(OTHER_RESOURCE),
            "instability": 0.0,
            "resistance": -0.5,
            "optimalWindowMidpoint": 0.5,
            "optimalWindowMidpointRandomness": 0.25,
            "optimalWindowThinness": 2.0,
            "explosionMultiplier": 4.0,
            "clusterFactor": 0.125,
        })
    );
    Ok(())
}

#[test]
fn a_refining_process_carries_its_name_speed_and_quality() -> TestResult {
    let (built, catalog) = build(None)?;
    assert_eq!(
        catalog["refiningProcesses"],
        json!([
            {
                "guid": guid(SLOW_WASTEFUL),
                "className": "processingtype-slowwasteful",
                "recordPath": "libs/foundry/records/refiningprocess/processingtype-slowwasteful",
                "processName": {
                    "key": "@refinery_ui_ProcessingType_SlowWasteful",
                    "en": null,
                    "de": null
                },
                "refiningSpeed": "Slow",
                "refiningQuality": "Wasteful",
            },
            {
                "guid": guid(FAST_CAREFUL),
                "className": "processingtype-fastcareful",
                "recordPath": "libs/foundry/records/refiningprocess/processingtype-fastcareful",
                "processName": {
                    "key": "@refinery_ui_ProcessingType_FastCareful",
                    "en": "Fast, Careful",
                    "de": null
                },
                "refiningSpeed": "Fast",
                "refiningQuality": "Careful",
            },
        ]),
        "ordered by GUID, the leftover left out"
    );
    let kind = built.summary().kind(Kind::RefiningProcesses);
    assert_eq!(
        kind.map(|kind| (kind.count, kind.skipped, kind.complete)),
        Some((2, 1, true)),
        "counts"
    );
    Ok(())
}

#[test]
fn a_filter_marks_both_kinds_partial_and_selects_by_record_path() -> TestResult {
    let filter = Filter::new("*/mineableelements/agricium_*");
    let (_, catalog) = build(Some(&filter))?;
    assert_eq!(
        class_names(&catalog, "mineableElements"),
        ["agricium_deposit", "agricium_ore"]
    );
    assert!(
        class_names(&catalog, "refiningProcesses").is_empty(),
        "filtered"
    );
    for kind in ["mineableElements", "refiningProcesses"] {
        assert_eq!(
            catalog["completeness"][kind],
            json!({"status": "partial", "filter": "*/mineableelements/agricium_*"}),
            "{kind}"
        );
    }
    Ok(())
}
