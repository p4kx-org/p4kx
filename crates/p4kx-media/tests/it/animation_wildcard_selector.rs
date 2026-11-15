// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::path::Path;

use p4kx_extraction::{Archive, Filter, OpenOptions};
use p4kx_media::{ExportOptions, MediaKind, MediaRecord};
use p4kx_testkit::{ArchiveBuilder, TestEntry, test_key};
use serde_json::Value;

use super::support::{self, TestResult};

const SOURCE: &str = "Data/Objects/wildcard.chrparams";
const STREAM: &str = "animation-definition.jsonl";
const PARAMS: &[u8] = br##"<Params>
<AnimationList>
    <Animation Name="hero*" Path="Animations/A/single.caf"/>
    <Animation Name="#FilePath" Path="Animations\A\"/>
    <Animation Name="gesture" Path="*.caf"/>
    <Animation Name="#filepath"/>
    <Animation Name="gesture?" Path="*.caf"/>
    <Animation Name="#filepath" Path=""/>
    <Animation Name="reset?" Path="*/*.caf"/>
    <Animation Name="ambiguous*" Path="one.caf" path="two.caf"/>
</AnimationList>
<AnimationList><Animation Name="other*" Path="other.caf"/></AnimationList>
</Params>"##;

#[test]
fn wildcard_rows_keep_original_spelling_positions_and_scope_without_candidates() -> TestResult {
    let temp = tempfile::tempdir()?;
    let archive_path = temp.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored(SOURCE, PARAMS).encrypted())
        .entry(TestEntry::stored("Data/Animations/A/single.caf", b"decoy"))
        .write_to(&archive_path)?;
    let archive = Archive::open_with(&archive_path, OpenOptions::new().key(Some(test_key())))?;
    let output = temp.path().join("out");
    let options = ExportOptions {
        kinds: vec![MediaKind::AnimationDefinition],
        include: Some(Filter::parse(SOURCE)?),
        ..ExportOptions::default()
    };
    let index = support::published(p4kx_media::export(&archive, &output, &options)?)?;
    let rows = support::records(&output, &index, STREAM)?;
    let patterns: Vec<_> = rows
        .iter()
        .enumerate()
        .filter(|(_, row)| row.subresource.namespace == "animation-definition-wildcard")
        .collect();
    let base = output.join(index.artifact_base);
    verify_patterns(&rows, &patterns)?;
    let (_, first) = patterns.first().ok_or("wildcard source")?;
    let raw = first.source.raw.as_ref().ok_or("retained wildcard XML")?;
    assert_eq!(
        fs::read(base.join(&raw.path))?,
        PARAMS,
        "original source bytes"
    );
    let (ordinal, _) = patterns.get(1).ok_or("wildcard with authored folder")?;
    verify_origins(&base, *ordinal)
}

fn verify_patterns(rows: &[MediaRecord], patterns: &[(usize, &MediaRecord)]) -> TestResult {
    let expected = [
        ("hero*", "Animations/A/single.caf", "missing", None),
        ("gesture", "*.caf", "authored", Some("Animations\\A\\")),
        ("gesture?", "*.caf", "malformed", None),
        ("reset?", "*/*.caf", "reset", Some("")),
        ("other*", "other.caf", "missing", None),
    ];
    assert_eq!(
        patterns.len(),
        expected.len(),
        "all valid declarations in order"
    );
    let validator = jsonschema::validator_for(&p4kx_media::builtin_record_schema()?)?;
    for ((_, row), (name, path, scope, folder)) in patterns.iter().zip(expected) {
        validator
            .validate(&serde_json::to_value(row)?)
            .map_err(|error| error.to_string())?;
        for (field, expected) in [
            ("state", Some("unexpanded")),
            ("name/qualified_name", Some("Name")),
            ("name/value", Some(name)),
            ("path/qualified_name", Some("Path")),
            ("path/value", Some(path)),
            ("preceding_filepath/state", Some(scope)),
            ("preceding_filepath/path/value", folder),
        ] {
            assert_eq!(
                row.data
                    .pointer(&format!("/payload/{field}"))
                    .and_then(Value::as_str),
                expected,
                "original wildcard field {field}",
            );
        }
        let original = rows
            .iter()
            .find(|item| {
                item.subresource.namespace == "animation-definition-declaration"
                    && item.subresource.indices == row.subresource.indices
            })
            .ok_or("ordered original wildcard path declaration")?;
        assert_eq!(
            original.data.pointer("/payload/position"),
            row.data.pointer("/payload/path/position"),
            "wildcard path identity equals its XML attribute identity",
        );
        assert!(
            row.data.pointer("/payload/reference").is_none()
                && row.data.pointer("/payload/archive_path").is_none(),
            "unexpanded selector has no resolved path or candidate claims",
        );
    }
    Ok(())
}

fn verify_origins(base: &Path, ordinal: usize) -> TestResult {
    let ordinal = ordinal.to_string();
    let origins = fs::read_to_string(base.join("field-origins.jsonl"))?;
    let mut checked = 0;
    for line in origins.lines() {
        let origin: Value = serde_json::from_str(line)?;
        if origin.pointer("/scope/stream/kind").and_then(Value::as_str)
            != Some("animation-definition")
            || origin.pointer("/scope/ordinal").and_then(Value::as_str) != Some(ordinal.as_str())
            || !matches!(
                origin.get("field").and_then(Value::as_str),
                Some(
                    "/data/payload/name/value"
                        | "/data/payload/path/value"
                        | "/data/payload/preceding_filepath/path/value"
                )
            )
        {
            continue;
        }
        assert_eq!(
            origin
                .get("containsEncryptedSource")
                .and_then(Value::as_bool),
            Some(true),
            "original encrypted declaration accepted as actual evidence",
        );
        assert_eq!(
            origin
                .get("sourceUsesRefs")
                .and_then(Value::as_array)
                .map(Vec::len),
            Some(1),
            "the value has its own exact selector coordinate",
        );
        checked += 1;
    }
    assert_eq!(
        checked, 3,
        "wildcard name, path and folder each have evidence"
    );
    Ok(())
}
