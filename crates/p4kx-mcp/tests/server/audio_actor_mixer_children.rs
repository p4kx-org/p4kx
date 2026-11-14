// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{TestBank, TestEntry, TestHircObject, TestNodeBase};
use serde_json::{Value, json};

use crate::audio::serve;
use crate::support::{Install, TestResult};

const BANK: &str = "Data/Sounds/wwise/Actor.bnk";
const CHILDREN: [u32; 3] = [51, 0, 51];

#[test]
fn actor_mixer_children_have_individual_encrypted_receipts_and_original_order() -> TestResult {
    let body = TestBank::new(3)
        .object(TestHircObject::actor_mixer(
            9,
            &CHILDREN,
            TestNodeBase::Plain,
        ))
        .bytes();
    let install = Install::new(vec![TestEntry::zstd(BANK, body.as_slice()).encrypted()])?;
    let mut server = serve(&install)?;
    let result = server.ok("list_soundbank_objects", &json!({"soundbank": BANK}))?;
    let object = result.pointer("/objects/0").ok_or("physical ActorMixer")?;
    assert_eq!(
        object.get("children"),
        Some(&json!(CHILDREN)),
        "legacy children"
    );
    let shared = object
        .get("node_fields")
        .ok_or("original ActorMixer node")?;
    let original = shared
        .get("actor_mixer_children")
        .ok_or("original ActorMixer children")?;
    let count = original.get("declared_count").ok_or("authored count")?;
    assert_eq!(count.get("value"), Some(&json!(3)), "authored count");
    assert_eq!(
        count.get("containsEncryptedSource"),
        Some(&json!(true)),
        "owning bank"
    );
    let rows = original
        .get("children")
        .and_then(Value::as_array)
        .ok_or("child rows")?;
    assert_eq!(rows.len(), CHILDREN.len(), "all physical rows");
    for (ordinal, (row, value)) in rows.iter().zip(CHILDREN).enumerate() {
        assert_eq!(
            row.pointer("/ordinal/value"),
            Some(&json!(ordinal)),
            "physical position"
        );
        verify_child(shared, row.get("id").ok_or("child field")?, &body, value)?;
    }
    Ok(())
}

fn verify_child(shared: &Value, id: &Value, original: &[u8], expected: u32) -> TestResult {
    assert_eq!(
        id.get("value"),
        Some(&json!(expected)),
        "physical child value"
    );
    assert_eq!(
        id.get("containsEncryptedSource"),
        Some(&json!(true)),
        "owning bank"
    );
    let refs = id
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or("physical receipt")?;
    let [reference] = refs.as_slice() else {
        return Err("expected one direct child receipt".into());
    };
    let uses = shared
        .pointer("/source_inventory/uses")
        .and_then(Value::as_array)
        .ok_or("inventory")?;
    let source = uses
        .iter()
        .find(|used| used.get("id") == Some(reference))
        .ok_or("source use")?;
    let range = source
        .pointer("/location/indices")
        .and_then(Value::as_array)
        .ok_or("physical word range")?;
    let [start, end] = range.as_slice() else {
        return Err("expected two physical offsets".into());
    };
    let offset = |value: &Value| -> Result<usize, Box<dyn std::error::Error>> {
        Ok(value.as_str().ok_or("decimal offset")?.parse()?)
    };
    assert_eq!(
        original.get(offset(start)?..offset(end)?),
        Some(expected.to_le_bytes().as_slice()),
        "original ID word"
    );
    Ok(())
}
