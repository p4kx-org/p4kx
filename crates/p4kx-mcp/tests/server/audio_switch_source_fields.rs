// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{TestBank, TestContainerKind, TestEntry, TestHircObject, TestNodeBase};
use serde_json::{Value, json};

use crate::audio::serve;
use crate::support::{Install, TestResult};

#[test]
fn switch_mcp_reports_the_actual_encrypted_state_and_configuration_words() -> TestResult {
    let path = "Data/Sounds/wwise/Switch.bnk";
    let bytes = TestBank::new(9)
        .version(154)
        .object(TestHircObject::container(
            TestContainerKind::Switch,
            6,
            &[10, 11],
            TestNodeBase::Plain,
        ))
        .bytes();
    let install = Install::new(vec![TestEntry::zstd(path, bytes).encrypted()])?;
    let mut server = serve(&install)?;
    let result = server.ok(
        "list_soundbank_objects",
        &json!({"soundbank": path, "id": 6}),
    )?;
    let source = result
        .pointer("/objects/0/source_fields")
        .ok_or("original Switch fields")?;
    assert_eq!(
        source.pointer("/fields/group_kind/value"),
        Some(&json!("state")),
        "original known state code"
    );
    assert_eq!(
        source.pointer("/fields/group_kind/containsEncryptedSource"),
        Some(&json!(true)),
        "direct derived contribution"
    );
    let rows = source
        .get("rows")
        .and_then(Value::as_array)
        .ok_or("Switch source rows")?;
    let config = rows
        .iter()
        .find(|row| row.get("kind") == Some(&json!("configuration")))
        .ok_or("checked configuration")?;
    assert_eq!(
        config.pointer("/fade_in/signed_value/value"),
        Some(&json!(0)),
        "explicit zero fade"
    );
    assert_eq!(
        config.pointer("/fade_in/bits/containsEncryptedSource"),
        Some(&json!(true)),
        "exact encrypted word"
    );
    let schema = server
        .tools()?
        .into_iter()
        .find(|tool| tool["name"] == "list_soundbank_objects")
        .ok_or("soundbank objects tool")?;
    assert_eq!(
        schema.pointer("/outputSchema/$defs/SwitchObjectSourceFields/properties/rows/type"),
        Some(&json!("array")),
        "the shared MCP schema describes the complete source rows",
    );
    Ok(())
}
