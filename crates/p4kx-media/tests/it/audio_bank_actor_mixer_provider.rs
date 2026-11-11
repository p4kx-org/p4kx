// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{HircLayout, Soundbank};
use p4kx_media::{ExportOptions, ExportOutcome, MediaRecord, Status};
use p4kx_testkit::{TestBank, TestHircObject, TestNodeBase};
use serde_json::json;

use super::audio_bank_state_provider::{
    PATH, archive, check_retained, options, row, source_span, verify_layout,
};
use super::support;
use super::support::audio_provider::{Origins, Owner, TestResult};

const ACTOR_TYPE: u8 = 7;
const OBJECT: u32 = 86;
const PARENT: u32 = 0x2C21_1B2E;
const CHILDREN: [u32; 3] = [0x2141_1242, 0x2141_1242, 0xDC42_1855];
const BANK: &str = "audio-bank-hirc-actor-mixer";
const CHILD: &str = "audio-bank-hirc-actor-mixer-child";
const STATE: &str = "audio-bank-hirc-inline-state";
const RTPC: &str = "audio-bank-hirc-actor-mixer-rtpc";
const CURVE: &str = "audio-bank-hirc-actor-mixer-rtpc-curve";
const POINT: &str = "audio-bank-hirc-actor-mixer-rtpc-point";

#[test]
fn actor_mixer_exports_checked_node_fields_and_physical_children() -> TestResult {
    let directory = tempfile::tempdir()?;
    let source = fixture(154);
    verify_layout(&source, HircLayout::Decoded)?;
    let archive = archive(directory.path(), &source)?;
    let output = directory.path().join("out");
    let index = support::published(p4kx_media::export(&archive, &output, &options())?)?;
    let base = output.join(&index.artifact_base);
    let records = support::records(&output, &index, "audio.jsonl")?;
    let origins = Origins::read(&base)?;
    let (position, header) = row(&records, BANK, &[1, 0])?;
    assert_eq!(
        header.status,
        Status::Complete,
        "complete admitted original ActorMixer"
    );
    assert_eq!(
        header.data.pointer("/payload/parent_id/value"),
        Some(&json!(PARENT)),
        "the original nonzero parent"
    );
    assert_word(
        &source,
        &origins,
        position,
        header,
        "parent_id",
        "actorMixerParentId",
        &PARENT.to_le_bytes(),
    )?;
    assert_eq!(
        header.data.pointer("/payload/child_count/value"),
        Some(&json!(3)),
        "declared count includes the repeated child"
    );
    assert_word(
        &source,
        &origins,
        position,
        header,
        "child_count",
        "actorMixerChildCount",
        &3_u32.to_le_bytes(),
    )?;
    check_children(&source, &origins, &records, &CHILDREN)?;
    check_node_chunks(&source, &origins, &records)?;
    check_retained(&base, header, &source)?;
    validate(&records)
}

#[test]
fn actor_mixer_child_truncation_keeps_earlier_state_rtpc_and_complete_child_words() -> TestResult {
    let directory = tempfile::tempdir()?;
    let original = fixture(154);
    let bank = Soundbank::parse(&original)?;
    let object = bank
        .objects()
        .first()
        .ok_or("original fixture ActorMixer")?;
    let body = bank.object_body(object);
    let short = body
        .get(4..body.len() - 1)
        .ok_or("truncate one original child byte")?;
    let source = TestBank::new(3)
        .object(TestHircObject::raw(ACTOR_TYPE, OBJECT, short))
        .bytes();
    verify_layout(&source, HircLayout::Malformed)?;
    let archive = archive(directory.path(), &source)?;
    let output = directory.path().join("out");
    let options = ExportOptions {
        diagnostic: true,
        ..options()
    };
    let ExportOutcome::Incomplete { index, diagnostic } =
        p4kx_media::export(&archive, &output, &options)?
    else {
        return Err("diagnostic generation with the original partial ActorMixer".into());
    };
    let base = diagnostic.parent().ok_or("diagnostic generation base")?;
    let records = support::records(base, &index, "audio.jsonl")?;
    let origins = Origins::read(base)?;
    let (_, header) = row(&records, BANK, &[1, 0])?;
    assert_eq!(
        header.status,
        Status::Partial,
        "an unread child word prevents complete coverage"
    );
    assert_eq!(
        header.data.pointer("/payload/child_count/value"),
        Some(&json!(3)),
        "declared count remains an observed field"
    );
    assert_eq!(
        header.data.pointer("/payload/checked_children"),
        Some(&json!(2)),
        "only complete authored children fit"
    );
    check_children(&source, &origins, &records, &CHILDREN[..2])?;
    check_node_chunks(&source, &origins, &records)?;
    check_retained(base, header, &source)?;
    validate(&records)
}

#[test]
fn actor_mixer_retains_unsupported_version_with_no_inferred_fields() -> TestResult {
    let directory = tempfile::tempdir()?;
    let source = fixture(153);
    verify_layout(&source, HircLayout::Unsupported)?;
    let archive = archive(directory.path(), &source)?;
    let output = directory.path().join("out");
    let index = support::published(p4kx_media::export(&archive, &output, &options())?)?;
    let records = support::records(&output, &index, "audio.jsonl")?;
    let (_, header) = row(&records, BANK, &[1, 0])?;
    assert_eq!(
        header.data.pointer("/payload/layout"),
        Some(&json!("unsupported")),
        "unproved node format has an explicit disposition"
    );
    assert_eq!(
        header.data.pointer("/payload/version"),
        Some(&json!(153)),
        "actual source version"
    );
    assert!(
        header.data.pointer("/payload/parent_id").is_none(),
        "unsupported source supplies no authored parent observation"
    );
    assert!(
        records.iter().all(|record| !matches!(
            record.subresource.namespace.as_str(),
            CHILD | STATE | RTPC | CURVE | POINT
        )),
        "unproved chunks supply no inferred typed rows"
    );
    check_retained(&output.join(&index.artifact_base), header, &source)?;
    validate(&records)
}

fn fixture(version: u32) -> Vec<u8> {
    TestBank::new(3)
        .version(version)
        .object(TestHircObject::actor_mixer(
            OBJECT,
            &CHILDREN,
            TestNodeBase::Full,
        ))
        .bytes()
}

fn check_node_chunks(source: &[u8], origins: &Origins, rows: &[MediaRecord]) -> TestResult {
    for namespace in [STATE, RTPC] {
        let (_, row) = row(rows, namespace, &[1, 0])?;
        assert_eq!(
            row.status,
            Status::Complete,
            "independently checked node chunk {namespace}"
        );
    }
    let (position, curve) = row(rows, CURVE, &[1, 0, 0])?;
    assert_eq!(
        curve.data.pointer("/payload/rtpc_id/value"),
        Some(&json!(0x984F_8A08_u32)),
        "the original ActorMixer-owned curve ID"
    );
    assert_word(
        source,
        origins,
        position,
        curve,
        "rtpc_id",
        "rtpcId",
        &0x984F_8A08_u32.to_le_bytes(),
    )?;
    let (position, point) = row(rows, POINT, &[1, 0, 0, 1])?;
    assert_eq!(
        point.data.pointer("/payload/interpolation/value"),
        Some(&json!(0)),
        "the second original point retains the explicit zero word"
    );
    assert_word(
        source,
        origins,
        position,
        point,
        "interpolation",
        "rtpcInterpolation",
        &0_u32.to_le_bytes(),
    )?;
    let (position, property) = row(
        rows,
        "audio-bank-hirc-inline-state-property",
        &[1, 0, 0, 0, 0],
    )?;
    assert_word(
        source,
        origins,
        position,
        property,
        "value_bits",
        "stateValueBits",
        &96.0_f32.to_bits().to_le_bytes(),
    )
}

fn check_children(
    source: &[u8],
    origins: &Origins,
    rows: &[MediaRecord],
    expected: &[u32],
) -> TestResult {
    assert_eq!(
        rows.iter()
            .filter(|row| row.subresource.namespace == CHILD)
            .count(),
        expected.len(),
        "exact count of complete physical words, including duplicates"
    );
    for (ordinal, id) in expected.iter().enumerate() {
        let (position, child) = row(rows, CHILD, &[1, 0, u64::try_from(ordinal)?])?;
        assert_eq!(
            child.data.pointer("/payload/child_id/value"),
            Some(&json!(id)),
            "authored order and value of child {ordinal}"
        );
        assert_word(
            source,
            origins,
            position,
            child,
            "child_id",
            "actorMixerChildId",
            &id.to_le_bytes(),
        )?;
    }
    Ok(())
}

fn assert_word(
    source: &[u8],
    origins: &Origins,
    position: usize,
    record: &MediaRecord,
    field: &str,
    original: &str,
    expected: &[u8],
) -> TestResult {
    let range = format!("/payload/{field}/range");
    assert_eq!(
        source_span(source, &record.data, &range)?,
        expected,
        "reported span selects original {field} bytes"
    );
    let span = record.data.pointer(&range).ok_or("original range")?;
    let indices = json!([span.get("start"), span.get("end_exclusive")]);
    let datum = origins.field(
        "records",
        position,
        &format!("/data/payload/{field}/value"),
        true,
    )?;
    let owner = Owner {
        path: PATH,
        entry: 0,
        parent: None,
        stored: "encrypted",
        metadata: false,
        payload: true,
    };
    let uses = origins.require_owner(datum, owner)?;
    assert!(
        uses.iter().any(|used| {
            used.pointer("/location/field") == Some(&json!(original))
                && used.pointer("/location/indices") == Some(&indices)
                && used.get("role") == Some(&json!("payload"))
        }),
        "{field} has a complete direct receipt for the encrypted physical word"
    );
    Ok(())
}

fn validate(records: &[MediaRecord]) -> TestResult {
    let schema = jsonschema::validator_for(&p4kx_media::builtin_record_schema()?)?;
    for record in records.iter().filter(|row| {
        matches!(
            row.subresource.namespace.as_str(),
            BANK | CHILD | STATE | RTPC | CURVE | POINT
        )
    }) {
        schema
            .validate(&serde_json::to_value(record)?)
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}
