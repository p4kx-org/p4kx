// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Starmap objects and their world positions in the Basetool catalog, through the public API.

use crate::commodity_v3_fixture::{at, guid, text_row};
use crate::health_origin_assertions as proof;
use crate::starmap_fixture;

#[path = "starmap_definitions_contract.rs"]
mod definitions;
#[path = "starmap_objects_contract.rs"]
mod objects;
#[path = "starmap_placement_origins.rs"]
mod placement_origins;

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

fn object(catalog: &Value, record: u128) -> Result<&Value, Box<dyn Error>> {
    row(
        catalog,
        "/definitions/starmapObjects/rows",
        "/guid/value",
        record,
    )
}

fn placement(catalog: &Value, record: u128) -> Result<&Value, Box<dyn Error>> {
    row(
        catalog,
        "/placements/starmapPlacements/rows",
        "/objectGuid/value",
        record,
    )
}

fn row<'catalog>(
    catalog: &'catalog Value,
    rows: &str,
    guid_path: &str,
    record: u128,
) -> Result<&'catalog Value, Box<dyn Error>> {
    let wanted = serde_json::json!(guid(record));
    proof::rows(catalog, rows)?
        .iter()
        .find(|entry| entry.pointer(guid_path) == Some(&wanted))
        .ok_or_else(|| format!("missing {record:x} at {rows}").into())
}

fn position(catalog: &Value, record: u128) -> Result<Option<[f64; 3]>, Box<dyn Error>> {
    let value = at(placement(catalog, record)?, "/position/value")?;
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
            *at(placement(&catalog, record)?, "/placedUnderGuid/value")?,
            under.map_or(Value::Null, |parent| Value::String(guid(parent))),
            "placedUnderGuid of {record:x}"
        );
    }
    assert_eq!(
        at(object(&catalog, PLANET)?, "/parent/value/guid/value")?,
        guid(STAR).as_str(),
        "the Reference parent of the Record"
    );
    assert_eq!(
        at(object(&catalog, BELT)?, "/parent/value/guid/value")?,
        guid(STAR).as_str(),
        "parentGuid is the Record's, whatever the containers say"
    );
    Ok(())
}

#[test]
fn an_object_states_its_system_entity_name_type_and_name() -> TestResult {
    let (_, catalog) = build(None)?;
    let planet = object(&catalog, PLANET)?;
    let placed = placement(&catalog, PLANET)?;
    assert_eq!(
        at(placed, "/system/value")?,
        "stanton",
        "the folder of the system"
    );
    assert_eq!(
        at(placed, "/entityName/value")?,
        "planet_hurston",
        "the Child's entity"
    );
    for definition in [planet, object(&catalog, MOON)?] {
        assert_eq!(
            at(definition, "/type/value/guid/value")?,
            guid(PLANET_TYPE).as_str(),
            "both objects link the same physical type"
        );
    }
    let kind = row(
        &catalog,
        "/definitions/starmapTypes/rows",
        "/guid/value",
        PLANET_TYPE,
    )?;
    assert_eq!(
        at(kind, "/name/value")?,
        "Planet",
        "the authored name of the type Record"
    );
    let name = text_row(&catalog, "gameTexts", at(planet, "/name/value")?)?;
    assert_eq!(
        at(name, "/key/value")?,
        "@starmap_Hurston",
        "original Locale key"
    );
    assert_eq!(
        at(name, "/translations/value/en/displayText/value")?,
        "Hurston",
        "the resolved display name"
    );
    assert_eq!(
        at(planet, "/className/value")?,
        "Hurston",
        "the Record name without its type"
    );
    assert_eq!(
        at(planet, "/hideInStarmap/value")?,
        false,
        "the flag of the Record"
    );
    verify_placement_evidence(&catalog, placed)
}

fn verify_placement_evidence(catalog: &Value, placed: &Value) -> TestResult {
    assert_eq!(
        at(object(catalog, BELT)?, "/hideInStarmap/value")?,
        true,
        "hidden"
    );
    assert!(
        position(catalog, BELT)?.is_some(),
        "hidden object still has placement"
    );
    assert_eq!(
        at(placed, "/position/containsEncryptedSource")?,
        &Value::Null,
        "the inner ZIP has no supported encryption declaration"
    );
    assert_eq!(
        at(placed, "/position/sourceSetComplete")?,
        true,
        "the selected XML position retains every actual contributor"
    );
    Ok(())
}

#[test]
fn an_object_no_container_places_has_no_position_and_a_malformed_child_places_none() -> TestResult {
    let (_, catalog) = build(None)?;
    for record in [ORPHAN, BAD] {
        let entry = placement(&catalog, record)?;
        for field in ["position", "system", "entityName", "placedUnderGuid"] {
            proof::cell(entry, field, "absent")?;
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
    assert_eq!(
        at(placement(&catalog, JUMP)?, "/system/value")?,
        "stanton",
        "system"
    );
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
    let guids: Vec<&str> = proof::rows(&catalog, "/definitions/starmapObjects/rows")?
        .iter()
        .filter_map(|entry| entry.pointer("/guid/value").and_then(Value::as_str))
        .collect();
    let mut sorted = guids.clone();
    sorted.sort_unstable();
    assert_eq!(guids, sorted, "ordered by the text of the GUID");

    let filter = Filter::new("*/hurston*");
    let (built, filtered) = build(Some(&filter))?;
    assert_eq!(
        proof::rows(&filtered, "/definitions/starmapObjects/rows")?.len(),
        1,
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
    let (_, first) = build(None)?;
    let (_, second) = build(None)?;
    for path in [
        "/definitions/starmapObjects",
        "/placements/starmapPlacements",
    ] {
        assert_eq!(
            at(&first, path)?,
            at(&second, path)?,
            "deterministic {path}"
        );
    }
    let text = serde_json::to_string(at(&first, "/placements/starmapPlacements")?)?;
    assert!(
        !text.contains("-0.0"),
        "no negative zero in computed coordinates"
    );
    Ok(())
}

#[path = "starmap_container_instance_contract.rs"]
mod container_instances;
