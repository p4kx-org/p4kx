// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataForge, Guid, Value as OriginalValue};
use p4kx_testkit::{ArchiveBuilder, TestEntry};
use serde_json::Value;

use super::{TestResult, database, run, tag, text};

const TAGS: usize = 80;
const FIRST: u128 = 0x200;
const NAME: &str = "Tag\"\\é";

#[test]
fn nested_cli_json_keeps_every_original_tag_and_escaped_path() -> TestResult {
    let bytes = chain();
    original(&bytes)?;
    let directory = tempfile::tempdir()?;
    let archive = directory.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::zstd("Data\\Game2.dcb", bytes).encrypted())
        .write_to(&archive)?;
    let guid = text(FIRST);
    let output = run("tags", &archive, &[&guid, "--tree", "--json"], 0)?;
    let (head, nodes) = output
        .stdout
        .split_once(",\"tags\":[")
        .ok_or("nested tags array")?;
    let head: Value = serde_json::from_str(&format!("{head}}}"))?;
    assert_eq!(
        head.get("tree").and_then(Value::as_bool),
        Some(true),
        "the CLI emits the tree envelope"
    );
    assert_eq!(
        head.get("shown_count").and_then(Value::as_u64),
        Some(u64::try_from(TAGS)?),
        "the envelope counts every selected Tag"
    );
    assert_eq!(
        head.pointer("/issues/too_deep").and_then(Value::as_u64),
        Some(0),
        "the compatibility depth count remains zero"
    );
    assert_eq!(
        head.pointer("/issues/too_long").and_then(Value::as_u64),
        Some(0),
        "the compatibility path length count remains zero"
    );
    assert_nodes(nodes)
}

fn chain() -> Vec<u8> {
    let mut builder = database();
    for position in 0..TAGS {
        let guid = FIRST + position as u128;
        let next = [guid + 1];
        let children = if position + 1 == TAGS { &[][..] } else { &next };
        builder = builder.record(tag(guid, NAME, 7, children));
    }
    builder.build()
}

fn original(bytes: &[u8]) -> TestResult {
    let forge = DataForge::open(bytes)?;
    let guid: Guid = text(FIRST).parse()?;
    let first = forge
        .records()
        .find(|record| record.guid() == guid)
        .ok_or("original root")?;
    for position in 0..TAGS {
        let index = first.index() + u32::try_from(position)?;
        let record = forge.record(index).ok_or("actual physical tag")?;
        assert_eq!(
            record.guid(),
            text(FIRST + position as u128).parse::<Guid>()?,
            "the original Tag occupies the expected physical Record"
        );
        let tree = record.tree()?;
        assert_eq!(
            tree.error_count(),
            0,
            "the original Tag has no read failure"
        );
        assert!(
            matches!(tree.field("tagName"), Some(OriginalValue::String(NAME))),
            "the original name contains the actual escaped characters"
        );
        let Some(OriginalValue::Array(children)) = tree.field("children") else {
            return Err("original child array".into());
        };
        if position + 1 == TAGS {
            assert!(children.is_empty(), "the last original Tag has no children");
        } else {
            let [OriginalValue::Reference(child)] = children.as_ref() else {
                return Err("one genuine original child".into());
            };
            assert_eq!(
                child.record_index(),
                index + 1,
                "the authored relation points at the next physical Tag"
            );
        }
    }
    Ok(())
}

fn assert_nodes(mut nodes: &str) -> TestResult {
    for position in 0..TAGS {
        let (head, children) = nodes
            .split_once(",\"children\":[")
            .ok_or("next nested child")?;
        let node: Value = serde_json::from_str(&format!("{head}}}"))?;
        assert_eq!(
            node.get("guid").and_then(Value::as_str),
            Some(text(FIRST + position as u128).as_str()),
            "the emitted node retains the exact original GUID"
        );
        assert_eq!(
            node.get("name").and_then(Value::as_str),
            Some(NAME),
            "the CLI preserves the complete escaped name"
        );
        assert_eq!(
            node.get("path").and_then(Value::as_str),
            Some(vec![NAME; position + 1].join(".").as_str()),
            "the CLI preserves the full original path"
        );
        assert_eq!(
            node.get("legacy_id").and_then(Value::as_u64),
            Some(7),
            "each nested Tag retains its original legacy identifier"
        );
        assert!(
            node.get("parent").is_none() && node.get("depth").is_none(),
            "nested output keeps parent and depth implicit"
        );
        nodes = children;
    }
    assert_eq!(
        nodes,
        format!("{}]}}\n", "]}".repeat(TAGS)),
        "every selected child list and object is closed"
    );
    Ok(())
}
