// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Original decision and transition coordinates in decoded music nodes.

use std::ops::Range;

use p4kx_audio::{
    HircValue, MusicFadeTimingField, MusicPlaylistLoopField, MusicTransitionFadeSide, Soundbank,
};
use p4kx_testkit::{TestBank, TestHircObject};

use super::{TestResult, body_of};

const WORD: usize = size_of::<u32>();
const PLAYLIST: u32 = 8;
const SWITCH: u32 = 7;
const ITEM_LEN: usize = 30;
const ITEM_LOOP_OFFSET: usize = 4 * WORD;
const TRANSITION_RULE_IDS: [u32; 5] = [1, 0x5200_0001, 2, 0x5200_0002, 0x5200_0003];
const RULE_FADE_LEN: usize = 3 * WORD;
const SOURCE_RULE_LEN: usize = RULE_FADE_LEN + 2 * WORD + 1;
const DESTINATION_RULE_LEN: usize = RULE_FADE_LEN + 2 * WORD + 2 * size_of::<u16>() + 2;

#[test]
fn playlist_loop_proof_points_to_the_first_non_neutral_word() -> TestResult {
    let playlist = TestHircObject::music_playlist(PLAYLIST, &[40, 41]);
    let mut payload = body_of(playlist)?;
    let first_loop = payload
        .len()
        .checked_sub(2 * ITEM_LEN)
        .ok_or("two physical playlist items")?
        + ITEM_LOOP_OFFSET;
    let second_loop = first_loop + ITEM_LEN;
    write_words(&mut payload, first_loop, [1, 0, 0])?;
    write_words(&mut payload, second_loop, [1, 0, -7])?;

    let bytes = bank(TestHircObject::raw(13, PLAYLIST, payload));
    let parsed = Soundbank::parse(bytes)?;
    let object = parsed.object(PLAYLIST).ok_or("original playlist")?;
    let HircValue::MusicPlaylist(playlist) = &object.value else {
        return Err("expected a checked playlist".into());
    };
    let evidence = playlist.authored_loop_evidence().ok_or("loop evidence")?;
    assert_eq!(
        evidence.field,
        MusicPlaylistLoopField::Maximum,
        "only the upper bound"
    );
    assert_eq!(evidence.word.value, -7, "signed original upper bound");
    let offset = object.id_byte_range().end + second_loop + 2 * size_of::<i16>();
    assert_eq!(
        evidence.word.range,
        span(offset, size_of::<i16>()),
        "exact bank offset of the second item's upper loop bound"
    );
    Ok(())
}

#[test]
fn switch_proof_records_both_nonzero_decision_header_words() -> TestResult {
    const GROUP: u32 = 0x4a50_bb3a;
    let bytes = bank(TestHircObject::music_switch(SWITCH, &[40], GROUP, &[40]));
    let group = bytes
        .windows(WORD)
        .position(|window| window == GROUP.to_le_bytes())
        .ok_or("physical group ID")?;
    let count = group.checked_sub(WORD).ok_or("group count before ID")?;
    let size = group + WORD + size_of::<u8>();
    let parsed = Soundbank::parse(bytes)?;
    let HircValue::MusicSwitch(switch) = &parsed.object(SWITCH).ok_or("original switch")?.value
    else {
        return Err("expected a checked switch".into());
    };
    let evidence = switch
        .decision_conditions_evidence()
        .ok_or("decision evidence")?;
    assert_eq!(evidence.group_count.value, 1, "a nonempty group table");
    assert_eq!(evidence.tree_size.value, 24, "a root and one leaf");
    assert_eq!(
        evidence.group_count.range,
        span(count, WORD),
        "physical count"
    );
    assert_eq!(evidence.tree_size.range, span(size, WORD), "physical size");
    Ok(())
}

#[test]
fn allocated_transition_proof_keeps_exact_fade_segment_and_flag_bytes() -> TestResult {
    let mut bytes = bank(TestHircObject::music_switch(SWITCH, &[40], 0x42, &[40]));
    let allocation = allocation_flag_offset(&bytes)?;
    let segment = allocation + size_of::<u8>();
    let fade_offset = segment + WORD + RULE_FADE_LEN + 2 * WORD;
    write_word(&mut bytes, segment, 77_u32.to_le_bytes())?;
    write_word(&mut bytes, fade_offset, (-31_i32).to_le_bytes())?;

    let parsed = Soundbank::parse(bytes)?;
    let HircValue::MusicSwitch(switch) = &parsed.object(SWITCH).ok_or("original switch")?.value
    else {
        return Err("expected a checked switch".into());
    };
    let segment_evidence = switch
        .transition_segment_evidence()
        .ok_or("segment evidence")?;
    let fade_evidence = switch.transition_fade_evidence().ok_or("fade evidence")?;
    assert_eq!(
        segment_evidence.segment_id.value, 77,
        "authored transition object"
    );
    assert_eq!(
        segment_evidence.segment_id.range,
        span(segment, WORD),
        "exact segment word"
    );
    assert_eq!(
        segment_evidence.allocation_flag.range,
        span(allocation, size_of::<u8>()),
        "exact allocation byte"
    );
    assert_eq!(
        fade_evidence.side,
        MusicTransitionFadeSide::ObjectFadeOut,
        "fade-out member"
    );
    assert_eq!(
        fade_evidence.field,
        MusicFadeTimingField::Offset,
        "signed offset"
    );
    assert_eq!(
        fade_evidence.word.value.cast_signed(),
        -31,
        "original timing"
    );
    assert_eq!(
        fade_evidence.word.range,
        span(fade_offset, WORD),
        "exact fade word"
    );
    assert_eq!(
        fade_evidence.allocation_flag.as_ref(),
        Some(&segment_evidence.allocation_flag),
        "the same allocation permits both reads"
    );
    Ok(())
}

fn allocation_flag_offset(bytes: &[u8]) -> Result<usize, Box<dyn std::error::Error>> {
    let marker = TRANSITION_RULE_IDS.map(u32::to_le_bytes);
    let marker = marker.as_flattened();
    let rules: Vec<_> = bytes
        .windows(marker.len())
        .enumerate()
        .filter_map(|(start, window)| (window == marker).then_some(start + marker.len()))
        .collect();
    let [_, allocated_rule] = rules.as_slice() else {
        return Err("two original transition rules".into());
    };
    Ok(allocated_rule + SOURCE_RULE_LEN + DESTINATION_RULE_LEN)
}

fn bank(object: TestHircObject) -> Vec<u8> {
    TestBank::new(1)
        .version(154)
        .object(TestHircObject::event(42, &[1, 2, 3]))
        .object(object)
        .bytes()
}

fn write_words(bytes: &mut [u8], start: usize, words: [i16; 3]) -> TestResult {
    let target = bytes
        .get_mut(start..start + size_of_val(&words))
        .ok_or("original signed loop words")?;
    target.copy_from_slice(words.map(i16::to_le_bytes).as_flattened());
    Ok(())
}

fn write_word(bytes: &mut [u8], start: usize, word: [u8; WORD]) -> TestResult {
    bytes
        .get_mut(start..start + WORD)
        .ok_or("original transition word")?
        .copy_from_slice(&word);
    Ok(())
}

const fn span(start: usize, length: usize) -> Range<u64> {
    start as u64..(start + length) as u64
}
