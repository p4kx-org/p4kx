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

#[test]
fn root_member_accounting_requires_the_original_slot_even_when_its_value_is_null()
-> super::Result<()> {
    use std::collections::{BTreeMap, BTreeSet};

    use super::accounting::{self, Projection};

    let inventory = super::baseline::inventory();
    let key = FieldKey::new("SCItemQuantumDriveParams", "tracePoint");
    let mappings = inventory.reviewed()?;
    let mapping = mappings.get(&key).ok_or("reviewed arena root")?;
    let Disposition::Exported { group, path } = &mapping.disposition else {
        return Err("the reviewed root member has no catalog path".into());
    };
    let inventory = super::inventory::Inventory {
        fields: vec![(**mapping).clone()],
        ..inventory.clone()
    };
    let owner = super::fixtures::GUID.parse()?;
    let (schema, observations) = null_root_observation(owner, &key);
    let null_cell = carrier("null", Value::Null);
    let row = json!({"additionalFields": {
        "roots":[edge("tracePoint", "0"), edge("nested", "1")],
        "values":[{"value":null_cell}, owner_field("FixtureOther", "tracePoint", "2"), target("foreign")]
    }});

    let nested_only = json!({"additionalFields": {
        "roots":[edge("nested", "0")],
        "values":[owner_field("FixtureOther", "tracePoint", "1"), target("foreign")]
    }});
    let duplicate = json!({"additionalFields": {
        "roots":[edge("tracePoint", "0"), edge("tracePoint", "1")],
        "values":[{"value":null_cell}, {"value":null_cell}]
    }});
    let empty_text = json!({"additionalFields": {
        "roots":[edge("tracePoint", "0")],
        "values":[node(json!({"type":"string","value":{"type":"gameText","sourceText":""}}))]
    }});
    for (row, exported, unhandled) in [
        (&row, 1, 0),
        (&nested_only, 0, 1),
        (&duplicate, 2, 1),
        (&empty_text, 1, 1),
    ] {
        let projection = Projection {
            groups: BTreeMap::from([(group.clone(), BTreeMap::from([(owner, vec![row])]))]),
            rejected: BTreeSet::new(),
            exclusions: BTreeMap::new(),
        };
        let counts =
            accounting::count_fields(&schema, &observations, &inventory, &projection, &|| false)?;
        let count = counts.get(&key).ok_or("accounted arena root")?;
        assert_eq!(
            (count.populated, count.exported, count.unhandled),
            (0, exported, unhandled),
            "lost, altered or excess original slots fail the audit"
        );
    }
    let false_absence = json!({"additionalFields":{
        "roots":[edge("tracePoint", "0")],
        "values":[{"value":carrier("absent", Value::Null)}]
    }});
    assert!(path.select(&false_absence, &|| false).is_err());
    Ok(())
}

fn null_root_observation(
    owner: p4kx_extraction::Guid,
    key: &FieldKey,
) -> (super::schema::Schema, super::source::Observations) {
    use std::collections::BTreeMap;

    use super::schema::{Declaration, Schema};
    use super::source::Observation;

    let schema = Schema {
        declarations: BTreeMap::from([(
            key.clone(),
            Declaration {
                target: None,
                array: false,
            },
        )]),
        owners: BTreeMap::new(),
    };
    let observations = BTreeMap::from([(
        (owner, key.clone()),
        Observation {
            present: 1,
            null: 1,
            ..Observation::default()
        },
    )]);
    (schema, observations)
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
    owner_field(name, FIELD, slot)
}

fn owner_field(name: &str, field: &str, slot: &str) -> Value {
    node(json!({
        "type":"struct",
        "sourceType":carrier("present", json!(name)),
        "fields":[edge(field, slot)],
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
