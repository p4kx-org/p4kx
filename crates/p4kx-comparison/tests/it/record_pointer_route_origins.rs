// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};
use serde_json::Value;

use super::record_body_origins::{field_path, raw_bytes, reports, uses_at};

#[test]
fn a_changed_owned_value_retains_the_pointer_as_selector_only() -> Result<(), Box<dyn Error>> {
    let before = database(24);
    let after = database(77);
    let (report, _) = reports(&before, &after)?;
    let path = field_path(&report, "owned.weight")?;
    let uses = uses_at(&report, &format!("{path}/new/value"), Some(false), true)?;
    let (_, pointer) = raw_bytes(&after, &uses, "pointerSelector", "selector")?;
    assert_eq!(
        pointer,
        [1, 0, 0, 0, 0, 0, 0, 0],
        "the original pointer selects Child definition 1 and instance 0",
    );
    let (_, value) = raw_bytes(&after, &uses, "slot", "payload")?;
    assert_eq!(value, 77_i32.to_le_bytes(), "sole authored scalar");
    let payload_spans = uses
        .iter()
        .filter(|site| {
            site.pointer("/location/namespace").and_then(Value::as_str)
                == Some("dataforgeDecodedBytes")
                && site.get("role").and_then(Value::as_str) == Some("payload")
        })
        .count();
    assert_eq!(
        payload_spans, 1,
        "the route never contributes its bytes as payload"
    );
    Ok(())
}

fn database(weight: i32) -> Vec<u8> {
    DataForgeBuilder::new()
        .structure(TestStruct::new("Root").field("owned", TestType::StrongPointer("Child".into())))
        .structure(TestStruct::new("Child").field("weight", TestType::Int32))
        .record(TestRecord::new(1, "Root", "Root.One", "root.xml").value(
            "owned",
            TestValue::Struct(TestInstance::new("Child").value("weight", TestValue::Int32(weight))),
        ))
        .build()
}
