// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_media::{ExportOptions, MediaKind};
use p4kx_testkit::TestEntry;
use serde_json::Value;

use super::support::{self, TestResult};

#[test]
fn mannequin_header_links_keep_untyped_physical_targets_and_missing_references() -> TestResult {
    let temporary = tempfile::tempdir()?;
    let source = br#"<AnimDB TagDef="Animations/Mannequin/tags.xml"
                            FragDef="Data\Animations\Mannequin\missing.xml"
                            xmlns:x="urn:fixture" x:TagDef="shadow.xml">
                       <FragmentList><Node FragDef="unrelated.xml"/></FragmentList>
                     </AnimDB>"#;
    let archive = support::archive_with(
        temporary.path(),
        [
            TestEntry::stored("Data/Animations/Mannequin/sample.adb", source),
            TestEntry::stored("Data/Animations/Mannequin/TAGS.XML", b"unselected bytes"),
        ],
    )?;
    let output = temporary.path().join("out");
    let options = ExportOptions {
        kinds: vec![MediaKind::AnimationDefinition],
        ..ExportOptions::default()
    };
    let index = support::published(p4kx_media::export(&archive, &output, &options)?)?;
    let rows = support::records(&output, &index, "animation-definition.jsonl")?;
    let validator = jsonschema::validator_for(&p4kx_media::builtin_record_schema()?)?;
    for row in &rows {
        validator
            .validate(&serde_json::to_value(row)?)
            .map_err(|error| error.to_string())?;
    }
    let links: Vec<_> = rows
        .iter()
        .filter(|row| row.data.get("record_type").and_then(Value::as_str) == Some("link"))
        .collect();
    let [tag, fragment] = links.as_slice() else {
        return Err("expected two unqualified root declarations".into());
    };
    for (row, kind, status) in [
        (tag, "tag-definition", "resolved"),
        (fragment, "fragment-definition", "missing"),
    ] {
        assert_eq!(
            row.data.pointer("/payload/kind").and_then(Value::as_str),
            Some(kind),
            "distinct declared link role"
        );
        assert_eq!(
            row.data
                .pointer("/payload/reference/resolution/status")
                .and_then(Value::as_str),
            Some(status),
            "exact archive directory resolution"
        );
    }
    assert!(
        tag.data
            .pointer("/payload/reference/resolution/target/kind")
            .is_some_and(Value::is_null),
        "directory discovery keeps the XML provider unassigned"
    );
    Ok(())
}
