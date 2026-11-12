// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Starmap objects and their world positions in the Basetool catalog of a real Archive; runs with
//! `--ignored` when `P4KX_DATA_P4K` names one. The diagnostics print before any assertion.

use std::collections::BTreeMap;
use std::error::Error;
use std::time::SystemTime;

use p4kx_export::{BuiltCatalog, CatalogOptions, Kind, build_catalog};
use p4kx_extraction::Archive;
use p4kx_testkit::{real_archive_key, real_archive_path};
use serde_json::Value;

use crate::catalog_v3_consumer as consumer;
use crate::real_archive_canonical_texts::{TextDomain, TextIndex, TextReferenceShape};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

const MIN_OBJECTS: usize = 100;
const PLANET: &str = "Hurston";
const STATION: &str = "Everus Harbor";
const EXAMPLES: usize = 10;
const MAX_MISSING: usize = 5;
const NEAREST_PLANET_FROM_STAR: f64 = 1.0e9;
const FARTHEST_PLANET_FROM_STAR: f64 = 1.0e11;
const NEAREST_STATION_FROM_PLANET: f64 = 1.0e5;
const FARTHEST_STATION_FROM_PLANET: f64 = 1.0e8;

fn position(object: &Value) -> Option<[f64; 3]> {
    let parts = object.pointer("/position/value")?.as_array()?;
    Some([
        parts.first()?.as_f64()?,
        parts.get(1)?.as_f64()?,
        parts.get(2)?.as_f64()?,
    ])
}

fn text<'v>(object: &'v Value, pointer: &str) -> &'v str {
    object
        .pointer(pointer)
        .and_then(Value::as_str)
        .unwrap_or("(none)")
}

fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    a.iter()
        .zip(b)
        .map(|(a, b)| (a - b) * (a - b))
        .sum::<f64>()
        .sqrt()
}

struct StarmapRow<'catalog> {
    definition: &'catalog Value,
    placement: &'catalog Value,
    type_name: &'catalog str,
    name: Option<&'catalog str>,
}

fn joined_rows(catalog: &Value) -> TestResult<Vec<StarmapRow<'_>>> {
    let objects = consumer::rows(catalog, Kind::StarmapObjects)?;
    let placements = catalog
        .pointer("/placements/starmapPlacements/rows")
        .and_then(Value::as_array)
        .ok_or("starmap placement rows")?;
    let types = consumer::rows(catalog, Kind::StarmapTypes)?;
    let placements_by_record = by_index(placements, "/objectRecordIndex/value")?;
    let types_by_record = by_index(types, "/recordIndex/value")?;
    let texts = TextIndex::new(catalog)?;
    let mut joined = Vec::new();
    for definition in objects {
        let index = definition
            .pointer("/recordIndex/value")
            .and_then(Value::as_u64)
            .ok_or("starmap definition index")?;
        let &placement = placements_by_record
            .get(&index)
            .ok_or("placement by physical Record")?;
        assert_eq!(
            placement.pointer("/objectGuid/value"),
            definition.pointer("/guid/value"),
            "placement points to the actual physical owner"
        );
        let type_name = definition
            .pointer("/type/value/targetRecordIndex/value")
            .and_then(Value::as_u64)
            .and_then(|index| types_by_record.get(&index))
            .map_or("(none)", |kind| {
                kind.pointer("/name/value")
                    .and_then(Value::as_str)
                    .filter(|label| !label.starts_with('@'))
                    .unwrap_or_else(|| text(kind, "/className/value"))
            });
        let name = texts
            .reference(
                definition.get("name").ok_or("object name cell")?,
                TextDomain::GameText,
                TextReferenceShape::Required,
            )?
            .map(|name| name.shown("en"))
            .transpose()?
            .and_then(Value::as_str);
        joined.push(StarmapRow {
            definition,
            placement,
            type_name,
            name,
        });
    }
    assert_eq!(
        joined.len(),
        placements.len(),
        "one placement row per selected object"
    );
    Ok(joined)
}

fn by_index<'catalog>(
    rows: &'catalog [Value],
    pointer: &str,
) -> TestResult<BTreeMap<u64, &'catalog Value>> {
    let mut by_record = BTreeMap::new();
    for row in rows {
        let index = row
            .pointer(pointer)
            .and_then(Value::as_u64)
            .ok_or("physical Record index")?;
        if by_record.insert(index, row).is_some() {
            return Err("duplicate physical Record index".into());
        }
    }
    Ok(by_record)
}

fn named<'catalog>(
    objects: &'catalog [StarmapRow<'catalog>],
    name: &str,
) -> Option<&'catalog Value> {
    objects
        .iter()
        .find(|object| object.name == Some(name) && position(object.placement).is_some())
        .map(|object| object.placement)
}

#[expect(
    clippy::print_stdout,
    reason = "the diagnostics are the result of the verification"
)]
fn print_lines(title: &str, lines: &[String]) {
    println!("== {title} ({})", lines.len());
    for line in lines {
        println!("   {line}");
    }
}

fn print_unplaced(objects: &[StarmapRow<'_>]) {
    let mut by_type: BTreeMap<&str, Vec<&Value>> = BTreeMap::new();
    for object in objects
        .iter()
        .filter(|object| position(object.placement).is_none())
    {
        by_type
            .entry(object.type_name)
            .or_default()
            .push(object.definition);
    }
    let mut lines = Vec::new();
    for (kind, unplaced) in &by_type {
        let hidden = unplaced
            .iter()
            .filter(|object| object.pointer("/hideInStarmap/value") == Some(&Value::Bool(true)))
            .count();
        lines.push(format!(
            "{kind}: {} unplaced, {hidden} hidden",
            unplaced.len()
        ));
        for object in unplaced.iter().take(EXAMPLES) {
            lines.push(format!(
                "    {} | {} | hidden {} | parent {}",
                text(object, "/className/value"),
                text(object, "/recordPath/value"),
                object
                    .pointer("/hideInStarmap/value")
                    .unwrap_or(&Value::Null),
                text(object, "/parent/value/guid/value")
            ));
        }
    }
    print_lines("unplaced objects by type", &lines);
}

fn print_types(objects: &[StarmapRow<'_>]) {
    let mut counts: BTreeMap<&str, (usize, usize)> = BTreeMap::new();
    for object in objects {
        let slot = counts.entry(object.type_name).or_default();
        slot.0 += 1;
        slot.1 += usize::from(position(object.placement).is_some());
    }
    let lines: Vec<String> = counts
        .iter()
        .map(|(kind, (all, with))| format!("{kind}: {all} objects, {with} placed"))
        .collect();
    print_lines("objects by type", &lines);
}

#[expect(
    clippy::print_stdout,
    reason = "the diagnostics are the result of the verification"
)]
fn report(built: &BuiltCatalog, catalog: &Value, objects: &[StarmapRow<'_>]) -> TestResult {
    let seen = built.summary().starmap_report();
    println!("== summary {:?}", built.summary().starmap());
    println!(
        "== deepest external chain {} (limit 32), reading the system containers took {:?}",
        seen.deepest_external(),
        seen.elapsed()
    );
    print_lines("containers read", seen.containers());
    print_lines("containers that failed", seen.issues());
    print_lines("records placed twice", seen.repeated());
    print_lines("containers no Child names", seen.unreferenced());
    print_types(objects);
    print_unplaced(objects);
    println!(
        "== starmapObjects: {} bytes of JSON",
        catalog
            .pointer("/definitions/starmapObjects")
            .ok_or("starmap definition table")?
            .to_string()
            .len()
    );
    for name in [PLANET, STATION] {
        let found = named(objects, name);
        println!(
            "== {name}: {:?} system {:?} under {:?}",
            found.and_then(position),
            found.map(|object| text(object, "/system/value")),
            found.map(|object| text(object, "/placedUnderGuid/value"))
        );
    }
    Ok(())
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn the_starmap_of_a_real_archive_places_a_planet_and_a_station() -> TestResult {
    let archive = Archive::open_with_key(real_archive_path()?, Some(real_archive_key()?))?;
    let built = build_catalog(&archive, &CatalogOptions::new(SystemTime::now()))?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    let objects = joined_rows(&catalog)?;
    report(&built, &catalog, &objects)?;

    let summary = built.summary().starmap();
    assert!(objects.len() >= MIN_OBJECTS, "{} objects", objects.len());
    assert_eq!(summary.objects, objects.len(), "the summary counts objects");
    assert_eq!(
        summary.placed + summary.unplaced,
        summary.objects,
        "placed and unplaced add up"
    );
    assert!(summary.systems >= 1, "{summary:?}");
    assert_eq!(summary.unreadable_containers, 0, "{summary:?}");
    assert!(summary.missing_containers <= MAX_MISSING, "{summary:?}");
    assert_eq!(summary.malformed_children, 0, "{summary:?}");
    assert_eq!(
        built.summary().kind(Kind::StarmapObjects).map(|k| k.count),
        Some(objects.len()),
        "the kind summary"
    );
    let guids = objects
        .iter()
        .map(|object| consumer::identity(object.definition, Kind::StarmapObjects))
        .collect::<Result<Vec<_>, _>>()?;
    assert!(guids.is_sorted(), "ordered by GUID");

    let planet = named(&objects, PLANET).ok_or("a placed Hurston")?;
    let station = named(&objects, STATION).ok_or("a placed Everus Harbor")?;
    let planet_at = position(planet).ok_or("planet position")?;
    let star_distance = distance(planet_at, [0.0; 3]);
    assert!(
        (NEAREST_PLANET_FROM_STAR..FARTHEST_PLANET_FROM_STAR).contains(&star_distance),
        "{PLANET} is {star_distance} m from the origin of the system"
    );
    assert_eq!(
        text(planet, "/system/value"),
        "stanton",
        "the system of {PLANET}"
    );
    let planet_distance = distance(planet_at, position(station).ok_or("station position")?);
    assert!(
        (NEAREST_STATION_FROM_PLANET..FARTHEST_STATION_FROM_PLANET).contains(&planet_distance),
        "{STATION} is {planet_distance} m from {PLANET}"
    );
    Ok(())
}
