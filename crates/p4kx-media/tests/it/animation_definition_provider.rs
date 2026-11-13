// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;

use p4kx_extraction::{Archive, OpenOptions};
use p4kx_media::{ExportOptions, MediaKind, MediaRecord, Status};
use p4kx_testkit::{
    ArchiveBuilder, CryXmlByteOrder, CryXmlSignature, TestElement, TestEntry, test_key,
};
use serde_json::Value;

use super::support::{self, TestResult};

const STREAM: &str = "animation-definition.jsonl";
const PARAMS: &[u8] = br##"<?fixture preserve?><Params xmlns:x="urn:fixture"><!--kept-->
<AnimationList><Animation name="#filepath" path="Animations/Fixture"/>
<Animation name="$TracksDatabase" path="ANIMATIONS\FIXTURE\sample.dba"/>
<Animation name="$AnimEventDatabase" path="Animations/Fixture/events.animevents"/>
<Animation name="$AnimEventDatabase" path="../host.animevents"/>
<Animation name="open" path="open.caf"/><Animation name="open" path="variant.caf"/>
</AnimationList><x:Unknown x:value="a&amp;b">text</x:Unknown></Params>"##;

#[test]
fn definitions_preserve_original_xml_and_independent_link_evidence() -> TestResult {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("Data.p4k");
    let events = TestElement::new("anim_event_list").child(
        TestElement::new("animation")
            .attribute("name", "open.caf")
            .child(
                TestElement::new("event")
                    .attribute("time", "0.25")
                    .attribute("name", "fixture"),
            ),
    );
    let binary = events.to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Little);
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Objects/fixture.chrparams", PARAMS).encrypted())
        .entry(TestEntry::stored("Data/Animations/Fixture/sample.dba", b"not selected"))
        .entry(TestEntry::stored("Data/Animations/Fixture/events.animevents", binary))
        .entry(TestEntry::stored("Data/Animations/Fixture/graph.adb", br#"<AnimDB><SubADBs>
            <SubADB File="Animations/Fixture/graph.adb"/><SubADB File="Animations/Fixture/missing.adb"/>
            </SubADBs></AnimDB>"#))
        .write_to(&path)?;
    let archive = Archive::open_with(path, OpenOptions::new().key(Some(test_key())))?;
    let output = folder.path().join("out");
    let options = ExportOptions {
        kinds: vec![MediaKind::AnimationDefinition],
        ..ExportOptions::default()
    };
    let index = support::published(p4kx_media::export(&archive, &output, &options)?)?;
    let rows = support::records(&output, &index, STREAM)?;
    let schema = p4kx_media::builtin_record_schema()?;
    let validator = jsonschema::validator_for(&schema)?;
    for row in &rows {
        validator
            .validate(&serde_json::to_value(row)?)
            .map_err(|error| error.to_string())?;
        assert_eq!(row.status, Status::Complete);
    }
    verify_links(&rows);
    verify_source(&rows, &output.join(index.artifact_base))?;
    Ok(())
}

fn verify_links(rows: &[MediaRecord]) {
    let states: Vec<_> = rows
        .iter()
        .filter(|row| row.data.get("record_type").and_then(Value::as_str) == Some("link"))
        .filter_map(|row| {
            row.data
                .pointer("/payload/reference/resolution/status")
                .and_then(Value::as_str)
        })
        .collect();
    assert_eq!(
        states,
        [
            "not-selected",
            "resolved",
            "unavailable",
            "resolved",
            "missing"
        ],
        "ordered source link dispositions"
    );
}

fn verify_source(rows: &[MediaRecord], base: &std::path::Path) -> TestResult {
    let params = rows
        .iter()
        .find(|row| {
            row.data.pointer("/payload/family").and_then(Value::as_str) == Some("chrparams")
        })
        .ok_or("character params row")?;
    let raw = params.source.raw.as_ref().ok_or("retained params")?;
    assert_eq!(
        fs::read(base.join(&raw.path))?,
        PARAMS,
        "original XML bytes"
    );
    for (pointer, expected) in [
        ("/payload/qualified_name", "x:Unknown"),
        ("/payload/lexeme", "a&amp;b"),
        ("/payload/position/type", "cry-xml-attribute"),
        ("/payload/content/target", "fixture"),
    ] {
        assert!(
            rows.iter()
                .any(|row| row.data.pointer(pointer).and_then(Value::as_str) == Some(expected)),
            "source field {pointer}={expected}"
        );
    }
    Ok(())
}

#[test]
fn malformed_animation_definition_retains_bytes_in_diagnostic_output() -> TestResult {
    let scratch = tempfile::tempdir()?;
    let invalid = b"<Params><Broken>";
    let archive = support::archive_with(
        scratch.path(),
        [TestEntry::stored("Data/Objects/invalid.chrparams", invalid)],
    )?;
    let output = scratch.path().join("out");
    let outcome = p4kx_media::export(
        &archive,
        &output,
        &ExportOptions {
            diagnostic: true,
            kinds: vec![MediaKind::AnimationDefinition],
            ..ExportOptions::default()
        },
    )?;
    let p4kx_media::ExportOutcome::Incomplete { diagnostic, .. } = outcome else {
        return Err("a malformed definition has incomplete diagnostic output".into());
    };
    let base = diagnostic.parent().ok_or("diagnostic base")?;
    let rows = fs::read_to_string(base.join(STREAM))?;
    let Some(line) = rows.lines().next() else {
        return Err("diagnostic record missing".into());
    };
    let row: MediaRecord = serde_json::from_str(line)?;
    assert_eq!(row.status, Status::Failed);
    assert_eq!(
        row.data
            .pointer("/payload/projection")
            .and_then(Value::as_str),
        Some("invalid-source")
    );
    assert!(
        row.data
            .pointer("/payload/nodes")
            .is_some_and(Value::is_null)
    );
    assert!(!output.join("media-index.json").exists());
    let raw = row
        .source
        .raw
        .as_ref()
        .ok_or("retained diagnostic source")?;
    assert_eq!(fs::read(base.join(&raw.path))?, invalid);
    Ok(())
}
