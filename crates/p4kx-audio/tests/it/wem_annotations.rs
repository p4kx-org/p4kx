// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

mod list;

use std::io::Cursor;

use p4kx_audio::{AudioError, WemChunkCursor, WemChunkEntry, WemCue, WemReadError, WemSource};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn wem_annotations_keep_rifx_cue_fields_duplicate_ids_and_post_data_cues() -> TestResult {
    let mut body = 2_u32.to_be_bytes().to_vec();
    add_big_point(&mut body, [17, 31, 12, 9, 100], *b"data");
    add_big_point(&mut body, [17, 72, 3, 2, 200], *b"slnt");
    body.extend_from_slice(&[0xAB, 0xCD]);
    let bytes = wave(
        true,
        &[(*b"cue ", &body), (*b"data", &[4, 5]), (*b"cue ", &[0; 4])],
    );
    let mut source = open(&bytes)?;
    let mut cursor = source.chunk_cursor();
    let entry = next_chunk(&mut source, &mut cursor)?;
    let mut cue = source.cue(&entry)?;
    assert_eq!(cue.point_count(), 2, "original count word");
    assert_eq!(cue.trailing_range(), None, "scan in progress");

    assert_rifx_points(&mut cue)?;
    assert!(cue.next().is_none(), "all declared points visited");
    assert_eq!(cue.trailing_range(), Some(72..74), "raw bytes after points");

    assert_eq!(
        next_chunk(&mut source, &mut cursor)?.chunk().id,
        *b"data",
        "data between cues"
    );
    let later = next_chunk(&mut source, &mut cursor)?;
    assert_eq!(later.chunk().ordinal, 2, "post-DATA cue ordinal");
    let cue = source.cue(&later)?;
    assert_eq!(cue.point_count(), 0, "genuine empty cue");
    assert_eq!(cue.trailing_range(), Some(96..96), "genuine empty tail");
    Ok(())
}

#[test]
fn wem_annotations_cue_keeps_complete_prefix_and_checks_original_owner() -> TestResult {
    let mut body = 2_u32.to_be_bytes().to_vec();
    add_big_point(&mut body, [17, 31, 12, 9, 100], *b"data");
    let bytes = wave(true, &[(*b"cue ", &body), (*b"NEXT", &[0; 32])]);
    let mut source = open(&bytes)?;
    let mut other = open(&bytes)?;
    let mut cursor = source.chunk_cursor();
    assert!(
        matches!(
            other.next_chunk(&mut cursor),
            Some(Err(WemReadError::InvalidOccurrence))
        ),
        "an independent physical source cannot use this cursor"
    );
    let entry = next_chunk(&mut source, &mut cursor)?;
    assert!(
        matches!(other.cue(&entry), Err(WemReadError::InvalidOccurrence)),
        "matching source bytes do not authorize a foreign cue entry"
    );
    let mut cue = source.cue(&entry)?;
    assert_eq!(
        cue.next().ok_or("readable cue point")??.id,
        17,
        "the readable prefix"
    );
    assert!(
        matches!(
            cue.next(),
            Some(Err(WemReadError::Format(AudioError::Truncated {
                len: 48,
                needed: 72
            })))
        ),
        "a point cannot read past the end of its owning cue body"
    );
    assert!(cue.next().is_none(), "the cue failure is terminal locally");
    assert_eq!(cue.trailing_range(), None, "the cue remains incomplete");
    assert_eq!(
        next_chunk(&mut source, &mut cursor)?.chunk().id,
        *b"NEXT",
        "parent cursor still valid"
    );
    Ok(())
}

fn assert_rifx_points(cue: &mut WemCue<'_, Cursor<&[u8]>>) -> TestResult {
    let first = cue.next().ok_or("first cue point")??;
    assert_eq!(
        (first.ordinal, first.offset, first.id),
        (0, 24, 17),
        "first site"
    );
    assert_eq!(
        (
            first.position,
            first.target_chunk,
            first.chunk_start,
            first.block_start,
            first.sample_offset
        ),
        (31, *b"data", 12, 9, 100),
        "original RIFX position, FourCC and offsets"
    );
    let second = cue.next().ok_or("second cue point")??;
    assert_eq!(
        (second.ordinal, second.offset, second.id),
        (1, 48, 17),
        "duplicate cue ID"
    );
    assert_eq!(
        (
            second.position,
            second.target_chunk,
            second.chunk_start,
            second.block_start,
            second.sample_offset
        ),
        (72, *b"slnt", 3, 2, 200),
        "same ID in a distinct original occurrence"
    );
    Ok(())
}

fn add_big_point(body: &mut Vec<u8>, fields: [u32; 5], chunk: [u8; 4]) {
    let [id, position, chunk_start, block_start, sample_offset] = fields;
    body.extend_from_slice(&id.to_be_bytes());
    body.extend_from_slice(&position.to_be_bytes());
    body.extend_from_slice(&chunk);
    for word in [chunk_start, block_start, sample_offset] {
        body.extend_from_slice(&word.to_be_bytes());
    }
}

fn wave(big: bool, chunks: &[([u8; 4], &[u8])]) -> Vec<u8> {
    let mut body = b"WAVE".to_vec();
    add_chunks(&mut body, big, chunks);
    let mut file = if big { *b"RIFX" } else { *b"RIFF" }.to_vec();
    file.extend_from_slice(&length_word(body.len(), big));
    file.extend_from_slice(&body);
    file
}

fn add_chunks(body: &mut Vec<u8>, big: bool, entries: &[([u8; 4], &[u8])]) {
    for (tag, bytes) in entries {
        body.extend_from_slice(tag);
        body.extend_from_slice(&length_word(bytes.len(), big));
        body.extend_from_slice(bytes);
        if !bytes.len().is_multiple_of(2) {
            body.push(0);
        }
    }
}

fn length_word(len: usize, big: bool) -> [u8; 4] {
    let len = u32::try_from(len).unwrap_or(u32::MAX);
    if big {
        len.to_be_bytes()
    } else {
        len.to_le_bytes()
    }
}

fn next_chunk(
    source: &mut WemSource<Cursor<&[u8]>>,
    cursor: &mut WemChunkCursor,
) -> Result<WemChunkEntry, Box<dyn std::error::Error>> {
    Ok(source.next_chunk(cursor).ok_or("physical WEM chunk")??)
}

fn open(bytes: &[u8]) -> Result<WemSource<Cursor<&[u8]>>, WemReadError> {
    WemSource::open(Cursor::new(bytes), bytes.len() as u64)
}
