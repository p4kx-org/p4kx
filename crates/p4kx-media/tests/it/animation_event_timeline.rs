// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, OpenOptions};
use p4kx_media::{ExportOptions, MediaKind, MediaRecord, Status};
use p4kx_testkit::{
    ArchiveBuilder, CryXmlByteOrder, CryXmlSignature, TestElement, TestEntry, test_key,
};
use serde_json::{Value, json};

use super::support::{self, TestResult};

const STREAM: &str = "animation-definition.jsonl";
const SOURCE: &str = "Data/Animations/Fixture/test.animevents";

#[test]
fn timelines_keep_exact_paths_order_and_unknown_event_attributes() -> TestResult {
    let scratch = tempfile::tempdir()?;
    let bytes = br#"<anim_event_list xmlns:x="urn:event">
      <animation name="Animations/A/move.caf">
        <event name="cue" time="0.25" endTime="0.5" seekTime="" bone="joint"
          offset="1,2,3" dir="0,1,0" parameter="a&amp;b" model="" dropInZone="0"
          stopOnInterrupt="true" x:new="1"/>
        <event name="cue" time="0.25"/>
        <container><event time="three"/></container>
      </animation>
      <animation name="Data/Animations/B/move.caf"/>
      <animation name="../external.caf"/>
      <animation name="Animations/A/missing.caf"/>
      <animation/>
      <x:animation name="Animations/B/move.caf"><event/></x:animation>
    </anim_event_list>"#;
    let archive = support::archive_with(
        scratch.path(),
        [
            TestEntry::stored(SOURCE, bytes),
            TestEntry::stored("Data/Animations/A/move.caf", b"raw a"),
            TestEntry::stored("Data/Animations/B/move.caf", b"raw b"),
        ],
    )?;
    let (_, rows) = export(&archive, scratch.path())?;
    assert_plain_timelines(&rows)?;
    assert_attributes(&records(&rows, "event-attribute"))?;
    verify_schema(&rows)
}

fn assert_plain_timelines(rows: &[MediaRecord]) -> TestResult {
    let timelines = records(rows, "event-timeline");
    assert_eq!(
        timelines.len(),
        5,
        "one row per direct unqualified animation"
    );
    let expected = [
        ("Data/Animations/A/move.caf", "not-selected"),
        ("Data/Animations/B/move.caf", "not-selected"),
        ("", "unavailable"),
        ("Data/Animations/A/missing.caf", "missing"),
    ];
    for (row, (path, status)) in timelines.iter().zip(expected) {
        let archive_path = row.data.pointer("/payload/target/archive_path");
        assert_eq!(
            archive_path.and_then(Value::as_str).unwrap_or_default(),
            path,
            "target path uses the exact directory"
        );
        assert_eq!(
            row.data
                .pointer("/payload/target/reference/resolution/status")
                .and_then(Value::as_str),
            Some(status),
            "physical target disposition"
        );
    }
    assert_eq!(
        timelines
            .last()
            .and_then(|row| row.data.pointer("/payload/target")),
        Some(&json!({"state":"missing-name"})),
        "missing selector remains explicit"
    );
    let events = records(rows, "event");
    let [first, second] = events.as_slice() else {
        return Err("expected exactly two direct events".into());
    };
    assert_ne!(
        first.subresource, second.subresource,
        "each physical event has its own identity"
    );
    let identity = timelines
        .first()
        .and_then(|row| row.subresource.indices.first())
        .ok_or("first timeline has a source index")?
        .to_string();
    assert!(
        events.iter().all(|row| row
            .data
            .pointer("/payload/animation_element")
            .and_then(Value::as_str)
            == Some(identity.as_str())),
        "both event occurrences attach to the exact same timeline"
    );
    Ok(())
}

fn assert_attributes(attributes: &[&MediaRecord]) -> TestResult {
    let fields: Vec<_> = attributes
        .iter()
        .filter_map(|row| row.data.pointer("/payload/field").and_then(Value::as_str))
        .collect();
    assert_eq!(
        fields,
        [
            "name",
            "time",
            "end_time",
            "seek_time",
            "bone",
            "offset",
            "direction",
            "parameter",
            "model",
            "drop_in_zone",
            "stop_on_interrupt",
            "unknown",
            "name",
            "time",
        ],
        "physical attribute order and duplicate event kept"
    );
    let parameter = attributes.get(7).ok_or("parameter attribute")?;
    assert_eq!(
        parameter
            .data
            .pointer("/payload/value")
            .and_then(Value::as_str),
        Some("a&b"),
        "parsed source value"
    );
    assert_eq!(
        parameter
            .data
            .pointer("/payload/lexeme")
            .and_then(Value::as_str),
        Some("a&amp;b"),
        "exact XML spelling"
    );
    assert_eq!(
        attributes
            .get(8)
            .and_then(|row| row.data.pointer("/payload/value"))
            .and_then(Value::as_str),
        Some(""),
        "authored empty string"
    );
    assert_eq!(
        attributes
            .get(11)
            .and_then(|row| row.data.pointer("/payload/namespace"))
            .and_then(Value::as_str),
        Some("urn:event"),
        "unknown namespaced declaration has its namespace"
    );
    Ok(())
}

#[test]
fn cryxml_duplicates_and_encrypted_attribute_evidence_survive() -> TestResult {
    let scratch = tempfile::tempdir()?;
    let path = scratch.path().join("Data.p4k");
    write_binary_fixture(&path)?;
    let archive = Archive::open_with(path, OpenOptions::new().key(Some(test_key())))?;
    let (base, rows) = export(&archive, scratch.path())?;
    let timelines = records(&rows, "event-timeline");
    let [ambiguous, unique] = timelines.as_slice() else {
        return Err("expected two physical timelines".into());
    };
    assert_eq!(
        ambiguous.data.pointer("/payload/target"),
        Some(&json!({"state":"ambiguous-name"})),
        "duplicate CryXml names stay ambiguous"
    );
    assert_eq!(
        unique
            .data
            .pointer("/payload/target/reference/resolution/status")
            .and_then(Value::as_str),
        Some("not-selected"),
        "unique CAF Entry is found without loading it"
    );
    let attributes = records(&rows, "event-attribute");
    let [first, second] = attributes.as_slice() else {
        return Err("both original time attributes must survive".into());
    };
    assert_eq!(
        first.data.pointer("/payload/value").and_then(Value::as_str),
        Some("0"),
        "first time string"
    );
    assert_eq!(
        second
            .data
            .pointer("/payload/value")
            .and_then(Value::as_str),
        Some("00"),
        "second time string"
    );
    assert_ne!(
        first.data.pointer("/payload/position"),
        second.data.pointer("/payload/position"),
        "duplicate attributes have unique physical positions"
    );
    assert!(
        attributes.iter().all(|row| row
            .data
            .pointer("/payload/position/type")
            .and_then(Value::as_str)
            == Some("cry-xml-attribute")),
        "CryXml table coordinates are retained for both values"
    );
    verify_origins(&base, &rows, unique, second)?;
    verify_schema(&rows)
}

fn write_binary_fixture(path: &std::path::Path) -> TestResult {
    let binary = TestElement::new("anim_event_list")
        .child(
            TestElement::new("animation")
                .attribute("name", "Animations/Fixture/a.caf")
                .attribute("name", "Animations/Fixture/b.caf")
                .child(
                    TestElement::new("event")
                        .attribute("time", "0")
                        .attribute("time", "00"),
                ),
        )
        .child(TestElement::new("animation").attribute("name", "Animations/Fixture/a.caf"))
        .to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Little);
    ArchiveBuilder::new()
        .entry(TestEntry::stored(SOURCE, binary).encrypted())
        .entry(TestEntry::stored("Data/Animations/Fixture/a.caf", b"raw"))
        .write_to(path)?;
    Ok(())
}

fn verify_origins(
    base: &std::path::Path,
    rows: &[MediaRecord],
    timeline: &MediaRecord,
    attribute: &MediaRecord,
) -> TestResult {
    let file = std::fs::read_to_string(base.join("field-origins.jsonl"))?;
    let receipts = file
        .lines()
        .map(serde_json::from_str::<Value>)
        .collect::<Result<Vec<_>, _>>()?;
    for (row, field) in [
        (timeline, "/data/payload/target/authored_path"),
        (attribute, "/data/payload/value"),
    ] {
        let ordinal = rows
            .iter()
            .position(|candidate| std::ptr::eq(candidate, row))
            .ok_or("row")?;
        let ordinal = ordinal.to_string();
        let origin = receipts
            .iter()
            .find(|entry| {
                entry.pointer("/scope/ordinal").and_then(Value::as_str) == Some(ordinal.as_str())
                    && entry.pointer("/scope/stream/kind").and_then(Value::as_str)
                        == Some("animation-definition")
                    && entry.get("field").and_then(Value::as_str) == Some(field)
            })
            .ok_or_else(|| field.to_owned())?;
        assert_eq!(
            origin
                .get("containsEncryptedSource")
                .and_then(Value::as_bool),
            Some(true),
            "attribute comes from an encrypted Entry"
        );
        assert_eq!(
            origin.get("sourceSetComplete").and_then(Value::as_bool),
            Some(true),
            "attribute origin is complete"
        );
        assert_eq!(
            origin
                .get("sourceUsesRefs")
                .and_then(Value::as_array)
                .map(Vec::len),
            Some(1),
            "attribute points to one original slot"
        );
    }
    Ok(())
}

fn export(
    archive: &Archive,
    scratch: &std::path::Path,
) -> Result<(std::path::PathBuf, Vec<MediaRecord>), Box<dyn std::error::Error>> {
    let output = scratch.join("media");
    let options = ExportOptions {
        kinds: vec![MediaKind::AnimationDefinition],
        ..ExportOptions::default()
    };
    let index = support::published(p4kx_media::export(archive, &output, &options)?)?;
    let rows = support::records(&output, &index, STREAM)?;
    assert!(
        rows.iter().all(|row| row.status == Status::Complete),
        "all source facts exported"
    );
    Ok((output.join(index.artifact_base), rows))
}

fn records<'a>(rows: &'a [MediaRecord], kind: &str) -> Vec<&'a MediaRecord> {
    rows.iter()
        .filter(|row| row.data.get("record_type").and_then(Value::as_str) == Some(kind))
        .collect()
}

fn verify_schema(rows: &[MediaRecord]) -> TestResult {
    let validator = jsonschema::validator_for(&p4kx_media::builtin_record_schema()?)?;
    for row in rows {
        validator
            .validate(&serde_json::to_value(row)?)
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}
