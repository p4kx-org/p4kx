// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Starmap objects and their world positions in the Basetool catalog, through the public API.

use crate::starmap_fixture;

use std::error::Error;
use std::time::{Duration, UNIX_EPOCH};

use p4kx_export::{
    BuiltCatalog, CatalogOptions, CatalogPublicationMode, CraftingWarning, Filter, Kind,
    build_catalog,
};
use p4kx_extraction::Archive;
use serde_json::Value;
use starmap_fixture::{
    BAD, BELT, DOUBLE, JUMP, MARKER_TEST, MOON, ORPHAN, PLANET, PLANET_TYPE, STAR, STATION,
    write_archive,
};

type TestResult = Result<(), Box<dyn Error>>;

const EXPORTED_AT: Duration = Duration::from_secs(1_792_000_000);

fn build(filter: Option<&Filter>) -> Result<(BuiltCatalog, Value), Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let archive =
        Archive::open_with_key(write_archive(dir.path())?, Some(p4kx_testkit::test_key()))?;
    let mut options = CatalogOptions::new(UNIX_EPOCH + EXPORTED_AT);
    options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
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

fn object(catalog: &Value, record: u128) -> Result<&Value, Box<dyn Error>> {
    let wanted = guid(record);
    catalog["starmapObjects"]
        .as_array()
        .ok_or("starmapObjects")?
        .iter()
        .find(|entry| entry["guid"] == wanted.as_str())
        .ok_or_else(|| format!("the object {wanted}").into())
}

fn position(catalog: &Value, record: u128) -> Result<Option<[f64; 3]>, Box<dyn Error>> {
    let value = &object(catalog, record)?["position"];
    if value.is_null() {
        return Ok(None);
    }
    let parts = value.as_array().ok_or("position")?;
    let mut at = [0.0; 3];
    for (slot, part) in at.iter_mut().zip(parts) {
        *slot = part.as_f64().ok_or("coordinate")?;
    }
    Ok(Some(at))
}

#[test]
fn a_position_is_the_sum_of_the_positions_rotated_by_every_container_above() -> TestResult {
    let (_, catalog) = build(None)?;
    assert_eq!(
        position(&catalog, PLANET)?,
        Some([1000.0, 0.0, 0.0]),
        "a child of the system"
    );
    assert_eq!(
        position(&catalog, MOON)?,
        Some([998.0, 0.0, 0.0]),
        "a child inside the planet is turned a quarter about z by it: (0, 2, 0) becomes (-2, 0, 0)"
    );
    assert_eq!(
        position(&catalog, STATION)?,
        Some([1000.0, 10.0, 0.0]),
        "a child of an external container: (10, 0, 0) becomes (0, 10, 0)"
    );
    assert_eq!(
        position(&catalog, BELT)?,
        Some([100.0, 1.0, 0.0]),
        "a child below a container without a starmap record"
    );
    assert_eq!(
        position(&catalog, JUMP)?,
        Some([0.0, 0.0, 50.0]),
        "a jump point without rotation"
    );
    Ok(())
}

#[test]
fn the_hierarchy_names_the_nearest_record_above() -> TestResult {
    let (_, catalog) = build(None)?;
    for (record, under) in [(MOON, Some(PLANET)), (STATION, Some(PLANET)), (BELT, None)] {
        assert_eq!(
            object(&catalog, record)?["placedUnderGuid"],
            under.map_or(Value::Null, |parent| Value::String(guid(parent))),
            "placedUnderGuid of {record:x}"
        );
    }
    assert_eq!(
        object(&catalog, PLANET)?["parentGuid"],
        guid(STAR).as_str(),
        "the Reference parent of the Record"
    );
    assert_eq!(
        object(&catalog, BELT)?["parentGuid"],
        guid(STAR).as_str(),
        "parentGuid is the Record's, whatever the containers say"
    );
    Ok(())
}

#[test]
fn an_object_states_its_system_entity_name_type_and_name() -> TestResult {
    let (_, catalog) = build(None)?;
    let planet = object(&catalog, PLANET)?;
    assert_eq!(planet["system"], "stanton", "the folder of the system");
    assert_eq!(planet["entityName"], "planet_hurston", "the Child's entity");
    assert_eq!(planet["type"], "Planet", "the type Record without its type");
    assert_eq!(planet["typeGuid"], guid(PLANET_TYPE).as_str(), "type guid");
    assert_eq!(
        planet["name"],
        serde_json::json!({"key": "@starmap_Hurston", "en": "Hurston", "de": null}),
        "the localised name"
    );
    assert_eq!(
        planet["className"], "Hurston",
        "the Record name without its type"
    );
    assert_eq!(planet["hideInStarmap"], false, "the flag of the Record");
    assert_eq!(object(&catalog, BELT)?["hideInStarmap"], true, "hidden");
    Ok(())
}

#[test]
fn an_object_no_container_places_has_no_position_and_a_malformed_child_places_none() -> TestResult {
    let (_, catalog) = build(None)?;
    for record in [ORPHAN, BAD] {
        let entry = object(&catalog, record)?;
        for field in ["position", "system", "entityName", "placedUnderGuid"] {
            assert!(entry[field].is_null(), "{field} of {record:x}");
        }
    }
    Ok(())
}

#[test]
fn a_record_placed_twice_keeps_one_place_but_none_when_the_places_are_apart() -> TestResult {
    let (_, catalog) = build(None)?;
    assert_eq!(
        position(&catalog, JUMP)?,
        Some([0.0, 0.0, 50.0]),
        "terra places it half a metre away: one place, the first is kept"
    );
    assert_eq!(object(&catalog, JUMP)?["system"], "stanton", "system");
    assert_eq!(
        position(&catalog, DOUBLE)?,
        None,
        "stanton and terra place it two metres apart: no position"
    );
    Ok(())
}

#[test]
fn the_summary_counts_the_placed_objects_and_what_the_containers_lacked() -> TestResult {
    let (built, _) = build(None)?;
    let summary = built.summary().starmap();
    assert_eq!(
        (summary.objects, summary.placed, summary.unplaced),
        (9, 5, 4),
        "objects, placed, unplaced"
    );
    assert_eq!(summary.systems, 3, "broken, stanton and terra");
    assert_eq!(summary.ambiguous, 1, "the double is placed apart");
    assert_eq!(
        summary.malformed_children, 1,
        "the child with the position a,b,c"
    );
    assert_eq!(
        summary.unreadable_containers, 1,
        "the system that is no container"
    );
    assert_eq!(
        summary.missing_containers, 1,
        "the external container the Archive lacks"
    );
    assert!(
        built
            .summary()
            .warnings()
            .contains(&CraftingWarning::UnplacedStarmapContainers(2)),
        "a warning for the unreadable container and the malformed child, not the missing one"
    );
    Ok(())
}

#[test]
fn a_development_leftover_is_skipped_and_the_kind_is_counted() -> TestResult {
    let (built, catalog) = build(None)?;
    let kind = built
        .summary()
        .kind(Kind::StarmapObjects)
        .ok_or("the kind")?;
    assert_eq!(
        (kind.count, kind.skipped, kind.complete),
        (9, 1, true),
        "kind"
    );
    assert!(
        object(&catalog, MARKER_TEST).is_err(),
        "Marker_Test holds the word test"
    );
    assert_eq!(
        catalog["completeness"]["starmapObjects"],
        serde_json::json!({"status": "complete"}),
        "complete"
    );
    Ok(())
}

#[test]
fn the_objects_are_ordered_by_guid_and_a_filter_makes_the_kind_partial() -> TestResult {
    let (_, catalog) = build(None)?;
    let guids: Vec<&str> = catalog["starmapObjects"]
        .as_array()
        .ok_or("starmapObjects")?
        .iter()
        .filter_map(|entry| entry["guid"].as_str())
        .collect();
    let mut sorted = guids.clone();
    sorted.sort_unstable();
    assert_eq!(guids, sorted, "ordered by the text of the GUID");

    let filter = Filter::new("*/hurston*");
    let (built, filtered) = build(Some(&filter))?;
    assert_eq!(
        filtered["starmapObjects"].as_array().map(Vec::len),
        Some(1),
        "only Hurston"
    );
    assert_eq!(
        filtered["completeness"]["starmapObjects"]["status"], "partial",
        "partial"
    );
    assert_eq!(
        position(&filtered, PLANET)?,
        Some([1000.0, 0.0, 0.0]),
        "a position does not depend on the Filter"
    );
    assert_eq!(built.summary().starmap().objects, 1, "summary");
    Ok(())
}

#[test]
fn a_position_never_prints_a_negative_zero_and_two_builds_are_identical() -> TestResult {
    let (first, first_json) = build(None)?;
    let (_, second_json) = build(None)?;
    assert_eq!(
        first_json["starmapObjects"], second_json["starmapObjects"],
        "deterministic"
    );
    let text = std::str::from_utf8(first.json())?;
    assert!(!text.contains("-0.0"), "no negative zero");
    Ok(())
}
