// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Loadouts of ships and items in the P4KX catalog, through the public API: the references
//! of the ports, the summary, the Filter and the order.

use crate::loadouts_fixture::{
    self, ALPHA, BROKEN, GUN_DEFAULT, GUN_OVERRIDE, GUN_TEMPLATE, MISSING, SNUB, TURRET,
};
use crate::loadouts_support::{self, build, guid, known, loadout, names, port};

use std::error::Error;

use p4kx_export::{Filter, Kind, LoadoutSummary};
use p4kx_extraction::{Archive, ArchiveWatch, DataForgeEntry};
use serde_json::{Value, json};

type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn a_docked_vehicle_is_a_ship_reference_whose_ports_are_not_listed() -> TestResult {
    let (_, catalog) = build(None)?;
    let hangar = port(&loadout(&catalog, ALPHA)?["ports"], "hangar")?;
    assert_eq!(
        (
            &hangar["itemGuid"],
            &hangar["dockedVehicle"],
            &hangar["ports"]
        ),
        (&json!(guid(SNUB)), &json!(true), &json!([])),
        "marked and not expanded"
    );
    let own = loadout(&catalog, SNUB)?;
    assert_eq!(
        names(&own["ports"]),
        ["snub_gun"],
        "the docked vehicle has a loadout of its own"
    );
    Ok(())
}

#[test]
fn an_attachment_that_cannot_be_resolved_is_kept_with_its_code() -> TestResult {
    let (_, catalog) = build(None)?;
    let alpha = loadout(&catalog, ALPHA)?;
    let lost = port(&alpha["ports"], "lost")?;
    assert_eq!(
        (
            &lost["unresolved"],
            &lost["itemGuid"],
            &lost["itemClassName"]
        ),
        (&json!("unknown_class"), &Value::Null, &json!("NoSuchClass")),
        "a class name no Entity has"
    );
    let dangling = port(&alpha["ports"], "hardpoint_ref")?;
    assert_eq!(
        (
            &dangling["unresolved"],
            &dangling["itemGuid"],
            &dangling["itemClassName"]
        ),
        (
            &json!("dangling_reference"),
            &json!(guid(MISSING)),
            &Value::Null
        ),
        "a Reference to a Record that does not exist"
    );
    assert_eq!(
        alpha["issues"],
        json!(["dangling_reference", "unknown_class"]),
        "the codes, sorted"
    );
    assert_eq!(
        loadout(&catalog, BROKEN)?["issues"],
        json!(["vehicle_implementation"]),
        "a vehicle file that cannot be read"
    );
    Ok(())
}

#[test]
fn the_summary_counts_the_ports_and_the_entities_without_an_entry() -> TestResult {
    let (built, catalog) = build(None)?;
    assert_eq!(
        built.summary().loadouts(),
        LoadoutSummary {
            ships: 4,
            items: 3,
            ports: 17,
            attached: 10,
            docked_vehicles: 1,
            unresolved: 3,
            outside: 1,
            with_issues: 3,
        },
        "the leftover item the ship holds has no entry in items"
    );
    let template = guid(GUN_TEMPLATE);
    assert!(
        !known(&catalog).contains(&template.as_str()),
        "the leftover is no item"
    );
    let held = port(&loadout(&catalog, ALPHA)?["ports"], "hardpoint_old")?;
    assert_eq!(held["itemGuid"], json!(template), "but it stays referenced");
    let kind = built.summary().kind(Kind::Loadouts).ok_or("the kind")?;
    assert_eq!((kind.count, kind.complete), (7, true), "the kind summary");
    assert_eq!(
        catalog["completeness"]["loadouts"]["status"], "complete",
        "completeness"
    );
    Ok(())
}

#[test]
fn every_attached_guid_resolves_except_the_counted_ones() -> TestResult {
    let (built, catalog) = build(None)?;
    let known = known(&catalog);
    let mut outside = Vec::new();
    let mut stack: Vec<&Value> = catalog["loadouts"]
        .as_array()
        .ok_or("loadouts")?
        .iter()
        .flat_map(|entry| entry["ports"].as_array().into_iter().flatten())
        .collect();
    while let Some(port) = stack.pop() {
        stack.extend(port["ports"].as_array().into_iter().flatten());
        if port["unresolved"].is_null()
            && let Some(guid) = port["itemGuid"].as_str()
            && !known.contains(&guid)
            && !outside.contains(&guid)
        {
            outside.push(guid);
        }
    }
    assert_eq!(outside.len(), built.summary().loadouts().outside, "counted");
    for entry in catalog["loadouts"].as_array().ok_or("loadouts")? {
        let owner = entry["entityGuid"].as_str().ok_or("a GUID")?;
        assert!(known.contains(&owner), "{owner} is an item or a ship");
    }
    Ok(())
}

#[test]
fn a_filter_limits_the_loadouts_to_the_selected_entities_and_marks_the_kind_partial() -> TestResult
{
    let filter = Filter::new("*/spaceships/*");
    let (built, catalog) = build(Some(&filter))?;
    let owners: Vec<&str> = catalog["loadouts"]
        .as_array()
        .ok_or("loadouts")?
        .iter()
        .filter_map(|entry| entry["entityKind"].as_str())
        .collect();
    assert_eq!(owners, ["ship", "ship", "ship", "ship"], "only the ships");
    assert_eq!(
        catalog["completeness"]["loadouts"],
        json!({"status": "partial", "filter": "*/spaceships/*"}),
        "partial"
    );
    assert_eq!(
        built.summary().loadouts().items,
        0,
        "no item loadout under the Filter"
    );
    Ok(())
}

#[test]
fn a_shared_dataforge_preserves_sequential_ship_and_item_catalogs() -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = loadouts_fixture::write_archive(dir.path())?;
    let archive = Archive::open_with_key(path, Some(p4kx_testkit::test_key()))?;
    let watch = ArchiveWatch::new(&archive);
    watch.check()?;
    let entry = DataForgeEntry::read(&archive)?;

    {
        let ships = compare_reused_catalog(&archive, &entry, "*/spaceships/*")?;
        let ports = &loadout(&ships, ALPHA)?["ports"];
        let empty = port(ports, "hardpoint_empty")?;
        assert_eq!(empty["itemGuid"], Value::Null, "the empty vehicle port");
        let turret = port(ports, "hardpoint_turret")?;
        let nested = port(&turret["ports"], "gun_1")?;
        assert_eq!(
            nested["itemGuid"],
            json!(guid(GUN_OVERRIDE)),
            "the ship's nested attachment override"
        );
    }
    watch.check()?;

    let items = compare_reused_catalog(&archive, &entry, "*/scitem/*")?;
    let own = port(&loadout(&items, TURRET)?["ports"], "gun_1")?;
    assert_eq!(
        own["itemGuid"],
        json!(guid(GUN_DEFAULT)),
        "the next filtered read retains the item's own default attachment"
    );
    watch.check()?;
    Ok(())
}

fn compare_reused_catalog(
    archive: &Archive,
    entry: &DataForgeEntry,
    pattern: &str,
) -> Result<Value, Box<dyn Error>> {
    let filter = Filter::new(pattern);
    let options = loadouts_support::options(Some(&filter));
    let direct = p4kx_export::build_catalog(archive, &options)?;
    let reused = p4kx_export::build_catalog_from(archive, entry, &options)?;
    assert!(
        reused.json() == direct.json(),
        "{pattern} preserves all catalog bytes, including source metadata"
    );
    assert_eq!(reused.summary(), direct.summary(), "{pattern} summary");
    Ok(serde_json::from_slice(reused.json())?)
}

#[test]
fn equal_input_gives_identical_loadouts() -> TestResult {
    let (_, first) = build(None)?;
    let (_, second) = build(None)?;
    assert_eq!(
        serde_json::to_string(&first["loadouts"])?,
        serde_json::to_string(&second["loadouts"])?,
        "deterministic"
    );
    Ok(())
}
