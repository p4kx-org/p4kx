// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_dataforge::{DataForge, JsonLayout, RecordJson, RecordXml};
use serde_json::Value;

use crate::owned_source_controls as controls;
use crate::owned_source_fixture::{self as fixture, TestResult};

#[test]
fn json_and_xml_keep_every_valid_owned_level_and_final_null() -> TestResult {
    let bytes = fixture::database(1, 65);
    let forge = DataForge::open(&bytes)?;
    let record = controls::original(&forge, 1, 65)?;
    let tree = record.tree()?;
    controls::complete(&tree, 1, 65)?;
    let mut json = Vec::new();
    RecordJson::new(JsonLayout::Compact).write(&tree, &mut json)?;
    let document: Value = serde_json::from_slice(&json)?;
    check_json(&document)?;
    let mut xml = Vec::new();
    RecordXml::new().write(&tree, &mut xml)?;
    let text = std::str::from_utf8(&xml)?;
    let document = roxmltree::Document::parse(text)?;
    let nodes: Vec<_> = document
        .descendants()
        .filter(|node| node.has_tag_name("Node"))
        .collect();
    assert_eq!(nodes.len(), 65, "XML retains every original owned struct");
    assert_eq!(
        nodes.last().and_then(|node| node.attribute("mark")),
        Some("65"),
        "the actual last original scalar reaches XML"
    );
    assert!(
        !text.contains("p4kx:error"),
        "valid source nesting creates no XML error element"
    );
    Ok(())
}

fn check_json(document: &Value) -> TestResult {
    let mut current = document
        .pointer("/roots/0")
        .ok_or("the original root slot")?;
    for mark in 1..=65 {
        assert_eq!(
            current.get("$type"),
            Some(&Value::from("Node")),
            "the actual original struct type"
        );
        assert_eq!(
            current.get("mark"),
            Some(&Value::from(mark)),
            "all original JSON scalars survive"
        );
        current = current
            .get("next")
            .ok_or("the actual original next declaration")?;
    }
    assert_eq!(
        current,
        &Value::Null,
        "the final JSON null is the original terminal pointer"
    );
    Ok(())
}
