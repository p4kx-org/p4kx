// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;

use p4kx_testkit::{ArchiveBuilder, TestAtl, TestBank, TestEntry, TestHircObject, wwise_short_id};
use serde_json::{Value, json};

use super::txtp_origin_assertions::check_plain_fields;
use super::{Fixture, PLAY, TestResult, extract, hirc_payload, pcm};

const TRACK: &str = "bnkHircMusicTrack";
const MUSIC_NODE: &str = "bnkHircMusicNode";
const SOURCE_FADE: &str = "Play_Source_Transition_Fade";
const DESTINATION_FADE: &str = "Play_Destination_Transition_Fade";
const OBJECT_FADE: &str = "Play_Object_Transition_Fade";
const SEGMENT: &str = "Play_Transition_Segment";
const MATCH_SOURCE_CUE: &str = "Play_Transition_Match_Source_Cue";
const ZERO: &str = "Play_Default_Transitions";
const TRACK_SOURCE_FADE: &str = "Play_Track_Source_Fade";
const TRACK_DESTINATION_FADE: &str = "Play_Track_Destination_Fade";
const TRACK_ZERO: &str = "Play_Track_Zero_Fades";
const FLAGS: [&str; 3] = ["--format", "wav", "--txtp"];
const CLIP_CODE: &str = "omit-music-clip-timing";
const FADE_CODE: &str = "omit-music-transition-fades";
const SEGMENT_CODE: &str = "omit-music-transition-segments";
const SCHEDULING_CODE: &str = "omit-music-transition-scheduling";
const LOOPS_CODE: &str = "omit-container-loops";
const CONDITIONS_CODE: &str = "omit-container-conditions";
const AUTOMATION_CODE: &str = "omit-music-track-automation";
const CONTAINER_ID: u32 = 10;
const PLAYED_SEGMENT: u32 = 11;
const PLAYED_TRACK: u32 = 12;
const TRANSITION_SEGMENT: u32 = 13;
const TRANSITION_TRACK: u32 = 14;
const TRANSITION_WEM: u32 = 2_506;
const SWITCH_TRACK_WEM: u32 = 2_507;
const TRACK_SWITCH_GROUP: u32 = 0x4400_0000;
const TRACK_SWITCH_MEMBER: u32 = 0x4400_0001;
const WORD: usize = size_of::<u32>();
const FADE_LEN: usize = 3 * WORD;
const FADE_OFFSET: usize = 2 * WORD;
const SOURCE_RULE_LEN: usize = FADE_LEN + 2 * WORD + 1;
const DESTINATION_RULE_LEN: usize = FADE_LEN + 2 * WORD + 2 * size_of::<u16>() + 2;
const ALLOCATED_OBJECT: usize = SOURCE_RULE_LEN + DESTINATION_RULE_LEN + 1;

#[derive(Clone, Copy)]
enum AuthoredRule {
    SourceFade,
    DestinationFade,
    ObjectFade,
    Segment,
    MatchSourceCue,
    Default,
}

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
    for (event, media, authored) in [
        (SOURCE_FADE, 2_501, AuthoredRule::SourceFade),
        (DESTINATION_FADE, 2_502, AuthoredRule::DestinationFade),
        (OBJECT_FADE, 2_503, AuthoredRule::ObjectFade),
        (SEGMENT, 2_504, AuthoredRule::Segment),
        (ZERO, 2_505, AuthoredRule::Default),
        (MATCH_SOURCE_CUE, 2_508, AuthoredRule::MatchSourceCue),
    ] {
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
    for trigger in [TRACK_SOURCE_FADE, TRACK_DESTINATION_FADE, TRACK_ZERO] {
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

fn bank(event: &str, media: u32, authored: AuthoredRule) -> Result<Vec<u8>, Box<dyn Error>> {
    let container = match authored {
        AuthoredRule::Segment | AuthoredRule::ObjectFade => {
            TestHircObject::music_switch(CONTAINER_ID, &[PLAYED_SEGMENT], 0x42, &[PLAYED_SEGMENT])
        }
        _ => TestHircObject::music_playlist(CONTAINER_ID, &[PLAYED_SEGMENT]),
    };
    let mut bytes = TestBank::new(wwise_short_id(event))
        .version(154)
        .object(TestHircObject::event(wwise_short_id(event), &[1]))
        .object(TestHircObject::action(1, PLAY, CONTAINER_ID))
        .object(container)
        .object(TestHircObject::music_segment(
            PLAYED_SEGMENT,
            &[PLAYED_TRACK],
            &[],
        ))
        .object(TestHircObject::music_track(PLAYED_TRACK, &[media]))
        .object(TestHircObject::music_segment(
            TRANSITION_SEGMENT,
            &[TRANSITION_TRACK],
            &[],
        ))
        .object(TestHircObject::music_track(
            TRANSITION_TRACK,
            &[TRANSITION_WEM],
        ))
        .bytes();
    authored.apply(&mut bytes)?;
    Ok(bytes)
}

fn switch_track_bank() -> Result<Vec<u8>, Box<dyn Error>> {
    let mut bank = TestBank::new(wwise_short_id("SwitchTrack")).version(154);
    for (action, (event, track, source_offset, destination_time)) in (1..).zip([
        (TRACK_SOURCE_FADE, 20, -15, 0),
        (TRACK_DESTINATION_FADE, 21, 0, 18),
        (TRACK_ZERO, 22, 0, 0),
    ]) {
        bank = bank
            .object(TestHircObject::event(wwise_short_id(event), &[action]))
            .object(TestHircObject::action(action, PLAY, track))
            .object(switch_track(track, source_offset, destination_time)?);
    }
    Ok(bank.bytes())
}

fn switch_track(
    id: u32,
    source_offset: i32,
    destination_time: i32,
) -> Result<TestHircObject, Box<dyn Error>> {
    const SYNC_WORDS_LEN: usize = 2 * WORD;
    const TRANSITION_LEN: usize = 2 * FADE_LEN + SYNC_WORDS_LEN;

    let mut payload = hirc_payload(TestHircObject::music_switch_track(
        id,
        &[SWITCH_TRACK_WEM],
        TRACK_SWITCH_GROUP,
        &[TRACK_SWITCH_MEMBER],
    ))?;
    let start = payload
        .len()
        .checked_sub(TRANSITION_LEN + WORD)
        .ok_or("original switch track transition before the look-ahead word")?;
    assert_eq!(
        payload.get(start..start + TRANSITION_LEN),
        Some([0_u8; TRANSITION_LEN].as_slice()),
        "the synthetic track starts with two neutral fades and zero sync fields"
    );
    word(
        &mut payload,
        start + FADE_OFFSET,
        source_offset.to_le_bytes(),
    )?;
    word(
        &mut payload,
        start + FADE_LEN + SYNC_WORDS_LEN,
        destination_time.to_le_bytes(),
    )?;
    Ok(TestHircObject::raw(11, id, payload))
}

impl AuthoredRule {
    fn apply(self, bytes: &mut [u8]) -> TestResult {
        let rule_header = [1_u32, 0x5200_0001, 2, 0x5200_0002, 0x5200_0003].map(u32::to_le_bytes);
        let header = rule_header.as_flattened();
        let positions: Vec<_> = bytes
            .windows(header.len())
            .enumerate()
            .filter_map(|(offset, value)| (value == header).then_some(offset + header.len()))
            .collect();
        let [ordinary, allocated] = positions.as_slice() else {
            return Err("two original transition rules".into());
        };
        match self {
            Self::SourceFade => word(bytes, ordinary + FADE_OFFSET, (-15_i32).to_le_bytes())?,
            Self::DestinationFade => word(bytes, ordinary + SOURCE_RULE_LEN, 18_i32.to_le_bytes())?,
            Self::Segment => word(
                bytes,
                allocated + ALLOCATED_OBJECT,
                TRANSITION_SEGMENT.to_le_bytes(),
            )?,
            Self::ObjectFade => {
                word(
                    bytes,
                    allocated + ALLOCATED_OBJECT,
                    TRANSITION_SEGMENT.to_le_bytes(),
                )?;
                word(
                    bytes,
                    allocated + ALLOCATED_OBJECT + WORD + FADE_LEN + FADE_OFFSET,
                    (-31_i32).to_le_bytes(),
                )?;
            }
            Self::MatchSourceCue => {
                let flag = bytes
                    .get_mut(ordinary + SOURCE_RULE_LEN + DESTINATION_RULE_LEN - 1)
                    .ok_or("original match-source-cue flag")?;
                assert_eq!(*flag, 0, "the baseline rule has the field disabled");
                *flag = 1;
            }
            Self::Default => {}
        }
        Ok(())
    }
}

fn word(bytes: &mut [u8], offset: usize, value: [u8; WORD]) -> TestResult {
    bytes
        .get_mut(offset..offset + WORD)
        .ok_or("original transition word")?
        .copy_from_slice(&value);
    Ok(())
}
