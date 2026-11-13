// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::{Value, json};

use super::inventory::{Disposition, FieldKey};
use super::path::Path;

const OWNER: &str = "SQuantumDriveEffectTemplateRef";
const FIELD: &str = "tagRecord";

#[test]
fn quantum_effect_audit_retains_repeated_references_from_their_actual_owner() -> super::Result<()> {
    let payload = fixture();
    let values = mapping()?.select(&payload, &|| false)?;
    assert_eq!(
        values,
        [&json!("original"), &json!("original")],
        "each distinct original slot counts once; sibling owners do not supply missing values"
    );
    let mut missing = payload;
    *missing
        .pointer_mut("/effectTags/value/values/2/value")
        .ok_or("matching field fixture")? = carrier("null", Value::Null);
    let values = mapping()?.select(&missing, &|| false)?;
    assert_eq!(
        values,
        [&json!("original")],
        "a same-named populated field on another owner cannot replace a missing reference"
    );
    Ok(())
}

#[test]
fn malformed_or_reused_arena_edges_cannot_supply_reviewed_counts() -> super::Result<()> {
    let path = mapping()?;
    for ordinal in ["99", "1", "2", "03"] {
        let mut payload = fixture();
        *payload
            .pointer_mut("/effectTags/value/values/0/value/value/elements/1/valueIndex")
            .ok_or("fixture array slot")? = json!(ordinal);
        assert!(
            path.select(&payload, &|| false).is_err(),
            "dangling, cyclic or repeated ownership is rejected"
        );
    }
    let mut wrong_kind = fixture();
    *wrong_kind
        .pointer_mut("/effectTags/value/values/2/value/value/type")
        .ok_or("fixture Reference type")? = json!("string");
    assert!(
        path.select(&wrong_kind, &|| false).is_err(),
        "a field with a changed payload type cannot count as a Reference"
    );
    assert!(
        path.select(&fixture(), &|| true).is_err(),
        "arena inspection remains cancellable"
    );
    Ok(())
}

fn mapping() -> super::Result<Path> {
    let inventory = super::baseline::inventory();
    let mappings = inventory.reviewed()?;
    let mapping = mappings
        .get(&FieldKey::new(OWNER, FIELD))
        .ok_or("reviewed original effect Reference")?;
    let Disposition::Exported { path, .. } = &mapping.disposition else {
        return Err("the reviewed effect Reference has no catalog path".into());
    };
    Ok(path.clone())
}

fn fixture() -> Value {
    json!({
        "effectTags": {"value": {
            "roots": [edge("effectTags", "0")],
            "values": [
                node(json!({"type":"array","elements":[reference("1"),reference("3"),reference("5")]})),
                owner(OWNER, "2"),
                target("original"),
                owner("FixtureSiblingEffect", "4"),
                target("unrelated"),
                owner(OWNER, "6"),
                target("original"),
            ],
        }},
    })
}

fn node(value: Value) -> Value {
    json!({"value": carrier("present", value)})
}

fn owner(name: &str, slot: &str) -> Value {
    node(json!({
        "type":"struct",
        "sourceType":carrier("present", json!(name)),
        "fields":[edge(FIELD, slot)],
    }))
}

fn target(value: &str) -> Value {
    node(json!({"type":"recordReference","value":{"guid":carrier("present", json!(value))}}))
}

fn edge(name: &str, slot: &str) -> Value {
    json!({"name":carrier("present", json!(name)),"value":reference(slot)})
}

fn reference(slot: &str) -> Value {
    json!({"valueIndex":slot})
}

fn carrier(state: &str, value: Value) -> Value {
    Value::Object(serde_json::Map::from_iter([
        ("state".to_owned(), json!(state)),
        ("value".to_owned(), value),
        ("containsEncryptedSource".to_owned(), Value::Bool(false)),
        ("sourceUsesRefs".to_owned(), Value::Array(Vec::new())),
        ("sourceSetComplete".to_owned(), Value::Bool(true)),
    ]))
}
