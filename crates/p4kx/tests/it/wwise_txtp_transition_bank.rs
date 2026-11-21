// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_testkit::{TestBank, TestHircObject, wwise_short_id};

use super::super::{PLAY, TestResult, hirc_payload};
use super::{TRACK_DESTINATION_FADE, TRACK_SOURCE_FADE, TRACK_SYNC, TRACK_ZERO};

const CONTAINER_ID: u32 = 10;
const PLAYED_SEGMENT: u32 = 11;
const PLAYED_TRACK: u32 = 12;
const TRANSITION_SEGMENT: u32 = 13;
const TRANSITION_TRACK: u32 = 14;
pub(super) const TRANSITION_WEM: u32 = 2_506;
pub(super) const SWITCH_TRACK_WEM: u32 = 2_507;
const TRACK_SWITCH_GROUP: u32 = 0x4400_0000;
const TRACK_SWITCH_MEMBER: u32 = 0x4400_0001;
const WORD: usize = size_of::<u32>();
const FADE_LEN: usize = 3 * WORD;
const FADE_OFFSET: usize = 2 * WORD;
const SOURCE_RULE_LEN: usize = FADE_LEN + 2 * WORD + 1;
const DESTINATION_RULE_LEN: usize = FADE_LEN + 2 * WORD + 2 * size_of::<u16>() + 2;
const ALLOCATED_OBJECT: usize = SOURCE_RULE_LEN + DESTINATION_RULE_LEN + 1;
const SOURCE_SYNC_OFFSET: usize = FADE_LEN;
const SOURCE_CUE_OFFSET: usize = SOURCE_SYNC_OFFSET + WORD;
const DESTINATION_CUE_OFFSET: usize = SOURCE_RULE_LEN + FADE_LEN;
const DESTINATION_JUMP_ID_OFFSET: usize = DESTINATION_CUE_OFFSET + WORD;
const DESTINATION_JUMP_TYPE_OFFSET: usize = DESTINATION_JUMP_ID_OFFSET + WORD;
const DESTINATION_ENTRY_TYPE_OFFSET: usize = DESTINATION_JUMP_TYPE_OFFSET + size_of::<u16>();
pub(super) const NEXT_BEAT: u32 = 0x3;
pub(super) const SAME_TIME: u16 = 0x1;
pub(super) const NEXT_SEGMENT: u16 = 0x3;

#[derive(Clone, Copy)]
pub(super) enum AuthoredRule {
    SourceFade,
    DestinationFade,
    ObjectFade,
    Segment,
    MatchSourceCue,
    SourceSync,
    DestinationEntry,
    DestinationJump,
    InactiveParameters,
    UnknownSelectors,
    Default,
}

pub(super) fn bank(
    event: &str,
    media: u32,
    authored: AuthoredRule,
) -> Result<Vec<u8>, Box<dyn Error>> {
    let container = match authored {
        AuthoredRule::Segment | AuthoredRule::ObjectFade | AuthoredRule::DestinationEntry => {
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

pub(super) fn switch_track_bank() -> Result<Vec<u8>, Box<dyn Error>> {
    let mut bank = TestBank::new(wwise_short_id("SwitchTrack")).version(154);
    for (action, (event, track, source_offset, destination_time, sync)) in (1..).zip([
        (TRACK_SOURCE_FADE, 20, -15, 0, 0),
        (TRACK_DESTINATION_FADE, 21, 0, 18, 0),
        (TRACK_ZERO, 22, 0, 0, 0),
        (TRACK_SYNC, 23, 0, 0, NEXT_BEAT),
    ]) {
        bank = bank
            .object(TestHircObject::event(wwise_short_id(event), &[action]))
            .object(TestHircObject::action(action, PLAY, track))
            .object(switch_track(track, source_offset, destination_time, sync)?);
    }
    Ok(bank.bytes())
}

fn switch_track(
    id: u32,
    source_offset: i32,
    destination_time: i32,
    sync: u32,
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
    word(&mut payload, start + FADE_LEN, sync.to_le_bytes())?;
    word(
        &mut payload,
        start + FADE_LEN + SYNC_WORDS_LEN,
        destination_time.to_le_bytes(),
    )?;
    Ok(TestHircObject::raw(11, id, payload))
}

impl AuthoredRule {
    fn apply(self, bytes: &mut [u8]) -> TestResult {
        let [ordinary, allocated] = rule_positions(bytes)?;
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
            Self::MatchSourceCue => enable_match_source_cue(bytes, ordinary)?,
            Self::SourceSync => word(
                bytes,
                ordinary + SOURCE_SYNC_OFFSET,
                NEXT_BEAT.to_le_bytes(),
            )?,
            Self::DestinationEntry => word(
                bytes,
                ordinary + DESTINATION_ENTRY_TYPE_OFFSET,
                SAME_TIME.to_le_bytes(),
            )?,
            Self::DestinationJump => word(
                bytes,
                ordinary + DESTINATION_JUMP_TYPE_OFFSET,
                NEXT_SEGMENT.to_le_bytes(),
            )?,
            Self::InactiveParameters => {
                for offset in [
                    SOURCE_CUE_OFFSET,
                    DESTINATION_CUE_OFFSET,
                    DESTINATION_JUMP_ID_OFFSET,
                ] {
                    word(bytes, ordinary + offset, u32::MAX.to_le_bytes())?;
                }
            }
            Self::UnknownSelectors => {
                word(bytes, ordinary + SOURCE_SYNC_OFFSET, u32::MAX.to_le_bytes())?;
                for offset in [DESTINATION_ENTRY_TYPE_OFFSET, DESTINATION_JUMP_TYPE_OFFSET] {
                    word(bytes, ordinary + offset, u16::MAX.to_le_bytes())?;
                }
            }
            Self::Default => {}
        }
        Ok(())
    }
}

fn rule_positions(bytes: &[u8]) -> Result<[usize; 2], Box<dyn Error>> {
    let rule_header = [1_u32, 0x5200_0001, 2, 0x5200_0002, 0x5200_0003].map(u32::to_le_bytes);
    let header = rule_header.as_flattened();
    let mut positions = bytes
        .windows(header.len())
        .enumerate()
        .filter_map(|(offset, value)| (value == header).then_some(offset + header.len()));
    match (positions.next(), positions.next(), positions.next()) {
        (Some(ordinary), Some(allocated), None) => Ok([ordinary, allocated]),
        _ => Err("two original transition rules".into()),
    }
}

fn enable_match_source_cue(bytes: &mut [u8], ordinary: usize) -> TestResult {
    let flag = bytes
        .get_mut(ordinary + SOURCE_RULE_LEN + DESTINATION_RULE_LEN - 1)
        .ok_or("original match-source-cue flag")?;
    assert_eq!(*flag, 0, "the baseline rule has the field disabled");
    *flag = 1;
    Ok(())
}

fn word<const N: usize>(bytes: &mut [u8], offset: usize, value: [u8; N]) -> TestResult {
    bytes
        .get_mut(offset..offset + N)
        .ok_or("original transition word")?
        .copy_from_slice(&value);
    Ok(())
}
