// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::BodyVariant;
use p4kx_assembly::character::lowered_hash;
use p4kx_blender::{PlayerSource, player_source_schema};
use p4kx_extraction::CancelToken;
use serde_json::Value;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

#[path = "player_source_fixture.rs"]
mod fixture;

pub(super) use fixture::fixture_with_entries;
use fixture::{fixture_with, fixture_with_forge, fixture_with_motion, forge};

#[path = "player_adb_declaration_assertions.rs"]
mod adb_declaration_assertions;

#[path = "player_motion_option_assertions.rs"]
mod motion_option_assertions;

#[path = "player_tag_definition_assertions.rs"]
mod tag_definition_assertions;

#[path = "player_adb_tag_selectors.rs"]
mod adb_tag_selectors;

#[path = "player_weapon_motion_links.rs"]
mod weapon_motion_links;

#[path = "player_actor_rig_candidates.rs"]
mod actor_rig_candidates;

#[path = "player_controller_contexts.rs"]
mod controller_contexts;

#[path = "player_subadb_selectors.rs"]
mod subadb_selectors;

#[path = "player_subadb_graph_assertions.rs"]
mod subadb_graph_assertions;

#[path = "player_shared_capture.rs"]
mod shared_capture;

#[path = "player_rejected_capture.rs"]
mod rejected_capture;

#[test]
fn player_source_preserves_body_variant_port_order_and_exact_loadout_targets() -> TestResult {
    let (_folder, archive) = fixture_with_entries([])?;
    let cancel = CancelToken::new();
    let male = serde_json::to_value(PlayerSource::read(&archive, BodyVariant::Male, &cancel)?)?;
    let female = serde_json::to_value(PlayerSource::read(&archive, BodyVariant::Female, &cancel)?)?;
    let validator = jsonschema::validator_for(&player_source_schema())?;
    validator
        .validate(&male)
        .map_err(|error| error.to_string())?;
    validator
        .validate(&female)
        .map_err(|error| error.to_string())?;
    verify_bodies(&male, &female);
    verify_port_definitions(&male, &female);
    verify_loadout(&male);
    verify_xml_port_definitions(&male);
    verify_motion(&male);
    motion_option_assertions::verify(&male)?;
    subadb_graph_assertions::verify(&male)?;
    Ok(())
}

fn verify_bodies(male: &Value, female: &Value) {
    assert_eq!(
        male.pointer("/bodyVariant").and_then(Value::as_str),
        Some("male"),
        "explicit male source"
    );
    assert_eq!(
        female.pointer("/bodyVariant").and_then(Value::as_str),
        Some("female"),
        "explicit female source"
    );
    assert_eq!(
        male.pointer("/body/value/data/ports/value/1/parent")
            .and_then(Value::as_u64),
        Some(0),
        "physical parent slot"
    );
    assert_eq!(
        male.pointer("/body/value/data/ports/value/1/item/value/type")
            .and_then(Value::as_str),
        Some("empty"),
        "authored empty CHF port"
    );
    assert_eq!(
        male.pointer("/body/value/data/ports/value/2/item/value/type")
            .and_then(Value::as_str),
        Some("matched"),
        "exact CHF GUID"
    );
    assert_eq!(
        female
            .pointer("/body/value/data/ports/value/1/item/value/type")
            .and_then(Value::as_str),
        Some("unmatched"),
        "unknown GUID is retained"
    );
    assert_eq!(
        male.pointer("/body/value/data/ports/value/2/item/containsEncryptedSource")
            .and_then(Value::as_bool),
        Some(true),
        "original CHF and DataForge evidence"
    );
}

fn verify_port_definitions(male: &Value, female: &Value) {
    let paths = "/body/value/data/ports/value";
    let value = |doc: &Value, pointer: &str| {
        doc.pointer(&format!("{paths}/{pointer}"))
            .and_then(Value::as_str)
            .map(str::to_owned)
    };
    assert_eq!(
        value(male, "0/definition/value/type").as_deref(),
        Some("noOwner"),
        "CHF root has no parent"
    );
    assert_eq!(
        value(
            male,
            "1/definition/value/definition/attachment/value/helpers/value/0/name/value"
        )
        .as_deref(),
        Some("BodyAnchor"),
        "same CRC resolves on body"
    );
    assert_eq!(
        value(male, "2/definition/value/type").as_deref(),
        Some("ambiguousPort"),
        "both original same-name definitions remain visible"
    );
    assert_eq!(
        value(
            male,
            "3/definition/value/definition/attachment/value/helpers/value/0/name/value"
        )
        .as_deref(),
        Some("WeaponAnchor"),
        "same CRC resolves on immediate weapon owner"
    );
    assert_eq!(
        value(female, "1/definition/value/type").as_deref(),
        Some("unmatchedPort"),
        "missing owner-local declaration remains explicit"
    );
}

fn verify_xml_port_definitions(doc: &Value) {
    let value = |index, field| {
        doc.pointer(&format!(
            "/loadouts/0/value/declarations/{index}/definition/value/{field}"
        ))
        .and_then(Value::as_str)
    };
    assert_eq!(
        value(1, "type"),
        Some("noOwner"),
        "root-group declaration has no authored owner"
    );
    assert_eq!(
        value(3, "type"),
        Some("emptyOwner"),
        "explicit empty parent remains separate"
    );
    assert_eq!(
        value(9, "type"),
        Some("matched"),
        "child port on the named physical owner"
    );
    assert_eq!(
        value(9, "definition/attachment/value/helpers/value/0/name/value"),
        Some("BodyAnchor"),
        "the XML class owner supplies the same original physical port"
    );
    assert_eq!(
        value(10, "type"),
        Some("unmatchedPort"),
        "a name absent from that owner stays unresolved"
    );
}

fn verify_loadout(doc: &Value) {
    let rows = "/loadouts/0/value/declarations";
    assert_eq!(
        doc.pointer(&format!("{rows}/1/target/value/type"))
            .and_then(Value::as_str),
        Some("empty"),
        "empty original itemName"
    );
    assert_eq!(
        doc.pointer(&format!("{rows}/3/target/value/records"))
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(2),
        "both same-name weapon records"
    );
    assert_eq!(
        doc.pointer(&format!("{rows}/4/target/value/type"))
            .and_then(Value::as_str),
        Some("unmatched"),
        "named missing original target"
    );
    assert_eq!(
        doc.pointer(&format!("{rows}/5/target/state"))
            .and_then(Value::as_str),
        Some("absent"),
        "omitted itemName differs from empty"
    );
    assert_eq!(
        doc.pointer(&format!("{rows}/6/parent"))
            .and_then(Value::as_u64),
        Some(5),
        "explicit empty nested Items group"
    );
    assert!(
        doc.pointer("/documents/0/nodes")
            .and_then(Value::as_array)
            .is_some_and(|nodes| nodes
                .iter()
                .any(|node| node.get("name").and_then(Value::as_str) == Some("InventoryItem"))),
        "additional original sections retained"
    );
}

fn verify_motion(doc: &Value) {
    let component = "/motion/components/value/0";
    assert_eq!(
        doc.pointer(&format!("{component}/controller/value/type"))
            .and_then(Value::as_str),
        Some("present"),
        "player actor selects the physically admitted controller"
    );
    assert_eq!(
        doc.pointer(&format!("{component}/thirdPersonDatabase/value/type"))
            .and_then(Value::as_str),
        Some("present"),
        "player actor selects the physically admitted ADB"
    );
    assert_eq!(
        doc.pointer(&format!("{component}/controller/containsEncryptedSource"))
            .and_then(Value::as_bool),
        Some(true),
        "the encrypted original actor selector contributes to the XML join"
    );
    assert_eq!(
        doc.pointer("/documents/1/nodes")
            .and_then(Value::as_array)
            .map(|nodes| nodes
                .iter()
                .filter(|node| node.get("name").and_then(Value::as_str) == Some("Walk"))
                .count()),
        Some(2),
        "repeated controller declarations remain ordered in the original document"
    );
}

#[test]
fn checked_in_player_source_schema_is_current_and_valid() -> TestResult {
    let schema = player_source_schema();
    jsonschema::draft202012::meta::validate(&schema).map_err(|error| error.to_string())?;
    let mut generated = serde_json::to_string_pretty(&schema)?;
    generated.push('\n');
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../schemas/player-source.schema.json");
    if std::env::var_os("P4KX_BLESS").is_some_and(|value| value == "1") {
        std::fs::write(&path, &generated)?;
    }
    assert_eq!(
        std::fs::read_to_string(path)?,
        generated,
        "regenerate with P4KX_BLESS=1"
    );
    Ok(())
}
