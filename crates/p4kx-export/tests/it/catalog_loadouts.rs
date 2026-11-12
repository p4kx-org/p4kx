// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Loadouts of ships and items in the Basetool catalog, through the public API: the selection and
//! the ports.

#[path = "catalog_item_alignment.rs"]
mod catalog_item_alignment;

use crate::loadouts_fixture::{
    ALPHA, BROKEN, EMPTY_LOADOUT, GUN, GUN_DEFAULT, GUN_OVERRIDE, LOOP, NESTED, RACK, SNUB, TURRET,
};
use crate::loadouts_support::{build, guid, loadout, names, port};

use std::error::Error;

use serde_json::{Value, json};

type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn every_ship_and_every_item_with_ports_has_a_loadout_ordered_by_guid() -> TestResult {
    let (_, catalog) = build(None)?;
    let entries = catalog["loadouts"].as_array().ok_or("loadouts")?;
    let owners: Vec<(&str, &str)> = entries
        .iter()
        .filter_map(|entry| Some((entry["entityGuid"].as_str()?, entry["entityKind"].as_str()?)))
        .collect();
    let (alpha, snub, broken, nested) = (guid(ALPHA), guid(SNUB), guid(BROKEN), guid(NESTED));
    let (turret, rack, looped) = (guid(TURRET), guid(RACK), guid(LOOP));
    assert_eq!(
        owners,
        [
            (alpha.as_str(), "ship"),
            (snub.as_str(), "ship"),
            (broken.as_str(), "ship"),
            (nested.as_str(), "ship"),
            (turret.as_str(), "item"),
            (rack.as_str(), "item"),
            (looped.as_str(), "item"),
        ],
        "the ships and the items with ports, ordered by GUID"
    );
    for plain in [GUN, GUN_DEFAULT, GUN_OVERRIDE, EMPTY_LOADOUT] {
        assert!(
            loadout(&catalog, plain).is_err(),
            "an item without a port has no loadout"
        );
    }
    Ok(())
}

#[test]
fn the_ports_list_the_loadout_ports_then_the_vehicle_only_ports() -> TestResult {
    let (_, catalog) = build(None)?;
    let alpha = loadout(&catalog, ALPHA)?;
    assert_eq!(
        names(&alpha["ports"]),
        [
            "hardpoint_turret",
            "hardpoint_gun",
            "hangar",
            "lost",
            "hardpoint_old",
            "hardpoint_ref",
            "hardpoint_empty",
            "hardpoint_empty"
        ],
        "Loadout order, then the ports only the vehicle implementation knows, one per part"
    );
    let empty = port(&alpha["ports"], "hardpoint_empty")?;
    assert_eq!(
        (
            &empty["source"],
            &empty["minSize"],
            &empty["itemGuid"],
            &empty["visibility"]
        ),
        (
            &json!("vehicle"),
            &json!(2),
            &Value::Null,
            &json!("visible")
        ),
        "a vehicle-only port is empty"
    );
    assert_eq!(
        empty["types"],
        json!([
            {"type": "Missile", "subTypes": []},
            {"type": "Bomb", "subTypes": []}
        ]),
        "its types"
    );
    Ok(())
}

#[test]
fn a_record_port_keeps_its_definition_and_the_vehicle_file_gives_the_visibility() -> TestResult {
    let (_, catalog) = build(None)?;
    let turret = port(&loadout(&catalog, ALPHA)?["ports"], "hardpoint_turret")?;
    assert_eq!(
        (
            &turret["minSize"],
            &turret["maxSize"],
            &turret["types"],
            &turret["flags"],
            &turret["portTags"],
            &turret["requiredTags"],
            &turret["displayNameKey"],
            &turret["visibility"],
            &turret["source"]
        ),
        (
            &json!(2),
            &json!(4),
            &json!([{"type": "Turret", "subTypes": ["GunTurret"]}]),
            &json!(["uneditable"]),
            &json!(["Turret"]),
            &json!(["Mount"]),
            &Value::Null,
            &json!("invisible"),
            &json!("loadout")
        ),
        "the Record's definition wins; the vehicle file's flag hides the port"
    );
    assert_eq!(
        (&turret["itemGuid"], &turret["itemClassName"]),
        (&json!(guid(TURRET)), &json!("ACME_Turret")),
        "the attached item"
    );
    Ok(())
}

#[test]
fn a_port_the_record_lacks_takes_its_definition_from_the_vehicle_file() -> TestResult {
    let (_, catalog) = build(None)?;
    let gun = port(&loadout(&catalog, ALPHA)?["ports"], "hardpoint_gun")?;
    assert_eq!(
        (
            &gun["minSize"],
            &gun["maxSize"],
            &gun["types"],
            &gun["flags"],
            &gun["portTags"],
            &gun["requiredTags"],
            &gun["visibility"]
        ),
        (
            &json!(1),
            &json!(3),
            &json!([{"type": "WeaponGun", "subTypes": []}]),
            &json!(["uneditable", "invisible"]),
            &json!(["$Gun"]),
            &json!(["Mount", "Unknown", "Dup"]),
            &json!("invisible")
        ),
        "sizes, types, flags without the leading $, tags as stored and visibility"
    );
    Ok(())
}

#[test]
fn tags_are_free_strings_kept_as_stored_and_matched_by_name() -> TestResult {
    let (_, catalog) = build(None)?;
    let alpha = loadout(&catalog, ALPHA)?;
    let turret = port(&alpha["ports"], "hardpoint_turret")?;
    assert_eq!(
        (&turret["portTags"], &turret["requiredTags"]),
        (&json!(["Turret"]), &json!(["Mount"])),
        "the tags of a Record port"
    );
    let rack = port(&loadout(&catalog, RACK)?["ports"], "slot_1")?;
    assert_eq!(
        (&rack["portTags"], &rack["requiredTags"]),
        (&json!([]), &json!([])),
        "a port without tags"
    );
    let item = catalog["items"]
        .as_array()
        .ok_or("items")?
        .iter()
        .find(|item| item["guid"] == guid(GUN).as_str())
        .ok_or("the gun")?;
    assert_eq!(
        item["tags"],
        json!(["Gun", "Unknown", "Dup", "mount"]),
        "the words of the item's tags, in case as stored"
    );
    let gun = port(&alpha["ports"], "hardpoint_gun")?;
    assert!(
        gun.get("portTagGuids").is_none() && item.get("tagGuids").is_none(),
        "no tag GUIDs: the tags are matched by name"
    );
    Ok(())
}

#[test]
fn the_ports_of_an_attached_item_are_nested_with_the_overrides_of_the_loadout() -> TestResult {
    let (_, catalog) = build(None)?;
    let turret = port(&loadout(&catalog, ALPHA)?["ports"], "hardpoint_turret")?;
    assert_eq!(
        names(&turret["ports"]),
        ["gun_1", "gun_2"],
        "the turret's ports"
    );
    let first = port(&turret["ports"], "gun_1")?;
    assert_eq!(
        (
            &first["source"],
            &first["itemGuid"],
            &first["itemClassName"]
        ),
        (
            &json!("override"),
            &json!(guid(GUN_OVERRIDE)),
            &json!("ACME_Gun_Override")
        ),
        "the ship's Loadout replaces the turret's default"
    );
    let second = port(&turret["ports"], "gun_2")?;
    assert_eq!(
        (&second["source"], &second["itemGuid"]),
        (&json!("port_default"), &json!(guid(GUN_DEFAULT))),
        "the port's own default item"
    );
    let own = port(&loadout(&catalog, TURRET)?["ports"], "gun_1")?;
    assert_eq!(
        (&own["source"], &own["itemGuid"]),
        (&json!("port_default"), &json!(guid(GUN_DEFAULT))),
        "the turret's own loadout keeps its default"
    );
    Ok(())
}

#[test]
fn an_item_with_a_port_and_no_loadout_lists_the_port_empty() -> TestResult {
    let (_, catalog) = build(None)?;
    let rack = loadout(&catalog, RACK)?;
    assert_eq!(rack["ports"].as_array().map(Vec::len), Some(1), "one port");
    let slot = port(&rack["ports"], "slot_1")?;
    assert_eq!(
        (&slot["source"], &slot["itemGuid"], &slot["itemClassName"]),
        (&json!("unset"), &Value::Null, &Value::Null),
        "the port holds nothing"
    );
    Ok(())
}
