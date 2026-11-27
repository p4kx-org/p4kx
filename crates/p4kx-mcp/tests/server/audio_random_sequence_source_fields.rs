// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{TestBank, TestContainerKind, TestEntry, TestHircObject, TestNodeBase};
use serde_json::json;

use crate::audio::serve;
use crate::support::{Install, TestResult};

#[test]
fn encrypted_random_sequence_mcp_exposes_direct_original_weights() -> TestResult {
    let path = "Data/Sounds/wwise/Random.bnk";
    let bytes = TestBank::new(9)
        .version(154)
        .object(TestHircObject::container(
            TestContainerKind::Random,
            10,
            &[20, 21],
            TestNodeBase::Plain,
        ))
        .bytes();
    let install = Install::new(vec![TestEntry::zstd(path, bytes).encrypted()])?;
    let mut server = serve(&install)?;
    let result = server.ok(
        "list_soundbank_objects",
        &json!({"soundbank": path, "id": 10}),
    )?;
    let source = result
        .pointer("/objects/0/source_fields")
        .ok_or("original playlist metadata")?;
    assert_eq!(
        source.pointer("/fields/mode/value"),
        Some(&json!(0)),
        "random mode"
    );
    assert_eq!(
        source.pointer("/rows/0/play_id/value"),
        Some(&json!(20)),
        "original playlist order"
    );
    assert_eq!(
        source.pointer("/rows/0/weight/value"),
        Some(&json!(50_000)),
        "original weight"
    );
    assert_eq!(
        source.pointer("/rows/0/weight/containsEncryptedSource"),
        Some(&json!(true)),
        "direct encrypted source contribution"
    );
    let schema = server
        .tools()?
        .into_iter()
        .find(|tool| tool["name"] == "list_soundbank_objects")
        .ok_or("soundbank objects tool")?;
    assert_eq!(
        schema.pointer("/outputSchema/$defs/RandomSequenceObjectSourceFields/properties/rows/type"),
        Some(&json!("array")),
        "the MCP schema describes complete Random/Sequence source rows",
    );
    Ok(())
}
