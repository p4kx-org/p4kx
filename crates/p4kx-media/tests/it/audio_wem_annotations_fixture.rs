// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_extraction::ByteOrder;

use super::TestResult;

pub(super) const CUE_ID: u32 = 7001;
pub(super) const LABEL: &[u8] = b"  cue\0unconsumed";
pub(super) const REGION: &[u8] = b"\xe9\0";
pub(super) const CODE_PAGE: u16 = 1252;
pub(super) const NOTE_PREFIX: usize = 1_201;
pub(super) const TARGET: [u8; 4] = [b'd', 0xfe, b't', b'a'];
pub(super) const PURPOSE: [u8; 4] = [b'r', b'g', 0xfe, b'n'];
pub(super) const CUE_TAIL: [u8; 2] = [0xde, 0xad];
const HEADER_BYTES: usize = 8;

pub(super) struct Fixture {
    pub(super) bytes: Vec<u8>,
    pub(super) cue: Range<usize>,
    pub(super) label: Range<usize>,
    pub(super) note: Range<usize>,
    pub(super) region: Range<usize>,
}

pub(super) fn fixture(order: ByteOrder) -> TestResult<Fixture> {
    let mut bytes = start(order)?;
    let cue = chunk(&mut bytes, *b"cue ", &cue_body(order, true), order)?;
    let (list, items) = list_body(order)?;
    let parent = chunk(&mut bytes, *b"LIST", &list, order)?;
    let [label, note, region] = items.map(|item| shift(item, parent.start));
    let mut opaque = b"INFO".to_vec();
    chunk(&mut opaque, *b"labl", &[1, 2], order)?;
    chunk(&mut bytes, *b"LIST", &opaque, order)?;
    finish(&mut bytes, order)?;
    Ok(Fixture {
        bytes,
        cue,
        label,
        note,
        region,
    })
}

pub(super) fn damaged() -> TestResult<Vec<u8>> {
    let order = ByteOrder::Little;
    let mut bytes = start(order)?;
    chunk(&mut bytes, *b"cue ", &cue_body(order, false), order)?;
    let mut list = b"adtl".to_vec();
    chunk(&mut list, *b"labl", &[1, 2], order)?;
    let mut note = word(CUE_ID, order).to_vec();
    note.extend_from_slice(b"later\0");
    chunk(&mut list, *b"note", &note, order)?;
    chunk(&mut bytes, *b"LIST", &list, order)?;
    finish(&mut bytes, order)?;
    Ok(bytes)
}

fn start(order: ByteOrder) -> TestResult<Vec<u8>> {
    let mut bytes = match order {
        ByteOrder::Little => b"RIFF\0\0\0\0WAVE".to_vec(),
        ByteOrder::Big => b"RIFX\0\0\0\0WAVE".to_vec(),
    };
    let mut format = Vec::new();
    format.extend_from_slice(&half(1, order));
    format.extend_from_slice(&half(1, order));
    format.extend_from_slice(&word(48_000, order));
    format.extend_from_slice(&word(96_000, order));
    format.extend_from_slice(&half(2, order));
    format.extend_from_slice(&half(16, order));
    chunk(&mut bytes, *b"fmt ", &format, order)?;
    chunk(&mut bytes, *b"data", &[1, 2, 3, 4], order)?;
    Ok(bytes)
}

fn cue_body(order: ByteOrder, complete: bool) -> Vec<u8> {
    let mut bytes = word(2, order).to_vec();
    cue_point(&mut bytes, order, *b"data", 10);
    if complete {
        cue_point(&mut bytes, order, TARGET, 20);
        bytes.extend_from_slice(&CUE_TAIL);
    }
    bytes
}

fn cue_point(bytes: &mut Vec<u8>, order: ByteOrder, tag: [u8; 4], seed: u32) {
    bytes.extend_from_slice(&word(CUE_ID, order));
    bytes.extend_from_slice(&word(seed, order));
    bytes.extend_from_slice(&tag);
    for value in [seed + 1, seed + 2, seed + 3] {
        bytes.extend_from_slice(&word(value, order));
    }
}

fn list_body(order: ByteOrder) -> TestResult<(Vec<u8>, [Range<usize>; 3])> {
    let mut list = b"adtl".to_vec();
    let mut label = word(CUE_ID, order).to_vec();
    label.extend_from_slice(LABEL);
    let label = chunk(&mut list, *b"labl", &label, order)?;
    let mut note = word(CUE_ID, order).to_vec();
    note.push(0xff);
    note.resize(note.len() + NOTE_PREFIX - 1, b'x');
    note.extend_from_slice(b"\0unused");
    let note = chunk(&mut list, *b"note", &note, order)?;
    let mut region = word(CUE_ID, order).to_vec();
    region.extend_from_slice(&word(44, order));
    region.extend_from_slice(&PURPOSE);
    for value in [1, 2, 3, CODE_PAGE] {
        region.extend_from_slice(&half(value, order));
    }
    region.extend_from_slice(REGION);
    let region = chunk(&mut list, *b"ltxt", &region, order)?;
    chunk(&mut list, *b"JUNK", &[0x81, 0x82, 0x83], order)?;
    Ok((list, [label, note, region]))
}

fn chunk(
    bytes: &mut Vec<u8>,
    id: [u8; 4],
    body: &[u8],
    order: ByteOrder,
) -> TestResult<Range<usize>> {
    bytes.extend_from_slice(&id);
    bytes.extend_from_slice(&word(u32::try_from(body.len())?, order));
    let start = bytes.len();
    bytes.extend_from_slice(body);
    let end = bytes.len();
    if body.len() % 2 == 1 {
        bytes.push(0);
    }
    Ok(start..end)
}

fn finish(bytes: &mut [u8], order: ByteOrder) -> TestResult {
    let length = u32::try_from(bytes.len() - HEADER_BYTES)?;
    bytes
        .get_mut(4..8)
        .ok_or("original RIFF size word")?
        .copy_from_slice(&word(length, order));
    Ok(())
}

const fn shift(range: Range<usize>, start: usize) -> Range<usize> {
    start + range.start..start + range.end
}

const fn word(value: u32, order: ByteOrder) -> [u8; 4] {
    match order {
        ByteOrder::Little => value.to_le_bytes(),
        ByteOrder::Big => value.to_be_bytes(),
    }
}

const fn half(value: u16, order: ByteOrder) -> [u8; 2] {
    match order {
        ByteOrder::Little => value.to_le_bytes(),
        ByteOrder::Big => value.to_be_bytes(),
    }
}
