// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;

use p4kx_testkit::{ArchiveBuilder, TestAtl, TestBank, TestEntry, TestHircObject, wwise_short_id};
use serde_json::json;

use super::{Fixture, PLAY, TestResult, extract, pcm};

const SOURCE_FADE: &str = "Play_Source_Transition_Fade";
const DESTINATION_FADE: &str = "Play_Destination_Transition_Fade";
const OBJECT_FADE: &str = "Play_Object_Transition_Fade";
const SEGMENT: &str = "Play_Transition_Segment";
const ZERO: &str = "Play_Default_Transitions";
const FLAGS: [&str; 3] = ["--format", "wav", "--txtp"];
const CLIP_CODE: &str = "omit-music-clip-timing";
const FADE_CODE: &str = "omit-music-transition-fades";
const SEGMENT_CODE: &str = "omit-music-transition-segments";
const CONTAINER_ID: u32 = 10;
const PLAYED_SEGMENT: u32 = 11;
const PLAYED_TRACK: u32 = 12;
const TRANSITION_SEGMENT: u32 = 13;
const TRANSITION_TRACK: u32 = 14;
const TRANSITION_WEM: u32 = 2_506;
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
    Default,
}

#[test]
fn txtp_reports_each_proven_music_transition_fade_on_the_rendered_owner() -> TestResult {
    let fixture = fixture()?;
    expect(&fixture, SOURCE_FADE, &[CLIP_CODE, FADE_CODE])?;
    expect(&fixture, DESTINATION_FADE, &[CLIP_CODE, FADE_CODE])?;
    expect(&fixture, OBJECT_FADE, &[CLIP_CODE, FADE_CODE, SEGMENT_CODE])?;
    expect(&fixture, ZERO, &[CLIP_CODE])
}

#[test]
fn txtp_reports_an_allocated_transition_segment_independently_of_fades() -> TestResult {
    let fixture = fixture()?;
    expect(&fixture, SEGMENT, &[CLIP_CODE, SEGMENT_CODE])
}

fn expect(fixture: &Fixture, trigger: &str, codes: &[&str]) -> TestResult {
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
        text.lines().any(|line| line == playable),
        "the original playable file stays in the TXTP"
    );
    Ok(())
}

fn fixture() -> Result<Fixture, Box<dyn std::error::Error>> {
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
    builder
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

fn bank(
    event: &str,
    media: u32,
    authored: AuthoredRule,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
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
