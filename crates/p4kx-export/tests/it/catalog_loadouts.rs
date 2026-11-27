// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Loadouts of ships and items in the P4KX catalog, through the public API: the selection and
//! the ports.

#[path = "catalog_item_alignment.rs"]
mod catalog_item_alignment;

use crate::health_origin_assertions as proof;
use crate::loadouts_fixture::{
    ALPHA, BROKEN, EMPTY_LOADOUT, GUN, GUN_DEFAULT, GUN_OVERRIDE, LOOP, NESTED, RACK, SNUB, TURRET,
};
use crate::loadouts_support::{build, guid, loadout, names, port};

use std::error::Error;

use serde_json::{Value, json};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

fn type_values(port: &Value) -> TestResult<Value> {
    let mut kinds = Vec::new();
    for kind in proof::rows(port, "/types/value")? {
        let subtypes = match proof::at(kind, "/subTypes/value")? {
            Value::Null => &[],
            Value::Array(subtypes) => subtypes.as_slice(),
            _ => return Err("expected /subTypes/value to hold an array or null".into()),
        };
        let subtypes = subtypes
            .iter()
            .map(|subtype| proof::at(subtype, "/value"))
            .collect::<Result<Vec<_>, _>>()?;
        kinds.push(json!({
            "type": proof::at(kind, "/type/value")?,
            "subTypes": subtypes,
        }));
    }
    Ok(Value::Array(kinds))
}

#[test]
fn every_ship_and_every_item_with_ports_has_a_loadout_ordered_by_guid() -> TestResult {
    let (_, catalog) = build(None)?;
    let entries = catalog["loadouts"].as_array().ok_or("loadouts")?;
    let owners: Vec<(&str, &str)> = entries
        .iter()
        .filter_map(|entry| {
            Some((
                entry.pointer("/entityGuid/value").and_then(Value::as_str)?,
                entry.pointer("/entityKind/value").and_then(Value::as_str)?,
            ))
        })
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
            &empty["minSize"]["value"],
            &empty["itemGuid"]["value"],
            &empty["visibility"]["value"]
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
        type_values(empty)?,
        json!([
            {"type": "Missile", "subTypes": ["IR", "EM", "IR"]},
            {"type": "Bomb", "subTypes": []},
            {"type": "Missile", "subTypes": ["Dumbfire", "Heat"]}
        ]),
        "the XML type and subtype orders retain repeated entries"
    );
    assert_eq!(
        empty["displayNameKey"]["value"], "@fixture_vehicle_port",
        "the XML display key remains as authored"
    );
    Ok(())
}

#[test]
fn a_record_port_keeps_its_definition_and_the_vehicle_file_gives_the_visibility() -> TestResult {
    let (_, catalog) = build(None)?;
    let turret = port(&loadout(&catalog, ALPHA)?["ports"], "hardpoint_turret")?;
    assert_eq!(
        (
            &turret["minSize"]["value"],
            &turret["maxSize"]["value"],
            &type_values(turret)?,
            &turret["flags"]["value"],
            &turret["portTags"]["value"],
            &turret["requiredTags"]["value"],
            &turret["displayNameKey"]["value"],
            &turret["visibility"]["value"],
            &turret["source"]
        ),
        (
            &json!(2),
            &json!(4),
            &json!([{"type": "Turret", "subTypes": ["GunTurret", "GunTurret"]}]),
            &json!(["uneditable"]),
            &json!(["Turret"]),
            &json!(["Mount"]),
            &json!("@LOC_EMPTY"),
            &json!("invisible"),
            &json!("loadout")
        ),
        "the Record's definition wins; the vehicle file's flag hides the port"
    );
    assert_eq!(
        (
            &turret["itemGuid"]["value"],
            &turret["itemClassName"]["value"]
        ),
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
            &gun["minSize"]["value"],
            &gun["maxSize"]["value"],
            &type_values(gun)?,
            &gun["flags"]["value"],
            &gun["portTags"]["value"],
            &gun["requiredTags"]["value"],
            &gun["visibility"]["value"]
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
    assert_eq!(
        gun["displayNameKey"]["value"], "@LOC_EMPTY",
        "the XML placeholder is retained in the source cell"
    );
    let size = crate::health_origin_assertions::cell(gun, "minSize", "present")?;
    let uses = crate::health_origin_assertions::uses(&catalog, size)?;
    assert!(
        uses.iter().any(|usage| {
            usage.pointer("/location/namespace").and_then(Value::as_str) == Some("textXmlAttribute")
                && usage.pointer("/location/field").and_then(Value::as_str) == Some("minSize")
                && usage.get("role").and_then(Value::as_str) == Some("conversion")
        }),
        "vehicle fallback retains the actual minSize attribute",
    );
    assert!(
        uses.iter().any(|usage| {
            usage["location"]["namespace"] == "textXmlElement"
                && usage["location"]["field"] == "$children"
                && usage["role"] == "selector"
        }),
        "the original Part search supplies its structural selector"
    );
    assert_eq!(size["sourceSetComplete"], json!(true));
    let key = crate::health_origin_assertions::cell(gun, "displayNameKey", "present")?;
    let uses = crate::health_origin_assertions::uses(&catalog, key)?;
    assert!(
        uses.iter().any(|usage| {
            usage["location"]["namespace"] == "textXmlAttribute"
                && usage["location"]["field"] == "display_name"
                && usage["role"] == "payload"
        }),
        "the XML key binds only to its original display name attribute",
    );
    Ok(())
}

#[test]
fn tags_are_free_strings_kept_as_stored_and_matched_by_name() -> TestResult {
    let (_, catalog) = build(None)?;
    let alpha = loadout(&catalog, ALPHA)?;
    let turret = port(&alpha["ports"], "hardpoint_turret")?;
    assert_eq!(
        (
            &turret["portTags"]["value"],
            &turret["requiredTags"]["value"]
        ),
        (&json!(["Turret"]), &json!(["Mount"])),
        "the tags of a Record port"
    );
    let rack = port(&loadout(&catalog, RACK)?["ports"], "slot_1")?;
    assert_eq!(
        (&rack["portTags"]["value"], &rack["requiredTags"]["value"]),
        (&json!([]), &json!([])),
        "explicit empty text gives an empty list"
    );
    let item = catalog["items"]
        .as_array()
        .ok_or("items")?
        .iter()
        .find(|item| {
            item.pointer("/guid/value").and_then(Value::as_str) == Some(guid(GUN).as_str())
        })
        .ok_or("the gun")?;
    let attachment = crate::catalog_items::selected_attachment(&catalog, item)?;
    let tags = attachment
        .pointer("/tags/value")
        .and_then(Value::as_str)
        .ok_or("original Tags")?;
    assert_eq!(
        tags.split_whitespace().collect::<Vec<_>>(),
        ["Gun", "Unknown", "Dup", "mount"],
        "the original tags preserve all words, case and repeats"
    );
    assert!(
        item.get("tags").is_none(),
        "the root does not duplicate the original Tags"
    );
    let gun = port(&alpha["ports"], "hardpoint_gun")?;
    assert!(
        gun.get("portTagGuids").is_none() && item.get("tagGuids").is_none(),
        "no tag GUIDs: the tags are matched by name"
    );
    Ok(())
}

#[test]
fn xml_port_lists_keep_their_own_original_attributes_and_known_absence() -> TestResult {
    use crate::health_origin_assertions as proof;

    let (_, catalog) = build(None)?;
    let alpha = loadout(&catalog, ALPHA)?;
    let gun = port(&alpha["ports"], "hardpoint_gun")?;
    let empty = port(&alpha["ports"], "hardpoint_empty")?;
    for (row, field, state, namespace, role) in [
        (gun, "flags", "present", "textXmlAttribute", "conversion"),
        (gun, "portTags", "present", "textXmlAttribute", "conversion"),
        (
            gun,
            "requiredTags",
            "present",
            "textXmlAttribute",
            "conversion",
        ),
        (empty, "portTags", "absent", "textXmlElement", "selector"),
    ] {
        let value = proof::cell(row, field, state)?;
        let uses = proof::uses(&catalog, value)?;
        assert!(
            uses.iter().any(|usage| {
                usage.pointer("/location/namespace").and_then(Value::as_str) == Some(namespace)
                    && usage.pointer("/location/field").and_then(Value::as_str) == Some(field)
                    && usage.get("role").and_then(Value::as_str) == Some(role)
            }),
            "{field} retains the exact XML location for {state}",
        );
    }
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
            &first["itemGuid"]["value"],
            &first["itemClassName"]["value"]
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
        (&second["source"], &second["itemGuid"]["value"]),
        (&json!("port_default"), &json!(guid(GUN_DEFAULT))),
        "the port's own default item"
    );
    let own = port(&loadout(&catalog, TURRET)?["ports"], "gun_1")?;
    assert_eq!(
        (&own["source"], &own["itemGuid"]["value"]),
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
        (
            &slot["source"],
            &slot["itemGuid"]["value"],
            &slot["itemClassName"]["value"]
        ),
        (&json!("unset"), &Value::Null, &Value::Null),
        "the port holds nothing"
    );
    Ok(())
}

#[path = "catalog_consumer_refresh.rs"]
mod consumer_refresh;

#[path = "catalog_loadout_sdk.rs"]
mod direct_sdk;
