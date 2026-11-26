// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Loadouts of the P4KX catalog through the public API: an Entity that holds itself and an
//! empty port with a nested Loadout.

use crate::loadouts_fixture::{ALPHA, GUN, LOOP, NESTED};
use crate::loadouts_support::{build, guid, loadout, names, port};

use std::error::Error;

use serde_json::{Value, json};

type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn an_entity_that_holds_itself_is_an_unresolved_cycle_and_is_not_attached() -> TestResult {
    let (built, catalog) = build(None)?;
    let entry = loadout(&catalog, LOOP)?;
    let held = port(&entry["ports"], "loop_1")?;
    assert_eq!(
        (
            &held["unresolved"],
            &held["itemGuid"]["value"],
            &held["itemClassName"]["value"],
            &held["ports"]
        ),
        (
            &json!("cycle"),
            &json!(guid(LOOP)),
            &Value::Null,
            &json!([])
        ),
        "the cycle is cut at the port and the GUID the Loadout names is kept"
    );
    assert_eq!(entry["issues"], json!(["cycle"]), "the code is listed");
    let summary = built.summary().loadouts();
    assert_eq!(
        (summary.unresolved, summary.with_issues, summary.attached),
        (3, 3, 10),
        "counted as unresolved, and not among the 10 attached"
    );
    Ok(())
}

#[test]
fn an_empty_port_lists_the_ports_of_its_nested_loadout() -> TestResult {
    let (_, catalog) = build(None)?;
    let body = port(&loadout(&catalog, NESTED)?["ports"], "body")?;
    assert_eq!(
        (
            &body["itemGuid"]["value"],
            &body["itemClassName"]["value"],
            &body["unresolved"]
        ),
        (&Value::Null, &Value::Null, &Value::Null),
        "the empty port holds nothing itself"
    );
    assert_eq!(
        body["visibility"],
        json!({
            "state": "present",
            "value": "unspecified",
            "containsEncryptedSource": null,
            "sourceUsesRefs": [],
            "sourceSetComplete": false
        }),
        "a port without an observed flag definition keeps unknown visibility evidence"
    );
    assert_eq!(names(&body["ports"]), ["seat_1"], "the nested port");
    let seat = port(&body["ports"], "seat_1")?;
    assert_eq!(
        (&seat["itemGuid"]["value"], &seat["itemClassName"]["value"]),
        (&json!(guid(GUN)), &json!("ACME_Gun")),
        "and what the nested Loadout attaches to it"
    );
    Ok(())
}

#[test]
fn vehicle_parts_sharing_a_port_name_all_take_the_first_part_as_the_tree_does() -> TestResult {
    let (_, catalog) = build(None)?;
    let ports = &loadout(&catalog, ALPHA)?["ports"];
    let gun = port(ports, "hardpoint_gun")?;
    assert_eq!(
        (&gun["minSize"]["value"], &gun["maxSize"]["value"]),
        (&json!(1), &json!(3)),
        "a part named in another case does not replace the first"
    );
    let sizes: Vec<_> = ports
        .as_array()
        .into_iter()
        .flatten()
        .filter(|port| port["name"]["value"] == "hardpoint_empty")
        .map(|port| {
            (
                port["minSize"]["value"].clone(),
                port["maxSize"]["value"].clone(),
            )
        })
        .collect();
    assert_eq!(
        sizes,
        [(json!(2), json!(2)), (json!(2), json!(2))],
        "each of the two vehicle-only ports reads the first part"
    );
    Ok(())
}
