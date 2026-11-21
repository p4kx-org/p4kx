// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::TestHircObject;

use super::super::audio_bank_container_node_provider::fixture::{bank, plain_node};
use super::super::support::audio_provider::TestResult;

const MUSIC_TRACK_TYPE: u8 = 11;
const TRACK_ID: u32 = 0x7C00_3100;
const SOURCE_ID: u32 = 0x8122_1110;
const CACHE_ID: u32 = 0x9211_3330;
const SWITCH_ID: u32 = 0xA222_4450;

pub(super) fn clips() -> Vec<u8> {
    let mut body = Vec::new();
    append_words(&mut body, [1, 0x0004_0001]);
    body.push(2);
    append_words(&mut body, [SOURCE_ID, CACHE_ID, 4096]);
    body.extend([0, 0xD1]);
    append_words(&mut body, [2]);
    for (subtrack, play_at) in [(0, 120.0_f64), (1, 240.0)] {
        append_words(&mut body, [subtrack, SOURCE_ID, CACHE_ID, 0]);
        body.extend(
            [play_at, 0.5, 0.25, 1200.0]
                .into_iter()
                .flat_map(f64::to_le_bytes),
        );
    }
    append_words(&mut body, [3]);
    body
}

pub(super) fn switch() -> TestResult<Vec<u8>> {
    let mut body = plain_node()?;
    body.extend([3, 1]);
    append_words(&mut body, [0xDA00_1211, 0, 2, SWITCH_ID, SWITCH_ID]);
    Ok(body)
}

pub(super) fn append_words(tail: &mut Vec<u8>, words: impl IntoIterator<Item = u32>) {
    tail.extend(words.into_iter().flat_map(u32::to_le_bytes));
}

pub(super) fn track(version: u32, body: Vec<u8>) -> Vec<u8> {
    bank(
        version,
        TestHircObject::raw(MUSIC_TRACK_TYPE, TRACK_ID, body),
    )
}
