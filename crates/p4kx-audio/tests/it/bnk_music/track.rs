// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Decodes `MusicTrack` data and switch transition controls from synthetic banks.

use p4kx_audio::{HircValue, MediaStorage, MusicTrackKind};
use p4kx_testkit::TestHircObject;

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
        ("source time", 0, 45_i32, true),
        ("destination offset", 7 * WORD, -28, true),
        ("curve only", WORD, 2, false),
    ] {
        let mut body = original.clone();
        let field = body
            .get_mut(start + position..start + position + WORD)
            .ok_or("original switch transition word")?;
        assert_eq!(field, 0_u32.to_le_bytes(), "{name} starts at zero");
        field.copy_from_slice(&word.to_le_bytes());
        let (_, value) = decoded(TestHircObject::raw(11, 6, body), 154)?;
        let HircValue::MusicTrack(track) = value else {
            return Err("expected a checked switch track".into());
        };
        assert_eq!(track.has_switch_fades, expected, "{name}");
    }
    Ok(())
}
