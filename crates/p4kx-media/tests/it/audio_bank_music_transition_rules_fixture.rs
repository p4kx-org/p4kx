// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::Soundbank;
use p4kx_testkit::TestHircObject;

use super::super::audio_bank_container_node_provider::fixture::bank;
use super::super::support::audio_provider::TestResult;

const WORD: usize = size_of::<u32>();
const STINGER_WORDS: usize = 6;
const ORIGINAL_RULES: usize = 2;
const DEFAULT_ID_LIST_WORDS: usize = 5;
const SOURCE_WORD_BYTES: usize = 21;
const DESTINATION_WORD_BYTES: usize = 26;
const OBJECT_WORD_BYTES: usize = 30;
const DEFAULT_RULE_BYTES: usize =
    DEFAULT_ID_LIST_WORDS * WORD + SOURCE_WORD_BYTES + DESTINATION_WORD_BYTES + 1;
const ORIGINAL_RULE_BYTES: usize = WORD + ORIGINAL_RULES * DEFAULT_RULE_BYTES + OBJECT_WORD_BYTES;
const STINGER_TRIGGER: u32 = 0x5100_0001;
const STINGER_SEGMENT: u32 = 0x5100_0002;
const OWNER_ID: u32 = 0xA192_2000;
const CHILD_ID: u32 = 0xB012_8820;

pub(super) const SOURCE_IDS: [u32; 3] = [u32::MAX, 0, u32::MAX];
pub(super) const DESTINATION_IDS: [u32; 2] = [CHILD_ID, CHILD_ID];
pub(super) const OBJECT_ID: u32 = 0xC771_2210;

#[derive(Clone, Copy)]
pub(super) enum Owner {
    Switch,
    Playlist,
}

impl Owner {
    const fn type_byte(self) -> u8 {
        match self {
            Self::Switch => 12,
            Self::Playlist => 13,
        }
    }

    fn fixture(self) -> TestHircObject {
        match self {
            Self::Switch => {
                TestHircObject::music_switch(OWNER_ID, &[CHILD_ID], 0xD321_2200, &[CHILD_ID])
            }
            Self::Playlist => TestHircObject::music_playlist(OWNER_ID, &[CHILD_ID]),
        }
    }
}

pub(super) fn source(
    owner: Owner,
    version: u32,
    rules: &[u8],
    retain_suffix: bool,
) -> TestResult<Vec<u8>> {
    let original = bank(154, owner.fixture());
    let parsed = Soundbank::parse(&original)?;
    let object = parsed
        .objects()
        .first()
        .ok_or("original synthetic music owner")?;
    let body = parsed
        .object_body(object)
        .get(WORD..)
        .ok_or("original fixture body")?;
    let marker = [STINGER_TRIGGER.to_le_bytes(), STINGER_SEGMENT.to_le_bytes()];
    let marker = marker.as_flattened();
    let start = body
        .windows(marker.len())
        .position(|window| window == marker)
        .ok_or("original fixture stinger marker")?
        + STINGER_WORDS * WORD;
    let mut rewritten = body
        .get(..start)
        .ok_or("original music node prefix")?
        .to_vec();
    rewritten.extend(rules);
    if retain_suffix {
        let suffix = body
            .get(start + ORIGINAL_RULE_BYTES..)
            .ok_or("original music owner suffix")?;
        rewritten.extend(suffix);
    }
    Ok(bank(
        version,
        TestHircObject::raw(owner.type_byte(), OWNER_ID, rewritten),
    ))
}

pub(super) fn rule(
    buffer: &mut Vec<u8>,
    source: &[u32],
    destination: &[u32],
    allocation: u8,
) -> TestResult {
    ids(buffer, source)?;
    ids(buffer, destination)?;
    fade(buffer, -400, 0xFC00_1100, -7);
    words(buffer, [0xF100_1000, 0xDA00_1133]);
    buffer.push(0xE2);
    fade(buffer, 900, 0xFB00_2200, i32::MIN);
    words(buffer, [0, 0xD222_5511]);
    buffer.extend(0xFAC1_u16.to_le_bytes());
    buffer.extend(0xFD11_u16.to_le_bytes());
    buffer.extend([0xD1, 0xE2, allocation]);
    Ok(())
}

pub(super) fn transition_object(buffer: &mut Vec<u8>) {
    words(buffer, [OBJECT_ID]);
    fade(buffer, -21, 0xFA00_3300, 0);
    fade(buffer, 33, 0xF900_4400, -44);
    buffer.extend([0xD3, 0xE4]);
}

fn ids(buffer: &mut Vec<u8>, ids: &[u32]) -> TestResult {
    buffer.extend(u32::try_from(ids.len())?.to_le_bytes());
    words(buffer, ids.iter().copied());
    Ok(())
}

fn fade(buffer: &mut Vec<u8>, time: i32, curve: u32, offset: i32) {
    buffer.extend(time.to_le_bytes());
    buffer.extend(curve.to_le_bytes());
    buffer.extend(offset.to_le_bytes());
}

pub(super) fn words(buffer: &mut Vec<u8>, values: impl IntoIterator<Item = u32>) {
    buffer.extend(values.into_iter().flat_map(u32::to_le_bytes));
}
