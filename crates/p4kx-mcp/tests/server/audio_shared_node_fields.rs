// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{TestBank, TestContainerKind, TestEntry, TestHircObject, TestNodeBase};
use serde_json::json;

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
    for (pointer, value) in [
        ("/version", json!(154)),
        ("/object_status", json!("decoded")),
        ("/priority_flags", json!(0)),
        ("/properties/rows/0/scalar_bits", json!(3.0_f32.to_bits())),
    ] {
        let datum = node.pointer(pointer).ok_or("sourced node field")?;
        assert_eq!(datum.get("value"), Some(&value), "{pointer}");
        assert_eq!(
            datum.get("containsEncryptedSource"),
            Some(&json!(true)),
            "{pointer}"
        );
    }
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
