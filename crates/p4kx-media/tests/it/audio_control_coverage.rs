// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_extraction::{Archive, CancelToken, OpenOptions};
use p4kx_media::{ExportOptions, ExportOutcome, Identity, MediaError, MediaKind, MediaRecord};
use p4kx_testkit::{
    ArchiveBuilder, CryXmlByteOrder, CryXmlSignature, TestElement, TestEntry, test_key,
};
use serde_json::{Value, json};

use crate::support;
use crate::support::audio_provider::{TestResult, read_rows};

const PATH: &str = "Data/Libs/GameAudio/Synthetic.xml";

#[test]
fn every_atl_family_matches_between_plain_and_binary_with_immediate_origins() -> TestResult {
    let scratch = tempfile::tempdir()?;
    let archive_path = scratch.path().join("Data.p4k");
    let fixture = definition();
    let binary = fixture.to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Big);
    ArchiveBuilder::new()
        .entry(TestEntry::stored(PATH, fixture.to_xml().into_bytes()))
        .entry(TestEntry::stored(PATH, binary).encrypted())
        .write_to(&archive_path)?;
    let archive = Archive::open_with(archive_path, OpenOptions::new().key(Some(test_key())))?;
    let output = scratch.path().join("out");
    let index = support::published(p4kx_media::export(&archive, &output, &options())?)?;
    let rows = support::records(&output, &index, "audio-control.jsonl")?;
    check_equivalence(&rows);
    check_unknown(&rows);
    check_origins(&output.join(&index.artifact_base), &rows)?;
    let schema = jsonschema::validator_for(&p4kx_media::builtin_record_schema()?)?;
    for row in rows {
        assert!(
            schema.is_valid(&serde_json::to_value(row)?),
            "each published ATL wire row validates"
        );
    }
    Ok(())
}

fn check_equivalence(rows: &[MediaRecord]) {
    let observed = |entry, namespace| {
        rows.iter()
            .filter(|row| row.subresource.namespace == namespace && owner(row) == Some(entry))
            .map(|row| &row.data)
            .collect::<Vec<_>>()
    };
    let plain = observed(0, "audio-control-definition");
    let binary = observed(1, "audio-control-definition");
    assert_eq!(
        plain, binary,
        "ordered typed definition families agree across formats"
    );
    for family in [
        "rtpc",
        "wwise-rtpc",
        "switch",
        "switch-state",
        "wwise-switch",
        "wwise-state",
        "wwise-value",
        "environment",
        "wwise-aux-bus",
        "preload-request",
        "preload",
        "rtpc-dependee",
        "rtpc-dependees",
        "tags",
        "platform",
        "config-group",
        "external-source",
        "external-source-entry",
        "wwise-external-source",
        "trigger",
        "wwise-event",
    ] {
        assert!(
            plain
                .iter()
                .any(|row| row.pointer("/payload/family") == Some(&json!(family))),
            "original typed family {family}"
        );
    }
    assert_eq!(
        observed(0, "audio-control-field"),
        observed(1, "audio-control-field"),
        "typed facts preserve their values independently from format coordinates"
    );
}

fn check_unknown(rows: &[MediaRecord]) {
    for entry in [0, 1] {
        for (attribute, value) in [("min", "001.50"), ("custom_language", "Synthetic")] {
            assert!(
                rows.iter().any(|row| owner(row) == Some(entry)
                    && row.subresource.namespace == "audio-control-declaration"
                    && row.data.pointer("/payload/qualified_name") == Some(&json!(attribute))
                    && row.data.pointer("/payload/value") == Some(&json!(value))),
                "an unclassified source attribute remains available in physical Entry {entry}"
            );
        }
    }
}

fn check_origins(base: &Path, rows: &[MediaRecord]) -> TestResult {
    let receipts: Vec<Value> = read_rows(base.join("field-origins.jsonl"))?;
    for (entry, encrypted) in [(0, false), (1, true)] {
        let (ordinal, _) = rows
            .iter()
            .enumerate()
            .find(|(_, row)| {
                owner(row) == Some(entry)
                    && row.subresource.namespace == "audio-control-declaration"
                    && row.data.pointer("/payload/qualified_name")
                        == Some(&json!("wwise_duration_min"))
            })
            .ok_or("original event duration declaration")?;
        let receipt = receipts
            .iter()
            .find(|row| {
                row.pointer("/scope/stream/kind") == Some(&json!("audio-control"))
                    && row.pointer("/scope/ordinal") == Some(&json!(ordinal.to_string()))
                    && row.get("field") == Some(&json!("/data/payload/value"))
            })
            .ok_or("immediate ATL declaration receipt")?;
        assert_eq!(
            receipt.get("sourceSetComplete"),
            Some(&json!(true)),
            "complete XML field proof"
        );
        assert_eq!(
            receipt.get("containsEncryptedSource"),
            Some(&json!(encrypted)),
            "each physical declaration has independent protected-origin state"
        );
        assert!(
            receipt
                .get("sourceUsesRefs")
                .and_then(Value::as_array)
                .is_some_and(|uses| !uses.is_empty()),
            "the source-origin marker has an actual coordinate receipt"
        );
    }
    Ok(())
}

const fn owner(row: &MediaRecord) -> Option<usize> {
    match &row.source.identity {
        Identity::Entry { containers, entry } if containers.is_empty() => Some(entry.entry_index),
        _ => None,
    }
}

fn definition() -> TestElement {
    TestElement::new("ATLConfig")
        .child(
            TestElement::new("AudioRTPCs").child(
                TestElement::new("ATLRtpc")
                    .attribute("atl_name", "RtpcA")
                    .attribute("min", "001.50")
                    .child(
                        TestElement::new("WwiseRtpc")
                            .attribute("wwise_name", "RtpcSource")
                            .attribute("atl_mult", "01.0"),
                    ),
            ),
        )
        .child(
            TestElement::new("AudioSwitches").child(
                TestElement::new("ATLSwitch")
                    .attribute("atl_name", "SwitchA")
                    .child(
                        TestElement::new("ATLSwitchState")
                            .child(TestElement::new("WwiseSwitch"))
                            .child(TestElement::new("WwiseState").child(
                                TestElement::new("WwiseValue").attribute("wwise_value", "01"),
                            )),
                    ),
            ),
        )
        .child(
            TestElement::new("AudioEnvironments")
                .child(TestElement::new("ATLEnvironment").child(TestElement::new("WwiseAuxBus"))),
        )
        .child(preloads())
        .child(
            TestElement::new("AudioExternalSources").child(
                TestElement::new("ATLExternalSource").child(
                    TestElement::new("ATLExternalSourceEntry").child(
                        TestElement::new("WwiseExternalSource")
                            .attribute("custom_language", "Synthetic"),
                    ),
                ),
            ),
        )
        .child(
            TestElement::new("AudioTriggers").child(
                TestElement::new("ATLTrigger")
                    .attribute("atl_name", "TriggerA")
                    .child(
                        TestElement::new("WwiseEvent")
                            .attribute("wwise_name", "EventA")
                            .attribute("wwise_duration_min", "001.50"),
                    ),
            ),
        )
}

fn preloads() -> TestElement {
    TestElement::new("AudioPreloads").child(
        TestElement::new("ATLPreloadRequest")
            .attribute("atl_name", "PreloadA")
            .child(TestElement::new("ATLPreload").attribute("atl_name", "PreloadA"))
            .child(
                TestElement::new("RTPCDependees")
                    .child(TestElement::new("ATLRTPCDependee").attribute("atl_name", "RtpcA")),
            )
            .child(
                TestElement::new("ATLPlatforms").child(
                    TestElement::new("Platform").attribute("atl_config_group_name", "Default"),
                ),
            )
            .child(TestElement::new("ATLConfigGroup").attribute("atl_name", "Default"))
            .child(TestElement::new("Tags")),
    )
}

#[test]
fn malformed_atl_retains_only_diagnosed_output_and_exact_original_bytes() -> TestResult {
    let scratch = tempfile::tempdir()?;
    let input = b"<ATLConfig><Future/>";
    let archive = archive(scratch.path(), input)?;
    let output = scratch.path().join("out");
    let options = ExportOptions {
        diagnostic: true,
        ..options()
    };
    let ExportOutcome::Incomplete { index, diagnostic } =
        p4kx_media::export(&archive, &output, &options)?
    else {
        return Err("malformed ATL cannot publish a complete generation".into());
    };
    let base = diagnostic.parent().ok_or("diagnostic output root")?;
    let rows = support::records(base, &index, "audio-control.jsonl")?;
    let [row] = rows.as_slice() else {
        return Err("failed document diagnosis".into());
    };
    assert_eq!(
        row.subresource.namespace, "audio-control-failure",
        "typed source failure namespace"
    );
    assert_eq!(
        row.data.pointer("/payload"),
        Some(&json!("text-xml")),
        "the original parser failure stays typed"
    );
    let raw = row.source.raw.as_ref().ok_or("diagnostic raw source")?;
    assert_eq!(
        std::fs::read(base.join(&index.artifact_base).join(&raw.path))?,
        input,
        "original malformed source bytes remain recoverable"
    );
    assert!(
        !output.join("media-index.json").exists(),
        "diagnostic output is never a complete root"
    );
    Ok(())
}

#[test]
fn atl_projection_respects_generation_budgets_and_accepts_deep_flat_source_output() -> TestResult {
    let scratch = tempfile::tempdir()?;
    let xml = format!(
        "<ATLConfig>{}<Last/>{}</ATLConfig>",
        "<Future>".repeat(300),
        "</Future>".repeat(300)
    );
    let archive = archive(scratch.path(), xml.as_bytes())?;
    let output = scratch.path().join("complete");
    let index = support::published(p4kx_media::export(&archive, &output, &options())?)?;
    let rows = support::records(&output, &index, "audio-control.jsonl")?;
    assert!(
        rows.iter()
            .any(|row| row.data.pointer("/payload/qualified_name") == Some(&json!("Last"))),
        "300 levels of source nesting remain reachable in flat output"
    );
    let mut limited = options();
    limited.limits.records = 10;
    let failure = p4kx_media::export(&archive, &scratch.path().join("limited"), &limited)
        .expect_err("a chosen record budget must explicitly fail rather than truncate");
    assert!(
        budget(&failure, "records"),
        "typed record-budget error: {failure}"
    );
    let mut limited = options();
    limited.limits.source_bytes = xml.len() as u64;
    let failure = p4kx_media::export(&archive, &scratch.path().join("byte-limited"), &limited)
        .expect_err("retained source parsing must use the shared byte budget");
    assert!(
        budget(&failure, "source_bytes"),
        "typed source-byte budget error: {failure}"
    );
    let token = CancelToken::new();
    token.cancel();
    let cancelled = ExportOptions {
        cancel: token,
        ..options()
    };
    let failure = p4kx_media::export(&archive, &scratch.path().join("cancelled"), &cancelled);
    assert!(
        matches!(failure, Err(MediaError::Cancelled)),
        "ATL honors the caller's generation cancellation"
    );
    Ok(())
}

fn budget(error: &MediaError, expected: &str) -> bool {
    match error {
        MediaError::Limit { budget, .. } => *budget == expected,
        MediaError::Source { cause, .. } => budget(cause, expected),
        _ => false,
    }
}

fn archive(directory: &Path, input: &[u8]) -> TestResult<Archive> {
    let path = directory.join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored(PATH, input))
        .write_to(&path)?;
    Ok(Archive::open(path)?)
}

fn options() -> ExportOptions {
    ExportOptions {
        kinds: vec![MediaKind::AudioControl],
        ..ExportOptions::default()
    }
}
