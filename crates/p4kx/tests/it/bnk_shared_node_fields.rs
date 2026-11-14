// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{TestBank, TestContainerKind, TestHircObject, TestNodeBase};
use serde_json::Value;

use super::{TestResult, json, write_bank};

#[test]
fn a_shared_node_reports_original_values_and_byte_receipts() -> TestResult {
    let directory = tempfile::tempdir()?;
    let bank = TestBank::new(19)
        .object(TestHircObject::container(
            TestContainerKind::Blend,
            9,
            &[7],
            TestNodeBase::Full,
        ))
        .object(sound_without_node());
    let bytes = bank.bytes();
    let file = write_bank(directory.path(), &bank)?;
    let report = json(&[], &file)?;
    let object = report
        .pointer("/objects/0")
        .ok_or("selected Blend object")?;
    let node = object.get("node_fields").ok_or("shared node declaration")?;
    assert_eq!(
        node.pointer("/object_status/value"),
        Some(&serde_json::json!("decoded"))
    );
    assert_eq!(
        node.pointer("/version/value"),
        Some(&serde_json::json!(154))
    );
    let parent = object
        .get("parent")
        .and_then(Value::as_u64)
        .ok_or("legacy parent")?;
    let parent = u32::try_from(parent)?;
    check_word(node, "/parent_id", &parent.to_le_bytes(), &bytes)?;
    check_word(node, "/priority_flags", &[0], &bytes)?;
    check_word(
        node,
        "/properties/rows/0/scalar_bits",
        &3.0_f32.to_bits().to_le_bytes(),
        &bytes,
    )?;
    assert_eq!(
        node.pointer("/ranged_modifiers/coverage/declared_count/value"),
        Some(&serde_json::json!(1)),
        "authored ranged list"
    );
    let absent = report
        .pointer("/objects/1/node_fields")
        .ok_or("Sound observation")?;
    assert!(
        absent.get("properties").is_none(),
        "the Sound has no node properties"
    );
    assert!(
        absent.get("priority_flags").is_none(),
        "no flags were declared"
    );
    Ok(())
}

fn sound_without_node() -> TestHircObject {
    let mut body = 0x0004_0001_u32.to_le_bytes().to_vec();
    body.push(2);
    body.extend_from_slice(&11_u32.to_le_bytes());
    body.extend_from_slice(&0_u32.to_le_bytes());
    body.extend_from_slice(&4000_u32.to_le_bytes());
    body.push(0);
    TestHircObject::raw(2, 10, body)
}

fn check_word(node: &Value, path: &str, expected: &[u8], original: &[u8]) -> TestResult {
    let field = node.pointer(path).ok_or("sourced field")?;
    assert_eq!(
        field.get("containsEncryptedSource"),
        Some(&Value::Null),
        "loose files have no archive encryption declaration"
    );
    let receipts = node
        .pointer("/source_inventory/uses")
        .and_then(Value::as_array)
        .ok_or("source inventory")?;
    let references = field
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or("source references")?;
    let [reference] = references.as_slice() else {
        return Err("expected exactly one physical receipt".into());
    };
    let indices = receipts
        .iter()
        .find(|receipt| receipt.get("id") == Some(reference))
        .and_then(|receipt| receipt.pointer("/location/indices"))
        .and_then(Value::as_array)
        .ok_or("receipt offsets")?;
    let [start, end] = indices.as_slice() else {
        return Err("expected exactly two original offsets".into());
    };
    let offset = |value: &Value| -> Result<usize, Box<dyn std::error::Error>> {
        Ok(value.as_str().ok_or("decimal offset")?.parse()?)
    };
    assert_eq!(
        original.get(offset(start)?..offset(end)?),
        Some(expected),
        "{path}"
    );
    Ok(())
}
