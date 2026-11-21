// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, OpenOptions};
use p4kx_media::{ExportOptions, Identity, MediaKind, MediaRecord};
use p4kx_testkit::{
    ArchiveBuilder, CryXmlByteOrder, CryXmlSignature, TestElement, TestEntry, test_key,
};
use serde_json::{Value, json};

use crate::support;
use crate::support::audio_provider::{Origins, TestResult};

const PATH: &str = "Data/Libs/GameAudio/RadarAudioData.xml";
const FIELD: &str = "audio-control-radar-field";
const DEFINITION: &str = "audio-control-radar-definition";

#[test]
fn radar_audio_types_match_xml_and_cryxml_and_keep_physical_occurrences() -> TestResult {
    let scratch = tempfile::tempdir()?;
    let archive_path = scratch.path().join("Data.p4k");
    let fixture = radar();
    ArchiveBuilder::new()
        .entry(TestEntry::stored(PATH, fixture.to_xml().into_bytes()))
        .entry(
            TestEntry::stored(
                PATH,
                fixture.to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Little),
            )
            .encrypted(),
        )
        .write_to(&archive_path)?;
    let archive = Archive::open_with(archive_path, OpenOptions::new().key(Some(test_key())))?;
    let output = scratch.path().join("out");
    let options = ExportOptions {
        kinds: vec![MediaKind::AudioControl],
        ..ExportOptions::default()
    };
    let index = support::published(p4kx_media::export(&archive, &output, &options)?)?;
    let rows = support::records(&output, &index, "audio-control.jsonl")?;
    let origins = Origins::read(&output.join(&index.artifact_base))?;
    assert_schema_and_origins(&rows, &origins)?;
    let typed = |entry, namespace| {
        rows.iter()
            .filter(|row| row.subresource.namespace == namespace && owner(row) == Some(entry))
            .map(|row| &row.data["payload"])
            .collect::<Vec<_>>()
    };
    let fields = typed(0, FIELD);
    assert_eq!(fields, typed(1, FIELD));
    assert_eq!(typed(0, DEFINITION), typed(1, DEFINITION));
    assert_eq!(
        typed(0, DEFINITION),
        [
            json!("configuration"),
            json!("default-zoom"),
            json!("category-types"),
            json!("category-type"),
            json!("category-type"),
            json!("sound-events"),
            json!("sound-event"),
            json!("sound-event"),
        ]
        .iter()
        .collect::<Vec<_>>()
    );
    assert_eq!(fields.len(), 9);
    assert_typed_radar_fields(&fields);
    assert!(rows.iter().any(|row| {
        row.subresource.namespace == "audio-control-declaration"
            && row.data.pointer("/payload/value") == Some(&json!("RawExtension"))
    }));
    Ok(())
}

fn assert_schema_and_origins(rows: &[MediaRecord], origins: &Origins) -> TestResult {
    let schema = jsonschema::validator_for(&p4kx_media::builtin_record_schema()?)?;
    for (ordinal, row) in rows.iter().enumerate() {
        assert!(
            schema.is_valid(&serde_json::to_value(row)?),
            "radar audio row {ordinal} must satisfy the Media contract"
        );
        if row.subresource.namespace == FIELD {
            let source = owner(row).ok_or("root RadarAudioData entry")?;
            origins.field_in(
                "records",
                Some("audio-control"),
                ordinal,
                "/data/payload/value/state",
                source == 1,
            )?;
        }
    }
    Ok(())
}

fn assert_typed_radar_fields(fields: &[&Value]) {
    for (field, value) in [
        ("zoom-min", json!({"state": "empty"})),
        (
            "zoom-max",
            json!({"state": "invalid", "raw": "not-a-number"}),
        ),
        ("category-value", json!({"state": "integer", "raw": "+009"})),
        (
            "display-time",
            json!({"state": "number", "raw": "001.50", "value": 1.5}),
        ),
        (
            "sound-event-value",
            json!({"state": "text", "raw": "SyntheticCue"}),
        ),
    ] {
        assert!(
            fields
                .iter()
                .any(|row| row["field"] == json!(field) && row["value"] == value),
            "typed radar field {field}"
        );
    }
}

fn radar() -> TestElement {
    TestElement::new("RadarAudioData")
        .child(
            TestElement::new("DefaultZoom")
                .attribute("min", "")
                .attribute("max", "not-a-number")
                .attribute("extra", "RawExtension"),
        )
        .child(
            TestElement::new("CategoryTypes")
                .child(
                    TestElement::new("CategoryType")
                        .attribute("name", "SyntheticCategory")
                        .attribute("value", "+009")
                        .attribute("displayTime", "001.50"),
                )
                .child(TestElement::new("CategoryType").attribute("value", "-9")),
        )
        .child(
            TestElement::new("SoundEvents")
                .child(
                    TestElement::new("SoundEvent")
                        .attribute("name", "SyntheticSlot")
                        .attribute("value", "SyntheticCue"),
                )
                .child(TestElement::new("SoundEvent").attribute("name", "SyntheticSlot")),
        )
        .child(
            TestElement::new("Extension")
                .child(TestElement::new("SoundEvent").attribute("value", "NestedExtension")),
        )
}

const fn owner(row: &MediaRecord) -> Option<usize> {
    match &row.source.identity {
        Identity::Entry { containers, entry } if containers.is_empty() => Some(entry.entry_index),
        _ => None,
    }
}
