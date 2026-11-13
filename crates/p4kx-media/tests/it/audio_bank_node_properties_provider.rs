// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::HircLayout;
use p4kx_media::{ExportOutcome, MediaRecord, Status};
use serde_json::json;

use super::audio_bank_node_properties_fixture::{
    self as fixture, BITS, Evidence, IDS, Lists, Node, OVERRIDE_BUS_ID, PARENT_ID, PRIORITY_FLAGS,
    RANGE_BITS, RANGE_IDS,
};
use super::audio_bank_state_provider::{check_retained, options, row, verify_layout};
use super::support;
use super::support::audio_provider::TestResult;

const SUMMARY: &str = "audio-bank-hirc-node-properties";
const PROPERTY: &str = "audio-bank-hirc-node-property";

#[test]
fn node_property_rows_preserve_owning_bank_and_exact_words_for_sound_and_actor_mixer() -> TestResult
{
    for (owner, encrypted, bus) in [
        (Node::Sound, true, OVERRIDE_BUS_ID),
        (Node::ActorMixer, false, 0),
    ] {
        let source = fixture::bank(owner, 154, bus, Lists::Complete)?;
        verify_layout(&source, HircLayout::Decoded)?;
        export(
            &source,
            encrypted,
            Publication::Complete,
            |records, evidence| assert_complete(records, evidence, bus),
        )?;
    }
    Ok(())
}

fn assert_complete(records: &[MediaRecord], evidence: &Evidence<'_>, bus: u32) -> TestResult {
    let header = row(records, SUMMARY, &[1, 0])?;
    assert_layout(header.1, "decoded", Status::Complete);
    assert_routing(evidence, header, bus)?;
    assert_counts(header.1, "properties", 3, 3, 3, true);
    assert_counts(header.1, "ranged_modifiers", 2, 2, 4, true);
    evidence.range(header, "observed/properties/ids_range", &IDS)?;
    let words: Vec<_> = RANGE_BITS.into_iter().flat_map(u32::to_le_bytes).collect();
    evidence.range(header, "observed/ranged_modifiers/words_range", &words)?;
    assert_property_count(records, IDS.len() + RANGE_IDS.len());
    evidence.word(
        header,
        "observed/properties/declared_count",
        "nodePropertyCount",
        3,
        &[3],
    )?;
    for (ordinal, (&id, &bits)) in IDS.iter().zip(&BITS).enumerate() {
        let row = property(records, evidence, 0, ordinal, id)?;
        word(evidence, row, "scalar_bits", "nodePropertyScalarBits", bits)?;
    }
    for (ordinal, (&id, bits)) in RANGE_IDS
        .iter()
        .zip(RANGE_BITS.as_chunks::<2>().0)
        .enumerate()
    {
        let row = property(records, evidence, 1, ordinal, id)?;
        let [minimum, maximum] = bits;
        word(
            evidence,
            row,
            "minimum_bits",
            "nodePropertyMinimumBits",
            *minimum,
        )?;
        word(
            evidence,
            row,
            "maximum_bits",
            "nodePropertyMaximumBits",
            *maximum,
        )?;
    }
    Ok(())
}

#[test]
fn encrypted_truncated_ids_keep_exact_earlier_words_and_report_unknown_word_region() -> TestResult {
    let source = fixture::bank(
        Node::Sound,
        154,
        OVERRIDE_BUS_ID,
        Lists::Truncated(&[4, 0xF1, 3]),
    )?;
    verify_layout(&source, HircLayout::Malformed)?;
    export(
        &source,
        true,
        Publication::Diagnostic,
        |records, evidence| {
            let header = row(records, SUMMARY, &[1, 0])?;
            assert_layout(header.1, "malformed", Status::Partial);
            assert_routing(evidence, header, OVERRIDE_BUS_ID)?;
            assert_counts(header.1, "properties", 4, 2, 0, false);
            assert!(
                header
                    .1
                    .data
                    .pointer("/payload/observed/properties/words_range")
                    .is_none(),
                "truncated IDs do not establish the value array start"
            );
            assert!(
                header
                    .1
                    .data
                    .pointer("/payload/observed/ranged_modifiers")
                    .is_none(),
                "the subsequent count remains unreadable"
            );
            for (ordinal, id) in [0xF1, 3].into_iter().enumerate() {
                let property = property(records, evidence, 0, ordinal, id)?.1;
                assert!(
                    property
                        .data
                        .pointer("/payload/words/scalar_bits")
                        .is_none(),
                    "no scalar is fabricated after incomplete IDs"
                );
            }
            assert_property_count(records, 2);
            Ok(())
        },
    )
}

#[test]
fn plain_truncated_ranged_word_keeps_the_exact_minimum_and_empty_properties() -> TestResult {
    let mut lists = vec![0];
    lists.extend(fixture::bundle(&RANGE_IDS, &RANGE_BITS[..3])?);
    lists.extend([0xAA, 0xBB]);
    let source = fixture::bank(Node::ActorMixer, 154, 0, Lists::Truncated(&lists))?;
    verify_layout(&source, HircLayout::Malformed)?;
    export(
        &source,
        false,
        Publication::Diagnostic,
        |records, evidence| {
            let header = row(records, SUMMARY, &[1, 0])?;
            assert_layout(header.1, "malformed", Status::Partial);
            assert_routing(evidence, header, 0)?;
            assert_counts(header.1, "properties", 0, 0, 0, true);
            assert_counts(header.1, "ranged_modifiers", 2, 2, 3, false);
            let first = property(records, evidence, 1, 0, RANGE_IDS[0])?;
            word(
                evidence,
                first,
                "minimum_bits",
                "nodePropertyMinimumBits",
                RANGE_BITS[0],
            )?;
            word(
                evidence,
                first,
                "maximum_bits",
                "nodePropertyMaximumBits",
                RANGE_BITS[1],
            )?;
            let second = property(records, evidence, 1, 1, RANGE_IDS[1])?;
            word(
                evidence,
                second,
                "minimum_bits",
                "nodePropertyMinimumBits",
                RANGE_BITS[2],
            )?;
            assert!(
                second
                    .1
                    .data
                    .pointer("/payload/words/maximum_bits")
                    .is_none(),
                "partial maximum bytes are not a raw word"
            );
            assert_property_count(records, 2);
            Ok(())
        },
    )
}

#[test]
fn unproved_versions_and_absent_sound_nodes_have_explicit_summary_dispositions() -> TestResult {
    for (node, version, lists, expected) in [
        (Node::Sound, 153, Lists::Complete, "unsupported"),
        (Node::ActorMixer, 153, Lists::Complete, "unsupported"),
        (Node::Sound, 154, Lists::Absent, "absent"),
    ] {
        let source = fixture::bank(node, version, 0, lists)?;
        export(
            &source,
            true,
            Publication::Complete,
            |records, _evidence| {
                let header = row(records, SUMMARY, &[1, 0])?.1;
                assert_layout(header, expected, Status::Complete);
                assert_eq!(
                    header.data.pointer("/payload/version"),
                    Some(&json!(version)),
                    "original declared bank version"
                );
                assert!(
                    header.data.pointer("/payload/observed").is_none(),
                    "there is no proved authored property region"
                );
                assert_property_count(records, 0);
                Ok(())
            },
        )?;
    }
    Ok(())
}

#[test]
fn a_later_actor_mixer_failure_keeps_complete_node_property_coverage() -> TestResult {
    let original = fixture::bank(Node::ActorMixer, 154, OVERRIDE_BUS_ID, Lists::Complete)?;
    let source = fixture::truncate_after_lists(&original)?;
    verify_layout(&source, HircLayout::Malformed)?;
    export(
        &source,
        true,
        Publication::Diagnostic,
        |records, evidence| {
            let actor = row(records, "audio-bank-hirc-actor-mixer", &[1, 0])?.1;
            assert_layout(actor, "malformed", Status::Partial);
            let header = row(records, SUMMARY, &[1, 0])?;
            assert_layout(header.1, "decoded", Status::Complete);
            assert_routing(evidence, header, OVERRIDE_BUS_ID)?;
            assert_property_count(records, IDS.len() + RANGE_IDS.len());
            Ok(())
        },
    )
}

#[derive(Clone, Copy)]
enum Publication {
    Complete,
    Diagnostic,
}

fn export(
    source: &[u8],
    encrypted: bool,
    publication: Publication,
    inspect: impl FnOnce(&[MediaRecord], &Evidence<'_>) -> TestResult,
) -> TestResult {
    let directory = tempfile::tempdir()?;
    let archive = fixture::archive(directory.path(), source, encrypted)?;
    let output = directory.path().join("out");
    let mut options = options();
    options.diagnostic = matches!(publication, Publication::Diagnostic);
    let (index, root) = match (
        publication,
        p4kx_media::export(&archive, &output, &options)?,
    ) {
        (Publication::Complete, ExportOutcome::Published { index, .. }) => (index, output),
        (Publication::Diagnostic, ExportOutcome::Incomplete { index, diagnostic }) => {
            let root = diagnostic.parent().ok_or("diagnostic generation root")?;
            (index, root.to_path_buf())
        }
        _ => return Err("expected Media publication disposition".into()),
    };
    let records = support::records(&root, &index, "audio.jsonl")?;
    let base = root.join(&index.artifact_base);
    let evidence = Evidence::read(&base, source, encrypted)?;
    let raw = row(&records, "audio-bank-object", &[1, 0])?.1;
    check_retained(&base, raw, source)?;
    let sibling = row(&records, "audio-bank-object", &[1, 1])?.1;
    assert_eq!(
        sibling.data.pointer("/payload/object_id"),
        Some(&json!(1)),
        "the adjacent physical event remains readable"
    );
    inspect(&records, &evidence)?;
    validate(&records)
}

fn assert_layout(record: &MediaRecord, layout: &str, status: Status) {
    assert_eq!(record.status, status, "node property coverage status");
    assert_eq!(
        record.data.pointer("/payload/layout"),
        Some(&json!(layout)),
        "specific typed node property layout"
    );
}

fn assert_routing(evidence: &Evidence<'_>, row: (usize, &MediaRecord), bus: u32) -> TestResult {
    evidence.word(
        row,
        "observed/override_bus_id",
        "nodeOverrideBusId",
        bus,
        &bus.to_le_bytes(),
    )?;
    evidence.word(
        row,
        "observed/parent_id",
        "nodeParentId",
        PARENT_ID,
        &PARENT_ID.to_le_bytes(),
    )?;
    evidence.word(
        row,
        "observed/priority_flags",
        "nodePriorityFlags",
        u32::from(PRIORITY_FLAGS),
        &[PRIORITY_FLAGS],
    )
}

fn assert_counts(
    record: &MediaRecord,
    list: &str,
    declared: u8,
    ids: u8,
    words: u16,
    complete: bool,
) {
    let payload = &record.data;
    for (field, expected) in [
        ("declared_count/value", json!(declared)),
        ("checked_id_count", json!(ids)),
        ("checked_word_count", json!(words)),
        ("complete", json!(complete)),
    ] {
        assert_eq!(
            payload.pointer(&format!("/payload/observed/{list}/{field}")),
            Some(&expected),
            "exact {list} {field}"
        );
    }
}

fn property<'records>(
    records: &'records [MediaRecord],
    evidence: &Evidence<'_>,
    list: u64,
    ordinal: usize,
    id: u8,
) -> TestResult<(usize, &'records MediaRecord)> {
    let row = row(records, PROPERTY, &[1, 0, list, u64::try_from(ordinal)?])?;
    evidence.word(row, "property_id", "nodePropertyId", u32::from(id), &[id])?;
    Ok(row)
}

fn word(
    evidence: &Evidence<'_>,
    row: (usize, &MediaRecord),
    field: &str,
    original: &str,
    bits: u32,
) -> TestResult {
    evidence.word(
        row,
        &format!("words/{field}"),
        original,
        bits,
        &bits.to_le_bytes(),
    )
}

fn assert_property_count(records: &[MediaRecord], expected: usize) {
    assert_eq!(
        records
            .iter()
            .filter(|row| row.subresource.namespace == PROPERTY)
            .count(),
        expected,
        "one row per checked original ID occurrence"
    );
}

fn validate(records: &[MediaRecord]) -> TestResult {
    let schema = jsonschema::validator_for(&p4kx_media::builtin_record_schema()?)?;
    for row in records {
        schema
            .validate(&serde_json::to_value(row)?)
            .map_err(|cause| cause.to_string())?;
    }
    Ok(())
}
