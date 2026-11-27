// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, Filter};
use p4kx_media::{ExportOptions, MediaKind, MediaRecord, Source};
use p4kx_testkit::{ArchiveBuilder, TestBank, TestEntry, TestHircObject};
use serde_json::json;

use crate::support;
use crate::support::audio_provider::{TestResult, read_rows};

const ATL: &str = r#"
<ATLConfig>
  <AudioTriggers><ATLTrigger atl_name="SyntheticTrigger" path="Synthetic/">
    <WwiseEvent wwise_name="SyntheticEvent" wwise_localised="true" wwise_duration_min="001.50"/>
  </ATLTrigger></AudioTriggers>
  <Future xmlns:q="urn:synthetic" q:Count="009">first<Unknown/>last</Future>
</ATLConfig>
"#;

#[test]
fn audio_control_only_exports_typed_events_and_raw_xml_without_reading_filtered_soundbanks()
-> TestResult {
    let scratch = tempfile::tempdir()?;
    let archive_path = scratch.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored(
            "Data/Libs/GameAudio/Synthetic.XML",
            ATL.as_bytes(),
        ))
        .entry(TestEntry::stored("Data/Sounds/wwise/locked.bnk", b"unread").encrypted())
        .write_to(&archive_path)?;
    let archive = Archive::open(archive_path)?;
    let output = scratch.path().join("out");
    let options = ExportOptions {
        kinds: vec![MediaKind::AudioControl],
        include: Some(Filter::parse("*.xml")?),
        ..ExportOptions::default()
    };
    let index = support::published(p4kx_media::export(&archive, &output, &options)?)?;
    let base = output.join(&index.artifact_base);
    let sources: Vec<Source> = read_rows(base.join("sources.jsonl"))?;
    assert_eq!(
        sources.len(),
        1,
        "only the selected ATL definition has a raw receipt"
    );
    let rows = support::records(&output, &index, "audio-control.jsonl")?;
    check_standalone(&rows)
}

fn check_standalone(rows: &[MediaRecord]) -> TestResult {
    assert!(
        rows.iter().any(|row| {
            row.subresource.namespace == "audio-control-definition"
                && row.data.pointer("/payload/family") == Some(&json!("wwise-event"))
        }),
        "the original WwiseEvent is a typed ATL definition"
    );
    assert!(
        rows.iter().any(|row| {
            row.subresource.namespace == "audio-control-declaration"
                && row.data.pointer("/payload/qualified_name") == Some(&json!("wwise_duration_min"))
                && row.data.pointer("/payload/lexeme") == Some(&json!("001.50"))
        }),
        "the numeric literal remains byte-exact independently of parsed metadata"
    );
    assert!(
        rows.iter().any(|row| {
            row.subresource.namespace == "audio-control-declaration"
                && row.data.pointer("/payload/qualified_name") == Some(&json!("xmlns:q"))
        }),
        "unknown namespaces remain original declarations"
    );
    let event = rows
        .iter()
        .find(|row| {
            row.subresource.namespace == "audio-control-reference"
                && row.data.pointer("/payload/target/type") == Some(&json!("wwise-event"))
        })
        .ok_or("authored event edge without Audio provider")?;
    assert_eq!(
        event.data.pointer("/payload/link/status"),
        Some(&json!("not-selected")),
        "the authored Wwise Event survives an AudioControl-only generation"
    );
    let schema = jsonschema::validator_for(&p4kx_media::builtin_record_schema()?)?;
    for row in rows {
        assert!(
            schema.is_valid(&serde_json::to_value(row)?),
            "each ATL row validates"
        );
    }
    Ok(())
}

#[test]
fn selected_audio_resolves_an_authored_atl_event_with_exact_physical_candidates() -> TestResult {
    const SHORT_ID: u32 = 2_618_804_858;
    let scratch = tempfile::tempdir()?;
    let archive_path = scratch.path().join("Data.p4k");
    let target = TestBank::new(91)
        .object(TestHircObject::event(SHORT_ID, &[]))
        .bytes();
    ArchiveBuilder::new()
        .entry(TestEntry::stored(
            "Data/Libs/GameAudio/Synthetic.XML",
            ATL.as_bytes(),
        ))
        .entry(TestEntry::stored("Data/Sounds/wwise/target.bnk", target))
        .write_to(&archive_path)?;
    let archive = Archive::open(archive_path)?;
    let output = scratch.path().join("out");
    let options = ExportOptions {
        kinds: vec![MediaKind::AudioControl, MediaKind::Audio],
        ..ExportOptions::default()
    };
    let index = support::published(p4kx_media::export(&archive, &output, &options)?)?;
    let controls = support::records(&output, &index, "audio-control.jsonl")?;
    check_event_edge(&controls, SHORT_ID)?;
    let audio = support::records(&output, &index, "audio.jsonl")?;
    let query = audio
        .iter()
        .find(|row| row.subresource.namespace == "audio-bank-foreign-target-resolution")
        .ok_or("Audio provider inventory for the ATL Event")?;
    assert_eq!(
        query.subresource.indices,
        [0],
        "the physical query key matches the ATL edge"
    );
    assert_eq!(
        query.data.pointer("/payload/owner_entry_index"),
        Some(&json!("0")),
        "the physical ATL file is the reference owner"
    );
    assert_eq!(
        query.data.pointer("/payload/resolution"),
        Some(&json!("resolved")),
        "complete selected Wwise inventory proves a unique candidate"
    );
    Ok(())
}

fn check_event_edge(controls: &[MediaRecord], short_id: u32) -> TestResult {
    let link = controls
        .iter()
        .find(|row| row.subresource.namespace == "audio-control-reference")
        .ok_or("explicit authored Wwise Event edge")?;
    assert_eq!(
        link.data.pointer("/payload/target/name"),
        Some(&json!("SyntheticEvent")),
        "exact authored name"
    );
    assert_eq!(
        link.data.pointer("/payload/target/short_id"),
        Some(&json!(short_id)),
        "shared Wwise ShortID"
    );
    assert_eq!(
        link.data.pointer("/payload/link"),
        Some(&json!({"status":"soundbank-search","query_index":"0"})),
        "join to the owning directory query"
    );
    Ok(())
}
