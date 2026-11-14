// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{TestBank, TestContainerKind, TestEntry, TestHircObject, TestNodeBase};
use serde_json::json;

use crate::audio::serve;
use crate::support::{Install, TestResult};

#[test]
fn blend_source_fields_mcp_exposes_encrypted_original_children() -> TestResult {
    let path = "Data/Sounds/wwise/Blend.bnk";
    let original = TestBank::new(19)
        .version(154)
        .object(TestHircObject::container(
            TestContainerKind::Blend,
            9,
            &[7, 8],
            TestNodeBase::Plain,
        ))
        .bytes();
    let install = Install::new(vec![TestEntry::zstd(path, original).encrypted()])?;
    let mut server = serve(&install)?;
    let result = server.ok(
        "list_soundbank_objects",
        &json!({"soundbank": path, "id": 9}),
    )?;
    let source = result
        .pointer("/objects/0/source_fields")
        .ok_or("original Blend fields")?;
    assert_eq!(
        source.pointer("/fields/layers/declared_count/value"),
        Some(&json!(1))
    );
    assert_eq!(
        source.pointer("/fields/continuous_validation/containsEncryptedSource"),
        Some(&json!(true))
    );
    assert_eq!(source.pointer("/rows/1/child_id/value"), Some(&json!(8)));
    assert_eq!(
        source.pointer("/rows/1/child_id/containsEncryptedSource"),
        Some(&json!(true))
    );
    let schema = server
        .tools()?
        .into_iter()
        .find(|tool| tool.get("name") == Some(&json!("list_soundbank_objects")))
        .ok_or("soundbank objects tool")?;
    assert_eq!(
        schema.pointer("/outputSchema/$defs/BlendObjectSourceFields/properties/rows/type"),
        Some(&json!("array"))
    );
    Ok(())
}
