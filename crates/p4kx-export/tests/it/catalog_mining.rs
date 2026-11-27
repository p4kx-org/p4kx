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
    let built = build_catalog(&archive, options.with_full_provenance())?;
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
    let rows = if kind == "refiningProcesses" {
        catalog.pointer("/definitions/refiningProcesses/rows")
    } else {
        catalog.get(kind)
    };
    rows.and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.pointer("/className/value").and_then(Value::as_str))
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
        .filter(|entry| {
            entry
                .pointer("/resourceType/value/guid/value")
                .and_then(Value::as_str)
                == Some(shared.as_str())
        })
        .map(|entry| {
            (
                entry["className"]["value"].as_str(),
                entry["instability"]["value"]["finite"].as_f64(),
            )
        })
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
fn a_null_and_dangling_resource_link_keep_distinct_states() -> TestResult {
    let (_, catalog) = build(None)?;
    for (record, class, instability) in [
        (UNLINKED, "unlinked_ore", 12.0),
        (DANGLING, "dangling_ore", 13.0),
    ] {
        let entry = catalog["mineableElements"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|entry| entry["guid"]["value"] == guid(record).as_str())
            .ok_or("the element")?;
        assert_eq!(entry["className"]["value"], class, "direct class name");
        let reference = &entry["resourceType"];
        if record == UNLINKED {
            assert_eq!(reference["state"], "null", "authored null reference");
            assert!(reference["value"].is_null(), "state carries no value");
        } else {
            assert_eq!(reference["state"], "present", "the stored GUID survives");
            assert_eq!(reference["value"]["status"], "dangling", "missing target");
            assert!(
                reference["value"]["guid"]["value"].is_string(),
                "exact authored GUID"
            );
        }
        assert_eq!(
            entry["instability"]["value"]["finite"], instability,
            "{class} keeps its facts"
        );
    }
    Ok(())
}

#[test]
fn a_mineable_element_carries_direct_sourced_facts() -> TestResult {
    let (_, catalog) = build(None)?;
    let entry = catalog["mineableElements"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|entry| {
            entry.pointer("/guid/value").and_then(Value::as_str) == Some(guid(BEXALITE).as_str())
        })
        .ok_or("the element")?;
    assert_eq!(entry["className"]["value"], "bexalite_ore", "sourced name");
    assert_eq!(
        entry["recordPath"]["value"], "libs/foundry/records/mining/mineableelements/bexalite_ore",
        "sourced path"
    );
    assert_eq!(
        entry["resourceType"]["value"]["guid"]["value"],
        guid(OTHER_RESOURCE),
        "sourced link"
    );
    for duplicate in ["resourceTypeGuid", "sourceValues"] {
        assert!(
            entry.get(duplicate).is_none(),
            "duplicate wire value {duplicate}"
        );
    }
    let numeric = [
        ("instability", 0.0_f32),
        ("resistance", -0.5),
        ("optimalWindowMidpoint", 0.5),
        ("optimalWindowMidpointRandomness", 0.25),
        ("optimalWindowThinness", 2.0),
        ("explosionMultiplier", 4.0),
        ("clusterFactor", 0.125),
    ];
    for (field, finite) in numeric {
        let cell = &entry[field];
        assert_eq!(cell["state"], "present", "{field} source is readable");
        assert!(
            cell["containsEncryptedSource"].is_boolean(),
            "{field} encryption marker"
        );
        assert_eq!(cell["sourceSetComplete"], true, "{field} contributor set");
        assert!(
            cell["sourceUsesRefs"]
                .as_array()
                .is_some_and(|refs| !refs.is_empty()),
            "{field} source references"
        );
        assert_eq!(
            cell["value"]["finite"], finite,
            "{field} numeric presentation"
        );
        assert_eq!(
            cell["value"]["rawBits"],
            format!("{:08x}", finite.to_bits()),
            "{field} original Single"
        );
    }
    Ok(())
}

#[test]
fn a_refining_process_carries_its_name_speed_and_quality() -> TestResult {
    let (built, catalog) = build(None)?;
    let rows = catalog
        .pointer("/definitions/refiningProcesses/rows")
        .and_then(Value::as_array)
        .ok_or("canonical refining rows")?;
    let [slow, fast] = rows.as_slice() else {
        return Err("two selected refining processes".into());
    };
    for (row, identity, class, speed, quality, key, english) in [
        (
            slow,
            SLOW_WASTEFUL,
            "processingtype-slowwasteful",
            "Slow",
            "Wasteful",
            "@refinery_ui_ProcessingType_SlowWasteful",
            None,
        ),
        (
            fast,
            FAST_CAREFUL,
            "processingtype-fastcareful",
            "Fast",
            "Careful",
            "@refinery_ui_ProcessingType_FastCareful",
            Some("Fast, Careful"),
        ),
    ] {
        verify_name(&catalog, row, "processName", key, english)?;
        verify_refining_fields(row, identity, class, speed, quality);
    }
    assert!(
        catalog.get("refiningProcesses").is_none(),
        "no duplicate legacy root view"
    );
    let kind = built.summary().kind(Kind::RefiningProcesses);
    assert_eq!(
        kind.map(|kind| (kind.count, kind.skipped, kind.complete)),
        Some((2, 1, true)),
        "counts"
    );
    Ok(())
}

fn verify_refining_fields(row: &Value, identity: u128, class: &str, speed: &str, quality: &str) {
    assert_eq!(
        row.pointer("/guid/value"),
        Some(&json!(guid(identity))),
        "original GUID"
    );
    assert_eq!(
        row.pointer("/className/value"),
        Some(&json!(class)),
        "original class"
    );
    assert_eq!(
        row.pointer("/refiningSpeed/value"),
        Some(&json!(speed)),
        "authored speed label"
    );
    assert_eq!(
        row.pointer("/refiningQuality/value"),
        Some(&json!(quality)),
        "authored quality label"
    );
    assert!(row.get("fields").is_some(), "complete original owner arena");
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

pub(super) fn verify_name(
    catalog: &Value,
    owner: &Value,
    field: &str,
    key: &str,
    english: Option<&str>,
) -> TestResult {
    use crate::health_origin_assertions as proof;

    let id = proof::at(owner, &format!("/{field}/value"))?;
    proof::present(owner, field, id, Some(true))?;
    let row = proof::rows(catalog, "/texts/gameTexts")?
        .iter()
        .find(|text| text.pointer("/id/value") == Some(id))
        .ok_or("protected mining text target")?;
    assert_eq!(
        row.pointer("/key/value"),
        Some(&json!(key)),
        "the original mining text key"
    );
    assert_eq!(
        row.pointer("/translations/value/en/displayText/value"),
        Some(&json!(english)),
        "English mining display text"
    );
    assert!(
        row.pointer("/translations/value/de").is_none(),
        "German was not loaded"
    );
    Ok(())
}

#[path = "mining_process_query_suite.rs"]
mod processes;
