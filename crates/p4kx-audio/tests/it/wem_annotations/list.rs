// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::Cursor;

use p4kx_audio::{AudioError, WemList, WemListPayload, WemReadError, WemTextUtf8};

use super::{TestResult, add_chunks, next_chunk, open, wave};

#[test]
fn wem_annotations_list_keeps_typed_labels_and_all_original_subchunks() -> TestResult {
    let bytes = labels_wave();
    let mut source = open(&bytes)?;
    let mut cursor = source.chunk_cursor();
    let entry = next_chunk(&mut source, &mut cursor)?;
    let mut list = source.list(&entry)?;
    assert_eq!(list.list_type(), *b"adtl", "original list type");

    let label = list.next().ok_or("first label")??;
    assert_eq!(label.chunk().ordinal, 0, "original nested ordinal");
    let WemListPayload::Label(value) = label.payload() else {
        return Err("expected typed labl".into());
    };
    assert_eq!(value.cue_id, 17, "original cue ID");
    assert_eq!(
        value.raw_range,
        36..40,
        "stored bytes include terminator and tail"
    );
    assert_eq!(value.text_range, 36..38, "text ends before NUL");
    assert_eq!(value.terminator_offset, Some(38), "first NUL");
    assert_eq!(value.utf8, WemTextUtf8::Ascii, "only ASCII byte values");
    let mut text = [0; 2];
    list.read_text(&label, &mut text)?;
    assert_eq!(&text, b"Hi", "same-reader decoded projection input");
    assert!(
        matches!(
            list.read_text(&label, &mut [0; 4]),
            Err(WemReadError::TextBufferLength)
        ),
        "must read exactly the observed text extent"
    );

    let second = list.next().ok_or("second label")??;
    let WemListPayload::Label(value) = second.payload() else {
        return Err("expected second labl".into());
    };
    assert_eq!(value.cue_id, 17, "duplicate cue attachment is retained");
    assert_eq!(
        value.utf8,
        WemTextUtf8::Invalid,
        "invalid UTF-8 remains visible"
    );
    let mut text = [0; 1];
    list.read_text(&second, &mut text)?;
    assert_eq!(text, [0xFF], "invalid original byte remains exact");
    check_other_children(&mut list)?;
    Ok(())
}

#[test]
fn wem_annotations_validates_utf8_across_read_blocks_and_marks_missing_nul() -> TestResult {
    let mut label = 9_u32.to_le_bytes().to_vec();
    label.extend_from_slice(&[b'x'; 1023]);
    label.extend_from_slice(&[0xC3, 0xA9, 0, 0xFF]);
    let mut note = 9_u32.to_le_bytes().to_vec();
    note.push(0xC3);
    let list = list_body(*b"adtl", &[(*b"labl", &label), (*b"note", &note)]);
    let bytes = wave(false, &[(*b"LIST", &list)]);
    let mut source = open(&bytes)?;
    let mut cursor = source.chunk_cursor();
    let entry = next_chunk(&mut source, &mut cursor)?;
    let mut list = source.list(&entry)?;
    let label = list.next().ok_or("long label")??;
    let WemListPayload::Label(label) = label.payload() else {
        return Err("expected label".into());
    };
    assert_eq!(
        label.utf8,
        WemTextUtf8::Valid,
        "UTF-8 can straddle physical reads"
    );
    assert_eq!(
        label.terminator_offset,
        Some(1061),
        "NUL after the two-byte character"
    );
    assert_eq!(
        label.raw_range.end, 1063,
        "residual invalid byte is retained"
    );
    let note = list.next().ok_or("note")??;
    let WemListPayload::Note(note) = note.payload() else {
        return Err("expected note".into());
    };
    assert_eq!(
        note.utf8,
        WemTextUtf8::Invalid,
        "an incomplete UTF-8 sequence"
    );
    assert_eq!(note.terminator_offset, None, "no NUL in the note");
    assert_eq!(
        note.text_range, note.raw_range,
        "full unterminated bytes are retained"
    );
    Ok(())
}

#[test]
fn wem_annotations_utf8_receipt_ends_before_later_nul_scan() -> TestResult {
    let mut label = 9_u32.to_le_bytes().to_vec();
    label.push(0xFF);
    label.extend_from_slice(&[b'x'; 1026]);
    label.extend_from_slice(&[0, 0xFF]);
    let list = list_body(*b"adtl", &[(*b"labl", &label)]);
    let bytes = wave(false, &[(*b"LIST", &list)]);
    let mut source = open(&bytes)?;
    let mut cursor = source.chunk_cursor();
    let entry = next_chunk(&mut source, &mut cursor)?;
    let mut list = source.list(&entry)?;
    let label = list.next().ok_or("invalid long label")??;
    let WemListPayload::Label(label) = label.payload() else {
        return Err("expected label".into());
    };
    assert_eq!(
        label.utf8,
        WemTextUtf8::Invalid,
        "first block rejects UTF-8"
    );
    assert_eq!(
        label.utf8_input_range,
        36..1060,
        "only the first block reached UTF-8 validation"
    );
    assert_eq!(
        label.text_range,
        36..1063,
        "text range keeps all original pre-NUL bytes"
    );
    assert_eq!(
        label.terminator_offset,
        Some(1063),
        "the later NUL was still found"
    );
    assert_eq!(
        label.raw_range,
        36..1065,
        "the original tail is still recoverable"
    );
    Ok(())
}

#[test]
fn wem_annotations_list_keeps_unknown_meaning_and_bounds_malformed_nested_chunks() -> TestResult {
    let unknown = list_body(*b"UNKN", &[(*b"labl", &[0, 0, 0, 0, b'A', 0])]);
    let mut malformed = b"adtllabl".to_vec();
    malformed.extend_from_slice(&9_u32.to_le_bytes());
    malformed.push(b'Q');
    let bytes = wave(
        false,
        &[
            (*b"LIST", &unknown),
            (*b"LIST", &malformed),
            (*b"NEXT", &[0; 20]),
        ],
    );
    let mut source = open(&bytes)?;
    let mut cursor = source.chunk_cursor();
    let entry = next_chunk(&mut source, &mut cursor)?;
    let mut list = source.list(&entry)?;
    let opaque = list.next().ok_or("unknown LIST child")??;
    assert_eq!(list.list_type(), *b"UNKN", "original unknown list type");
    assert_eq!(opaque.chunk().id, *b"labl", "original child tag");
    assert_eq!(
        opaque.payload(),
        &WemListPayload::Opaque,
        "labl meaning requires adtl"
    );
    assert!(
        matches!(
            list.read_text(&opaque, &mut []),
            Err(WemReadError::InvalidOccurrence)
        ),
        "an opaque child does not expose a recognized text extent"
    );

    let entry = next_chunk(&mut source, &mut cursor)?;
    let mut list = source.list(&entry)?;
    assert!(
        matches!(
            list.next(),
            Some(Err(WemReadError::Format(AudioError::Truncated {
                len: 59,
                needed: 67
            })))
        ),
        "nested body cannot use following top-level bytes"
    );
    assert!(list.next().is_none(), "malformed nested directory ends");
    assert_eq!(
        next_chunk(&mut source, &mut cursor)?.chunk().id,
        *b"NEXT",
        "outer directory continues"
    );
    Ok(())
}

fn check_other_children(list: &mut WemList<'_, Cursor<&[u8]>>) -> TestResult {
    let short = list.next().ok_or("short label")??;
    assert_eq!(
        short.payload(),
        &WemListPayload::Malformed {
            available: 2,
            needed: 4
        },
        "bounded short label remains a row"
    );
    let opaque = list.next().ok_or("unknown child")??;
    assert_eq!(
        opaque.payload(),
        &WemListPayload::Opaque,
        "unknown child payload"
    );
    assert_eq!(opaque.chunk().id, *b"junk", "original unknown child tag");
    assert_eq!(opaque.chunk().next_offset, 76, "observed odd child padding");
    let region = list.next().ok_or("text region")??;
    let WemListPayload::TextRegion(value) = region.payload() else {
        return Err("expected typed ltxt".into());
    };
    assert_eq!(
        (value.cue_id, value.sample_length, value.purpose),
        (17, 84, *b"capt"),
        "region binding and length"
    );
    assert_eq!(
        (
            value.country,
            value.language,
            value.dialect,
            value.code_page
        ),
        (3, 4, 5, 1252),
        "original text declaration"
    );
    assert_eq!(
        value.text_range,
        104..107,
        "ltxt carries its full byte payload"
    );
    let mut text = [0; 3];
    list.read_text(&region, &mut text)?;
    assert_eq!(text, [0xE9, 0, 0xFF], "ltxt is not trimmed at a NUL");
    assert!(list.next().is_none(), "all original subchunks were visited");
    Ok(())
}

fn labels_wave() -> Vec<u8> {
    let mut region = 17_u32.to_le_bytes().to_vec();
    region.extend_from_slice(&84_u32.to_le_bytes());
    region.extend_from_slice(b"capt");
    for word in [3_u16, 4, 5, 1252] {
        region.extend_from_slice(&word.to_le_bytes());
    }
    region.extend_from_slice(&[0xE9, 0, 0xFF]);
    let body = list_body(
        *b"adtl",
        &[
            (*b"labl", &[17, 0, 0, 0, b'H', b'i', 0, 0xFF]),
            (*b"labl", &[17, 0, 0, 0, 0xFF, 0]),
            (*b"labl", &[1, 2]),
            (*b"junk", &[1, 2, 3]),
            (*b"ltxt", &region),
        ],
    );
    wave(false, &[(*b"LIST", &body)])
}

fn list_body(list_type: [u8; 4], entries: &[([u8; 4], &[u8])]) -> Vec<u8> {
    let mut body = list_type.to_vec();
    add_chunks(&mut body, false, entries);
    body
}
