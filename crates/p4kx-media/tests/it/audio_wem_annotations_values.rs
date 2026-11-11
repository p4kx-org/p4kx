// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_media::MediaRecord;
use serde_json::{Value, json};

use super::TestResult;
use super::fixture::{self, CUE_ID, Fixture};
use super::origins::{Origins, contains, exact};

pub(super) fn cue(
    records: &[(usize, &MediaRecord)],
    fixture: &Fixture,
    origins: &Origins,
    owner: usize,
) -> TestResult {
    let (_, header) = occurrence(records, "audio-wem-cue", &[2])?;
    assert_eq!(
        header.data.pointer("/payload/declared_points"),
        Some(&json!(2)),
        "raw point count"
    );
    let start = fixture.cue.start + 4;
    for (position, seed, target) in [(0, 10, *b"data"), (1, 20, fixture::TARGET)] {
        let (ordinal, point) = occurrence(records, "audio-wem-cue-point", &[2, position])?;
        let point_start = start + usize::try_from(position)? * 24;
        assert_eq!(
            point.data,
            json!({"record_type":"wem-cue-point","payload":{
                "chunk_ordinal":"2","ordinal":position.to_string(),
                "point_range":range(point_start..point_start+24),
                "id":CUE_ID,"position":seed,"target_chunk":target,
                "chunk_start":seed+1,"block_start":seed+2,"sample_offset":seed+3
            }}),
            "exact original point words and duplicate ID in declared order"
        );
        let evidence = origins.for_field(ordinal, owner, "/data/payload/sample_offset")?;
        assert!(
            exact(
                &evidence,
                "wemAnnotationField",
                "sampleOffset",
                point_start + 20,
                point_start + 24
            ),
            "actual source word for each cue point"
        );
    }
    let (_, tail) = occurrence(records, "audio-wem-cue-tail", &[2])?;
    assert_eq!(
        tail.data.pointer("/payload/trailing_range"),
        Some(&range(fixture.cue.end - 2..fixture.cue.end)),
        "actual opaque cue suffix remains recoverable"
    );
    Ok(())
}

pub(super) fn label(
    records: &[(usize, &MediaRecord)],
    fixture: &Fixture,
    origins: &Origins,
    owner: usize,
) -> TestResult {
    let (ordinal, label) = occurrence(records, "audio-wem-list-item", &[3, 0])?;
    let text_start = fixture.label.start + 4;
    let text_end = text_start + 5;
    assert_eq!(
        label.data.pointer("/payload/content"),
        Some(&json!({"type":"label","payload":{
            "cue_id":CUE_ID,"raw_range":range(text_start..fixture.label.end),
            "text_range":range(text_start..text_end),"terminator_offset":text_end,
            "utf8_input_range":range(text_start..text_end),"utf8":"ascii","utf8_text":"  cue"
        }})),
        "label retains whitespace, original terminator and unconsumed text suffix"
    );
    let evidence = origins.for_field(ordinal, owner, "/data/payload/content/payload/utf8_text")?;
    assert!(
        exact(
            &evidence,
            "wemAnnotationText",
            "utf8Text",
            text_start,
            text_end
        ),
        "text reread binds the actual selected range"
    );
    Ok(())
}

pub(super) fn note(
    records: &[(usize, &MediaRecord)],
    fixture: &Fixture,
    origins: &Origins,
    owner: usize,
) -> TestResult {
    let (ordinal, note) = occurrence(records, "audio-wem-list-item", &[3, 1])?;
    let start = fixture.note.start + 4;
    let terminator = start + fixture::NOTE_PREFIX;
    let value = note
        .data
        .pointer("/payload/content/payload")
        .ok_or("typed note")?;
    assert_eq!(
        value.get("utf8"),
        Some(&json!("invalid")),
        "invalid note bytes remain explicit"
    );
    assert_eq!(
        value.get("utf8_text"),
        Some(&Value::Null),
        "invalid bytes have no UTF-8 interpretation"
    );
    assert_eq!(
        value.get("terminator_offset"),
        Some(&json!(terminator)),
        "terminator found after invalid UTF-8 block"
    );
    let validated = value
        .pointer("/utf8_input_range/end_exclusive")
        .and_then(Value::as_u64)
        .ok_or("actual UTF-8 validator range")?;
    let validated = usize::try_from(validated)?;
    assert!(
        start < validated && validated < terminator,
        "invalid UTF-8 proof stops before later terminator scanning"
    );
    let utf8 = origins.for_field(ordinal, owner, "/data/payload/content/payload/utf8")?;
    assert!(
        exact(
            &utf8,
            "wemAnnotationText",
            "utf8Validation",
            start,
            validated
        ),
        "exact validity proof prefix"
    );
    assert!(
        !contains(&utf8, "nulScan"),
        "unused suffix is not a validity contributor"
    );
    let nul = origins.for_field(
        ordinal,
        owner,
        "/data/payload/content/payload/terminator_offset",
    )?;
    assert!(
        exact(&nul, "wemAnnotationText", "nulScan", start, terminator + 1),
        "separate terminator proof includes the terminator byte"
    );
    assert!(
        !contains(&nul, "utf8Validation"),
        "terminator selection is independent from UTF-8 interpretation"
    );
    Ok(())
}

pub(super) fn list(
    records: &[(usize, &MediaRecord)],
    fixture: &Fixture,
    origins: &Origins,
    owner: usize,
) -> TestResult {
    let (_, associated) = occurrence(records, "audio-wem-list", &[3])?;
    let (_, unknown) = occurrence(records, "audio-wem-list", &[4])?;
    assert_eq!(
        associated.data.pointer("/payload/list_type"),
        Some(&json!(b"adtl")),
        "original associated list type"
    );
    assert_eq!(
        unknown.data.pointer("/payload/list_type"),
        Some(&json!(b"INFO")),
        "original opaque list type"
    );
    let (ordinal, region) = occurrence(records, "audio-wem-list-item", &[3, 2])?;
    assert_eq!(
        region.data.pointer("/payload/content"),
        Some(&json!({"type":"text-region","payload":{
            "cue_id":CUE_ID,"sample_length":44,"purpose":fixture::PURPOSE,
            "country":1,"language":2,"dialect":3,"code_page":fixture::CODE_PAGE,
            "text_range":range(fixture.region.start+20..fixture.region.end)
        }})),
        "original region fields, code page and opaque bytes"
    );
    let encoding = origins.for_field(ordinal, owner, "/data/payload/content/payload/code_page")?;
    assert!(
        exact(
            &encoding,
            "wemAnnotationField",
            "codePage",
            fixture.region.start + 18,
            fixture.region.start + 20
        ),
        "exact original code-page word"
    );
    let (_, opaque) = occurrence(records, "audio-wem-list-item", &[3, 3])?;
    assert_eq!(
        opaque.data.pointer("/payload/content/type"),
        Some(&json!("opaque")),
        "unknown child body remains opaque"
    );
    assert!(
        opaque
            .data
            .pointer("/payload/padding_range/start")
            .is_some(),
        "actual nested padding is preserved"
    );
    let (ordinal, other) = occurrence(records, "audio-wem-list-item", &[4, 0])?;
    assert_eq!(
        other.data.pointer("/payload/content/type"),
        Some(&json!("opaque")),
        "unknown LIST type keeps child label opaque"
    );
    let gate = origins.for_field(ordinal, owner, "/data/payload/content/type")?;
    assert!(
        contains(&gate, "listType") && !contains(&gate, "listTag"),
        "unknown LIST type gates off child-tag interpretation"
    );
    Ok(())
}

fn occurrence<'record>(
    records: &[(usize, &'record MediaRecord)],
    namespace: &str,
    indices: &[u64],
) -> TestResult<(usize, &'record MediaRecord)> {
    records
        .iter()
        .find(|(_, row)| {
            row.subresource.namespace == namespace && row.subresource.indices.as_slice() == indices
        })
        .copied()
        .ok_or_else(|| format!("missing {namespace} at {indices:?}").into())
}

fn range(value: Range<usize>) -> Value {
    json!({"basis":{"type":"entry"},"start":value.start,"end_exclusive":value.end})
}
