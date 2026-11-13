// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{TestBank, TestContainerKind, TestEntry, TestHircObject, TestNodeBase};
use serde_json::{Value, json};

use crate::audio::serve;
use crate::support::{Install, TestResult};

#[test]
fn shared_node_fields_mcp_retains_encrypted_property_words_and_schema() -> TestResult {
    let path = "Data/Sounds/wwise/Shared.bnk";
    let bank = TestBank::new(19)
        .object(TestHircObject::container(
            TestContainerKind::Blend,
            9,
            &[7],
            TestNodeBase::Full,
        ))
        .bytes();
    let install = Install::new(vec![TestEntry::zstd(path, bank).encrypted()])?;
    let mut server = serve(&install)?;
    let result = server.ok(
        "list_soundbank_objects",
        &json!({"soundbank": path, "id": 9}),
    )?;
    let object = result
        .pointer("/objects/0")
        .ok_or("selected Blend object")?;
    assert_eq!(
        object.pointer("/source_fields/rows/0/child_id/value"),
        Some(&json!(7)),
        "type-specific declarations remain available"
    );
    let node = object.get("node_fields").ok_or("shared node declaration")?;
    verify_fields(node)?;
    let schema = server
        .tools()?
        .into_iter()
        .find(|tool| tool.get("name") == Some(&json!("list_soundbank_objects")))
        .ok_or("soundbank objects tool")?;
    assert_eq!(
        schema.pointer("/outputSchema/$defs/NodePropertyList/properties/rows/type"),
        Some(&json!("array")),
        "typed shared node schema"
    );
    Ok(())
}

fn verify_fields(node: &Value) -> TestResult {
    for (pointer, value) in [
        ("/version", json!(154)),
        ("/object_status", json!("decoded")),
        ("/priority_flags", json!(0)),
        ("/effects/slots/0/effect_id", json!(0x3ECB_429B_u32)),
        (
            "/metadata_effects/slots/0/effect_id",
            json!(0x0123_4567_u32),
        ),
        ("/aux/user_bus_ids/0", json!(0x6300_0001_u32)),
        ("/aux/reflections_bus_id", json!(0)),
        ("/advanced/virtual_queue_behavior", json!(1)),
        ("/positioning/transition_time", json!(1000)),
        ("/positioning/rows/0/fields/coordinates/x_bits", json!(0)),
        (
            "/inline_state/rows/4/properties/0/value_bits",
            json!(96.0_f32.to_bits()),
        ),
        ("/rtpc/rows/0/rtpc_id", json!(0x984F_8A08_u32)),
        ("/rtpc/rows/1/from_bits", json!(0)),
        ("/properties/rows/0/scalar_bits", json!(3.0_f32.to_bits())),
        (
            "/properties/rows/0/interpretation/property",
            json!("volume-decibels"),
        ),
        ("/properties/rows/0/interpretation/scalar", json!(3.0)),
        (
            "/ranged_modifiers/rows/0/interpretation/property",
            json!("low-pass-filter-percent"),
        ),
    ] {
        let datum = node.pointer(pointer).ok_or("sourced node field")?;
        assert_eq!(datum.get("value"), Some(&value), "{pointer}");
        assert_eq!(
            datum.get("containsEncryptedSource"),
            Some(&json!(true)),
            "{pointer}"
        );
    }
    Ok(())
}
