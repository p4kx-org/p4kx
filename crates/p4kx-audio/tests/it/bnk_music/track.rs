// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Decodes `MusicTrack` data and switch transition controls from synthetic banks.

use std::ops::Range;

use p4kx_audio::{
    HircValue, MediaStorage, MusicTrack, MusicTrackClipTimingKind, MusicTrackEvidenceKind,
    MusicTrackKind, Soundbank,
};
use p4kx_testkit::{TestBank, TestHircObject};

use super::{PARENT, TestResult, body_of, decoded};

#[test]
fn a_music_track_is_decoded_with_its_sources_clips_and_kind() -> TestResult {
    let object = TestHircObject::music_track(6, &[700, 800]);
    let HircValue::MusicTrack(track) = decoded(object, 154)?.1 else {
        return Err("a music track".into());
    };
    assert_eq!(track.parent_id, PARENT, "parent");
    assert_eq!(track.kind, MusicTrackKind::Normal, "kind");
    assert_eq!(track.switch, None, "no switch group");
    assert!(track.has_automation, "two checked original points");
    assert!(!track.has_switch_fades, "standard track has no switch fade");
    assert_eq!(
        track
            .sources
            .iter()
            .map(|source| (source.source_id, source.storage, source.in_memory_size))
            .collect::<Vec<_>>(),
        [
            (700, MediaStorage::Streamed, 4096),
            (800, MediaStorage::Streamed, 4096)
        ],
        "sources"
    );
    assert_eq!(
        track
            .clips
            .iter()
            .map(|clip| (clip.source_id, clip.begin_trim.to_bits()))
            .collect::<Vec<_>>(),
        [(700, 0.5_f64.to_bits()), (800, 0.5_f64.to_bits())],
        "clips"
    );
    Ok(())
}

#[test]
fn a_switch_track_keeps_its_group_and_switches() -> TestResult {
    let object = TestHircObject::music_switch_track(6, &[700], 0x4000, &[0x61, 0x62]);
    let HircValue::MusicTrack(track) = decoded(object, 154)?.1 else {
        return Err("a music track".into());
    };
    assert_eq!(track.kind, MusicTrackKind::Switch);
    assert!(!track.has_switch_fades, "neutral switch fade words");
    let switch = track.switch.ok_or("a switch group")?;
    assert_eq!(
        (switch.group_id, switch.default_switch, switch.switches),
        (0x4000, 0, vec![0x61, 0x62])
    );
    Ok(())
}

#[test]
fn a_music_track_observes_source_time_and_destination_offset_independently() -> TestResult {
    const WORD: usize = size_of::<u32>();
    const SWITCH_TRANSITION_AND_LOOKAHEAD: usize = 9 * WORD;
    let object = TestHircObject::music_switch_track(6, &[700], 0x4000, &[0x61]);
    let original = body_of(object)?;
    let start = original
        .len()
        .checked_sub(SWITCH_TRANSITION_AND_LOOKAHEAD)
        .ok_or("switch transition immediately before lookahead")?;
    for (name, position, word, expected) in [
        (
            "source time",
            0,
            45_i32,
            Some(MusicTrackEvidenceKind::SourceFadeTime),
        ),
        (
            "destination offset",
            7 * WORD,
            -28,
            Some(MusicTrackEvidenceKind::DestinationFadeOffset),
        ),
        ("curve only", WORD, 2, None),
    ] {
        let mut body = original.clone();
        let field = body
            .get_mut(start + position..start + position + WORD)
            .ok_or("original switch transition word")?;
        assert_eq!(field, 0_u32.to_le_bytes(), "{name} starts at zero");
        field.copy_from_slice(&word.to_le_bytes());
        let bytes = track_bank(body);
        let bank = Soundbank::parse(bytes.as_slice())?;
        let object = bank.objects().get(1).ok_or("the second HIRC object")?;
        let HircValue::MusicTrack(track) = &object.value else {
            return Err("expected a checked switch track".into());
        };
        let evidence = track.switch_fade_evidence();
        assert_eq!(track.has_switch_fades, expected.is_some(), "{name}");
        assert_eq!(evidence.map(|site| site.kind), expected, "{name} site");
        if let Some(site) = evidence {
            assert_eq!(
                site.original.value,
                u32::from_le_bytes(word.to_le_bytes()),
                "{name} bits"
            );
            check_word(
                &bytes,
                &site.original.range,
                object.id_byte_range().end + start + position,
                word.to_le_bytes(),
            );
        }
    }
    Ok(())
}

#[test]
fn a_switch_track_cites_a_default_or_checked_member_count() -> TestResult {
    const WORD: usize = size_of::<u32>();
    const TRANSITION_AND_LOOKAHEAD: usize = 9 * WORD;
    for (default_value, members, expected) in [
        (
            0_u32,
            [0x61_u32].as_slice(),
            MusicTrackEvidenceKind::MemberCount,
        ),
        (0x62, [].as_slice(), MusicTrackEvidenceKind::DefaultSwitch),
        (
            0x63,
            [0x61].as_slice(),
            MusicTrackEvidenceKind::DefaultSwitch,
        ),
    ] {
        let object = TestHircObject::music_switch_track(6, &[700], 0x4000, members);
        let mut body = body_of(object)?;
        let default_start = body
            .len()
            .checked_sub(TRANSITION_AND_LOOKAHEAD + 2 * WORD + size_of_val(members))
            .ok_or("default precedes the member count and transition")?;
        let field = body
            .get_mut(default_start..default_start + WORD)
            .ok_or("default word")?;
        field.copy_from_slice(&default_value.to_le_bytes());
        let bytes = track_bank(body);
        let bank = Soundbank::parse(bytes.as_slice())?;
        let object = bank.objects().get(1).ok_or("the second HIRC object")?;
        let HircValue::MusicTrack(track) = &object.value else {
            return Err("expected a checked switch track".into());
        };
        let evidence = track
            .switch_condition_evidence()
            .ok_or("precise condition")?;
        assert_eq!(
            evidence.kind, expected,
            "the nonzero default precedes the member count"
        );
        let (position, word) = if default_value == 0 {
            (default_start + WORD, 1_u32)
        } else {
            (default_start, default_value)
        };
        assert_eq!(evidence.original.value, word, "original condition bits");
        check_word(
            &bytes,
            &evidence.original.range,
            object.id_byte_range().end + position,
            word.to_le_bytes(),
        );
    }
    Ok(())
}

#[test]
fn a_music_track_keeps_automation_and_each_clip_control_separately() -> TestResult {
    const WORD: usize = size_of::<u32>();
    const TIME_WORD: usize = size_of::<f64>();
    const SOURCE_LEN: usize = 4 * WORD + 2;
    const CLIP_START: usize = WORD + SOURCE_LEN + 1 + WORD;
    const EVENT_START: usize = CLIP_START + 3 * WORD;
    const PLAY_AT_START: usize = CLIP_START + 4 * WORD;
    const BEGIN_TRIM_START: usize = PLAY_AT_START + TIME_WORD;
    const POINT_COUNT_START: usize = PLAY_AT_START + 4 * TIME_WORD + 4 * WORD;
    const POSTED_EVENT: u32 = 144;
    let mut body = body_of(TestHircObject::music_track(6, &[700]))?;
    body.get_mut(EVENT_START..EVENT_START + WORD)
        .ok_or("event word")?
        .copy_from_slice(&POSTED_EVENT.to_le_bytes());
    body.get_mut(PLAY_AT_START..PLAY_AT_START + TIME_WORD)
        .ok_or("play-at word")?
        .copy_from_slice(&f64::INFINITY.to_le_bytes());
    let bytes = track_bank(body);
    let bank = Soundbank::parse(bytes.as_slice())?;
    let object = bank.objects().get(1).ok_or("the second HIRC object")?;
    let HircValue::MusicTrack(track) = &object.value else {
        return Err("expected a checked music track".into());
    };
    let start = object.id_byte_range().end;
    let points = track
        .automation_point_count_evidence()
        .ok_or("automation points")?;
    let event = track.clip_event_evidence().ok_or("posted clip event")?;
    assert_eq!(points.value, 2, "two original points");
    assert_eq!(event.value, POSTED_EVENT, "authored event ID");
    check_word(
        &bytes,
        &points.range,
        start + POINT_COUNT_START,
        2_u32.to_le_bytes(),
    );
    check_word(
        &bytes,
        &event.range,
        start + EVENT_START,
        POSTED_EVENT.to_le_bytes(),
    );
    check_clip_timing(
        track,
        &bytes,
        start + PLAY_AT_START,
        start + BEGIN_TRIM_START,
    )
}

fn check_clip_timing(
    track: &MusicTrack,
    bytes: &[u8],
    play_at: usize,
    begin_trim: usize,
) -> TestResult {
    let timing = track.clip_timing_evidence().ok_or("finite trim")?;
    let nonfinite = track
        .nonfinite_clip_timing_evidence()
        .ok_or("nonfinite play at")?;
    assert_eq!(
        timing.kind,
        MusicTrackClipTimingKind::BeginTrim,
        "finite trim is independent"
    );
    assert_eq!(
        nonfinite.kind,
        MusicTrackClipTimingKind::PlayAt,
        "nonfinite schedule is independent"
    );
    check_word(
        bytes,
        &timing.original.range,
        begin_trim,
        0.5_f64.to_le_bytes(),
    );
    check_word(
        bytes,
        &nonfinite.original.range,
        play_at,
        f64::INFINITY.to_le_bytes(),
    );
    Ok(())
}

fn track_bank(body: Vec<u8>) -> Vec<u8> {
    TestBank::new(1)
        .version(154)
        .object(TestHircObject::event(1, &[]))
        .object(TestHircObject::raw(11, 6, body))
        .bytes()
}

fn check_word<const LEN: usize>(bytes: &[u8], range: &Range<u64>, start: usize, word: [u8; LEN]) {
    let end = start + LEN;
    assert_eq!(
        *range,
        start as u64..end as u64,
        "exact bank-relative word extent"
    );
    assert_eq!(
        bytes.get(start..end),
        Some(word.as_slice()),
        "original bytes at the evidence position"
    );
}
