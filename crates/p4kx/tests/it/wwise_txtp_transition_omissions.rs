// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;

use p4kx_testkit::{ArchiveBuilder, TestAtl, TestEntry};
use serde_json::{Value, json};

use super::txtp_origin_assertions::check_plain_fields;
use super::{Fixture, TestResult, extract, pcm};

#[path = "wwise_txtp_transition_bank.rs"]
mod transition_bank;

use transition_bank::{
    AuthoredRule, NEXT_BEAT, NEXT_SEGMENT, SAME_TIME, SWITCH_TRACK_WEM, TRANSITION_WEM, bank,
    switch_track_bank,
};

const TRACK: &str = "bnkHircMusicTrack";
const MUSIC_NODE: &str = "bnkHircMusicNode";
const SOURCE_FADE: &str = "Play_Source_Transition_Fade";
const DESTINATION_FADE: &str = "Play_Destination_Transition_Fade";
const OBJECT_FADE: &str = "Play_Object_Transition_Fade";
const SEGMENT: &str = "Play_Transition_Segment";
const MATCH_SOURCE_CUE: &str = "Play_Transition_Match_Source_Cue";
const SOURCE_SYNC: &str = "Play_Transition_Next_Beat";
const DESTINATION_ENTRY: &str = "Play_Transition_Same_Time";
const DESTINATION_JUMP: &str = "Play_Transition_Next_Segment";
const INACTIVE_PARAMETERS: &str = "Play_Transition_Inactive_Parameters";
const UNKNOWN_SELECTORS: &str = "Play_Transition_Unknown_Selectors";
const ZERO: &str = "Play_Default_Transitions";
const TRACK_SOURCE_FADE: &str = "Play_Track_Source_Fade";
const TRACK_DESTINATION_FADE: &str = "Play_Track_Destination_Fade";
const TRACK_ZERO: &str = "Play_Track_Zero_Fades";
const TRACK_SYNC: &str = "Play_Track_Next_Beat";
const FLAGS: [&str; 3] = ["--format", "wav", "--txtp"];
const CLIP_CODE: &str = "omit-music-clip-timing";
const FADE_CODE: &str = "omit-music-transition-fades";
const SEGMENT_CODE: &str = "omit-music-transition-segments";
const SCHEDULING_CODE: &str = "omit-music-transition-scheduling";
const LOOPS_CODE: &str = "omit-container-loops";
const CONDITIONS_CODE: &str = "omit-container-conditions";
const AUTOMATION_CODE: &str = "omit-music-track-automation";
const SOURCE_SYNC_MEDIA: u32 = 2_509;
const DESTINATION_ENTRY_MEDIA: u32 = 2_510;
const DESTINATION_JUMP_MEDIA: u32 = 2_511;
const CONTAINER_EVENTS: [(&str, u32, AuthoredRule); 11] = [
    (SOURCE_FADE, 2_501, AuthoredRule::SourceFade),
    (DESTINATION_FADE, 2_502, AuthoredRule::DestinationFade),
    (OBJECT_FADE, 2_503, AuthoredRule::ObjectFade),
    (SEGMENT, 2_504, AuthoredRule::Segment),
    (ZERO, 2_505, AuthoredRule::Default),
    (MATCH_SOURCE_CUE, 2_508, AuthoredRule::MatchSourceCue),
    (SOURCE_SYNC, SOURCE_SYNC_MEDIA, AuthoredRule::SourceSync),
    (
        DESTINATION_ENTRY,
        DESTINATION_ENTRY_MEDIA,
        AuthoredRule::DestinationEntry,
    ),
    (
        DESTINATION_JUMP,
        DESTINATION_JUMP_MEDIA,
        AuthoredRule::DestinationJump,
    ),
    (INACTIVE_PARAMETERS, 2_512, AuthoredRule::InactiveParameters),
    (UNKNOWN_SELECTORS, 2_513, AuthoredRule::UnknownSelectors),
];

#[test]
fn txtp_reports_each_proven_music_transition_fade_on_the_rendered_owner() -> TestResult {
    let fixture = fixture()?;
    for (trigger, field) in [
        (SOURCE_FADE, "musicTransitionSourceOffsetBits"),
        (DESTINATION_FADE, "musicTransitionDestinationTimeBits"),
    ] {
        let report = expect(
            &fixture,
            trigger,
            &[CLIP_CODE, FADE_CODE, LOOPS_CODE, AUTOMATION_CODE],
        )?;
        check_plain_fields(&report, FADE_CODE, MUSIC_NODE, &[field])?;
        check_plain_fields(&report, LOOPS_CODE, MUSIC_NODE, &["musicPlaylistLoopCount"])?;
    }
    let report = expect(
        &fixture,
        OBJECT_FADE,
        &[
            CLIP_CODE,
            FADE_CODE,
            SEGMENT_CODE,
            CONDITIONS_CODE,
            AUTOMATION_CODE,
        ],
    )?;
    check_plain_fields(
        &report,
        FADE_CODE,
        MUSIC_NODE,
        &[
            "musicTransitionObjectFadeOutOffsetBits",
            "musicTransitionAllocationFlag",
        ],
    )?;
    expect(&fixture, ZERO, &[CLIP_CODE, LOOPS_CODE, AUTOMATION_CODE])?;
    Ok(())
}

#[test]
fn txtp_reports_an_allocated_transition_segment_independently_of_fades() -> TestResult {
    let fixture = fixture()?;
    let report = expect(
        &fixture,
        SEGMENT,
        &[CLIP_CODE, SEGMENT_CODE, CONDITIONS_CODE, AUTOMATION_CODE],
    )?;
    check_plain_fields(
        &report,
        SEGMENT_CODE,
        MUSIC_NODE,
        &[
            "musicTransitionObjectSegmentId",
            "musicTransitionAllocationFlag",
        ],
    )?;
    check_plain_fields(
        &report,
        CONDITIONS_CODE,
        MUSIC_NODE,
        &["musicSwitchGroupCount", "musicSwitchTreeSize"],
    )
}

#[test]
fn txtp_reports_a_transition_cue_condition_without_a_fade_or_segment() -> TestResult {
    let fixture = fixture()?;
    let report = expect(
        &fixture,
        MATCH_SOURCE_CUE,
        &[CLIP_CODE, SCHEDULING_CODE, LOOPS_CODE, AUTOMATION_CODE],
    )?;
    check_plain_fields(
        &report,
        SCHEDULING_CODE,
        MUSIC_NODE,
        &["musicTransitionDestinationMatchSourceCueName"],
    )?;
    expect(&fixture, ZERO, &[CLIP_CODE, LOOPS_CODE, AUTOMATION_CODE])?;
    Ok(())
}

#[test]
fn txtp_reports_checked_transition_selectors_without_a_fade_or_segment() -> TestResult {
    let fixture = fixture()?;
    let next_beat = NEXT_BEAT.to_le_bytes();
    let same_time = SAME_TIME.to_le_bytes();
    let next_segment = NEXT_SEGMENT.to_le_bytes();
    for (event, media, rule, extra, field, expected_bytes) in [
        (
            SOURCE_SYNC,
            SOURCE_SYNC_MEDIA,
            AuthoredRule::SourceSync,
            LOOPS_CODE,
            "musicTransitionSourceSyncType",
            next_beat.as_slice(),
        ),
        (
            DESTINATION_ENTRY,
            DESTINATION_ENTRY_MEDIA,
            AuthoredRule::DestinationEntry,
            CONDITIONS_CODE,
            "musicTransitionEntryType",
            same_time.as_slice(),
        ),
        (
            DESTINATION_JUMP,
            DESTINATION_JUMP_MEDIA,
            AuthoredRule::DestinationJump,
            LOOPS_CODE,
            "musicTransitionJumpToType",
            next_segment.as_slice(),
        ),
    ] {
        let report = expect(
            &fixture,
            event,
            &[CLIP_CODE, SCHEDULING_CODE, extra, AUTOMATION_CODE],
        )?;
        check_plain_fields(&report, SCHEDULING_CODE, MUSIC_NODE, &[field])?;
        let original = bank(event, media, rule)?;
        check_selector_bytes(&report, field, &original, expected_bytes)?;
    }
    Ok(())
}

#[test]
fn txtp_preserves_playback_for_inactive_and_unknown_transition_selectors() -> TestResult {
    let fixture = fixture()?;
    for event in [ZERO, INACTIVE_PARAMETERS, UNKNOWN_SELECTORS] {
        expect(&fixture, event, &[CLIP_CODE, LOOPS_CODE, AUTOMATION_CODE])?;
    }
    Ok(())
}

pub(super) fn check_selector_bytes(
    report: &Value,
    field: &str,
    original: &[u8],
    expected_bytes: &[u8],
) -> TestResult {
    let indices = report
        .pointer("/txtp/files/0/provenance/uses")
        .and_then(Value::as_array)
        .and_then(|uses| {
            uses.iter().find(|source| {
                source.pointer("/location/field").and_then(Value::as_str) == Some(field)
            })
        })
        .and_then(|source| source.pointer("/location/indices"))
        .and_then(Value::as_array)
        .ok_or("diagnostic selector byte coordinates")?;
    let [start, end] = indices.as_slice() else {
        return Err("selector uses exactly one original byte range".into());
    };
    let start = start
        .as_str()
        .ok_or("selector byte start")?
        .parse::<usize>()?;
    let end = end.as_str().ok_or("selector byte end")?.parse::<usize>()?;
    assert_eq!(
        original.get(start..end),
        Some(expected_bytes),
        "{field} cites every original selector byte"
    );
    Ok(())
}

#[test]
fn txtp_reports_both_switch_track_fades_on_their_rendered_paths() -> TestResult {
    let fixture = fixture()?;
    for (trigger, field) in [
        (TRACK_SOURCE_FADE, "musicTrackSourceFadeOffsetBits"),
        (TRACK_DESTINATION_FADE, "musicTrackDestinationFadeTimeBits"),
    ] {
        let report = expect(
            &fixture,
            trigger,
            &[CLIP_CODE, FADE_CODE, CONDITIONS_CODE, AUTOMATION_CODE],
        )?;
        check_plain_fields(&report, FADE_CODE, TRACK, &[field])?;
    }
    let report = expect(
        &fixture,
        TRACK_ZERO,
        &[CLIP_CODE, CONDITIONS_CODE, AUTOMATION_CODE],
    )?;
    check_plain_fields(
        &report,
        CONDITIONS_CODE,
        TRACK,
        &["musicTrackSwitchMemberCount"],
    )
}

#[test]
fn txtp_reports_switch_track_sync_without_a_fade() -> TestResult {
    let fixture = fixture()?;
    let report = expect(
        &fixture,
        TRACK_SYNC,
        &[CLIP_CODE, SCHEDULING_CODE, CONDITIONS_CODE, AUTOMATION_CODE],
    )?;
    let field = "musicTrackTransitionSyncType";
    check_plain_fields(&report, SCHEDULING_CODE, TRACK, &[field])?;
    check_selector_bytes(
        &report,
        field,
        &switch_track_bank()?,
        &NEXT_BEAT.to_le_bytes(),
    )
}

fn expect(fixture: &Fixture, trigger: &str, codes: &[&str]) -> Result<Value, Box<dyn Error>> {
    let report = extract(fixture, trigger, &FLAGS, 0)?;
    assert_eq!(
        report.pointer("/txtp/files/0/approximations"),
        Some(&json!(codes)),
        "only observed features of this physical owner are reported for {trigger}",
    );
    let path = fixture.out.join(format!("{trigger}.txtp"));
    let text = fs::read_to_string(path)?;
    let header: Vec<_> = text
        .lines()
        .filter_map(|line| line.strip_prefix("# approximation: "))
        .collect();
    assert_eq!(header, codes, "header agrees with JSON for {trigger}");
    let playable = format!("{trigger}.wav");
    assert!(
        text.lines()
            .filter(|line| !line.starts_with('#'))
            .eq([playable.as_str()]),
        "the single playable command stays byte-identical for {trigger}"
    );
    Ok(report)
}

fn fixture() -> Result<Fixture, Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let archive = dir.path().join("Data.p4k");
    let out = dir.path().join("out");
    let mut atl = TestAtl::new("MusicTransitions");
    let mut builder = ArchiveBuilder::new();
    for (event, media, authored) in CONTAINER_EVENTS {
        atl = atl.trigger(event, &[event]);
        builder = builder
            .entry(TestEntry::stored(
                format!("Data/Sounds/wwise/{event}.bnk"),
                bank(event, media, authored)?,
            ))
            .entry(TestEntry::stored(
                format!("Data/Sounds/wwise/Media/{media}.wem"),
                pcm(),
            ));
    }
    for trigger in [
        TRACK_SOURCE_FADE,
        TRACK_DESTINATION_FADE,
        TRACK_ZERO,
        TRACK_SYNC,
    ] {
        atl = atl.trigger(trigger, &[trigger]);
    }
    builder
        .entry(TestEntry::stored(
            "Data/Sounds/wwise/SwitchTrack.bnk",
            switch_track_bank()?,
        ))
        .entry(TestEntry::stored(
            format!("Data/Sounds/wwise/Media/{SWITCH_TRACK_WEM}.wem"),
            pcm(),
        ))
        .entry(TestEntry::stored(
            format!("Data/Sounds/wwise/Media/{TRANSITION_WEM}.wem"),
            pcm(),
        ))
        .entry(TestEntry::zstd(
            "Data/Libs/GameAudio/MusicTransitions.xml",
            atl.to_cryxml(),
        ))
        .write_to(&archive)?;
    Ok(Fixture {
        _dir: dir,
        archive,
        out,
    })
}
