// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::{Path, PathBuf};

use p4kx_testkit::{ArchiveBuilder, TestElement, TestEntry, TestRecord};
use serde_json::Value;

use super::{LOCATIONS, PLANET, TestResult, build, shop};
use crate::health_origin_assertions as health;
use crate::shops_fixture::{self, REGISTRY};
use crate::starmap_fixture;

#[path = "shop_starmap_batch.rs"]
mod batch;
#[path = "shop_starmap_companions.rs"]
mod companions;

const DISTINCT: u128 = 0x8100_0000_0000_4000_8000_0000_0000_0001;
const ROOT: &str = "Data\\ObjectContainers\\PU\\system\\stanton\\stantonsystem.socpak";

const OTHER: &str = "Data/ObjectContainers/PU/system/stanton/other.socpak";

#[derive(Clone, Copy)]
enum Route {
    Plain,
    UnavailableGrid,
    UnreadableChild,
}

fn archive(
    dir: &Path,
    alias: Option<u128>,
    unreadable: bool,
    route: Route,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let mut database = starmap_fixture::database();
    if let Some(guid) = alias {
        let candidate = TestRecord::new(
            guid,
            "StarMapObject",
            "StarMapObject.hURSToN",
            "libs/foundry/records/starmap/pu/system/stanton/hurston_alias.xml",
        );
        let candidate = if unreadable {
            candidate.with_instance_index(u16::MAX)
        } else {
            candidate
        };
        database = database.record(candidate);
    }
    let mut children = vec![starmap_fixture::child(
        "100,0,0",
        Some(starmap_fixture::PLANET),
        "planet_hurston",
    )];
    let mut root = TestElement::new("ObjectContainer");
    if matches!(route, Route::UnavailableGrid) {
        root = root
            .attribute("xmlns", "urn:fixture")
            .attribute("gridRadius", "4");
    }
    if matches!(route, Route::UnreadableChild) {
        children.push(
            starmap_fixture::child("0,0,0", None, "unknown_route")
                .attribute("external", "1")
                .attribute("name", OTHER),
        );
    }
    let root = root.child(starmap_fixture::children(children));
    let path = dir.join("Data.p4k");
    let mut builder = ArchiveBuilder::v2()
        .entry(TestEntry::stored("Data\\Game2.dcb", database.build()).with_encryption_field(0))
        .entry(TestEntry::stored(REGISTRY, shops_fixture::registry()).with_encryption_field(0))
        .entry(
            TestEntry::stored(
                ROOT,
                starmap_fixture::socpak("stantonsystem.xml", root.to_xml()),
            )
            .encrypted(),
        );
    if matches!(route, Route::UnreadableChild) {
        builder = builder.entry(TestEntry::stored(OTHER, "unreadable container"));
    }
    builder.write_to(&path)?;
    Ok(path)
}

fn count_field(
    catalog: &Value,
    cell: &Value,
    field: &str,
    role: &str,
) -> Result<usize, Box<dyn std::error::Error>> {
    Ok(health::uses(catalog, cell)?
        .iter()
        .filter(|used| {
            used.pointer("/location/field").and_then(Value::as_str) == Some(field)
                && used["role"] == role
        })
        .count())
}

#[test]
fn class_only_and_placed_entity_aliases_keep_exact_origins() -> TestResult {
    let (_, catalog) = build(|dir| archive(dir, None, false, Route::Plain), None)?;
    let class = health::present(
        shop(&catalog, LOCATIONS[0])?,
        "starmapObjectGuid",
        &Value::from(PLANET),
        Some(false),
    )?;
    assert_eq!(
        count_field(&catalog, class, "guid", "payload")?,
        1,
        "the winner has its original physical GUID"
    );
    assert_eq!(
        count_field(&catalog, class, "recordName", "conversion")?,
        1,
        "the original Record header supplied the matching class"
    );
    assert_eq!(
        count_field(&catalog, class, "@SuperName", "conversion")?,
        1,
        "the Shop registry supplied the location selector"
    );
    assert_eq!(
        count_field(&catalog, class, "@entityName", "payload")?,
        0,
        "another name on the same object did not select this match"
    );

    let placed = health::present(
        shop(&catalog, LOCATIONS[1])?,
        "starmapObjectGuid",
        &Value::from(PLANET),
        Some(true),
    )?;
    assert_eq!(
        count_field(&catalog, placed, "guid", "payload")?,
        1,
        "the placement still returns a physical Record GUID"
    );
    assert_eq!(
        count_field(&catalog, placed, "recordName", "conversion")?,
        0,
        "the class name did not select the XML alias"
    );
    assert_eq!(
        count_field(&catalog, placed, "@entityName", "payload")?,
        1,
        "the original encrypted Child name supplied the match"
    );
    assert_eq!(
        count_field(&catalog, placed, "@starMapRecord", "selector")?,
        1,
        "the Child binds the matching name to the original GUID"
    );
    health::closure(&catalog)
}

#[test]
fn a_missing_location_match_keeps_its_lookup_input() -> TestResult {
    let (_, catalog) = build(|dir| archive(dir, None, false, Route::Plain), None)?;
    let missing = health::cell(shop(&catalog, LOCATIONS[5])?, "starmapObjectGuid", "absent")?;
    assert_eq!(
        missing["containsEncryptedSource"],
        Value::Null,
        "a no-match result cannot certify its negative source scope"
    );
    assert_eq!(
        missing["sourceSetComplete"], false,
        "the referred location lacks a proven complete negative survey"
    );
    assert_eq!(
        count_field(&catalog, missing, "@SuperName", "conversion")?,
        1,
        "the original attempted lookup selector remains"
    );
    health::closure(&catalog)
}

#[test]
fn equal_nominal_guids_keep_the_winner_and_distinct_guids_keep_the_conflict() -> TestResult {
    for (extra, state) in [(starmap_fixture::PLANET, "present"), (DISTINCT, "invalid")] {
        let (_, catalog) = build(|dir| archive(dir, Some(extra), false, Route::Plain), None)?;
        let cell = health::cell(shop(&catalog, LOCATIONS[0])?, "starmapObjectGuid", state)?;
        if state == "present" {
            assert_eq!(
                cell["value"], PLANET,
                "physical aliases of the same nominal GUID keep the original winner"
            );
        }
        assert_eq!(
            count_field(&catalog, cell, "guid", "payload")?,
            2,
            "both physically matching GUID operands survive"
        );
        assert_eq!(
            count_field(&catalog, cell, "recordName", "conversion")?,
            2,
            "both original case-folded class selectors survive"
        );
        assert_eq!(
            cell["containsEncryptedSource"], false,
            "the unrelated encrypted placement is not a class-name input"
        );
        health::closure(&catalog)?;
    }
    Ok(())
}

#[test]
fn an_unreadable_competitor_keeps_found_links_incomplete() -> TestResult {
    let (_, catalog) = build(|dir| archive(dir, Some(DISTINCT), true, Route::Plain), None)?;
    for (location, encrypted) in [
        (LOCATIONS[0], Value::Null),
        (LOCATIONS[1], Value::Bool(true)),
    ] {
        let cell = health::cell(shop(&catalog, location)?, "starmapObjectGuid", "present")?;
        assert_eq!(
            cell["value"], PLANET,
            "the observed match remains available"
        );
        assert_eq!(
            cell["containsEncryptedSource"], encrypted,
            "known encryption survives incomplete selection evidence"
        );
        assert_eq!(
            cell["sourceSetComplete"], false,
            "an unreadable candidate keeps both name lookups incomplete"
        );
    }
    let absent = health::cell(
        shop(&catalog, LOCATIONS[5])?,
        "starmapObjectGuid",
        "unreadable",
    )?;
    assert_eq!(
        absent["sourceSetComplete"], false,
        "an incomplete inventory cannot establish absence for a supplied name"
    );
    health::closure(&catalog)
}

#[test]
fn unavailable_grid_keeps_a_complete_placed_name() -> TestResult {
    let (_, catalog) = build(
        |dir| archive(dir, None, false, Route::UnavailableGrid),
        None,
    )?;
    assert_eq!(
        catalog.pointer("/placements/locationPhysicsGrids/availability"),
        Some(&Value::from("unavailable")),
        "the independently published grid projection cannot read the namespaced descriptor"
    );
    let placed = health::present(
        shop(&catalog, LOCATIONS[1])?,
        "starmapObjectGuid",
        &Value::from(PLANET),
        Some(true),
    )?;
    assert_eq!(
        placed["sourceSetComplete"], true,
        "the captured Child and global name sources still prove the Shop link"
    );
    health::closure(&catalog)
}

#[test]
fn an_unreadable_route_can_hide_a_competing_name() -> TestResult {
    let (_, catalog) = build(
        |dir| archive(dir, None, false, Route::UnreadableChild),
        None,
    )?;
    let placed = health::cell(
        shop(&catalog, LOCATIONS[1])?,
        "starmapObjectGuid",
        "present",
    )?;
    assert_eq!(
        placed["value"], PLANET,
        "the known matching Child is retained"
    );
    assert_eq!(
        placed["containsEncryptedSource"], true,
        "the known protected Child remains protected in the incomplete survey"
    );
    assert_eq!(
        placed["sourceSetComplete"], false,
        "the other reachable descriptor could contain a competing placed name"
    );
    let unknown = health::cell(
        shop(&catalog, LOCATIONS[5])?,
        "starmapObjectGuid",
        "unreadable",
    )?;
    assert_eq!(
        unknown["sourceSetComplete"], false,
        "a required descriptor was unavailable during the global name lookup"
    );
    health::closure(&catalog)
}
