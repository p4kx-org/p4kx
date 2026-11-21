// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;

use p4kx_testkit::{ArchiveBuilder, TestAtl, TestBank, TestEntry, TestHircObject, wwise_short_id};
use serde_json::json;

use super::txtp_origin_assertions::check_plain_fields;
use super::{EMBEDDED, Fixture, PLAY, TestResult, extract, hirc_payload, pcm};

const TRACK: &str = "bnkHircMusicTrack";
const TIMED: &str = "Play_Timed_Clips";
const ZERO: &str = "Play_Zero_Clip";
const NONFINITE: &str = "Play_Nonfinite_Clip";
const DEFAULT_SWITCH: &str = "Play_Default_Track_Switch";
const FLAGS: [&str; 3] = ["--format", "wav", "--txtp"];
const TIMING_CODE: &str = "omit-music-clip-timing";
const EVENT_CODE: &str = "omit-music-clip-events";
const AUTOMATION_CODE: &str = "omit-music-track-automation";
const CONDITIONS_CODE: &str = "omit-container-conditions";
const NONFINITE_CODE: &str = "nonfinite-music-clip-timing";
const FIRST_WEM: u32 = 201;
const SECOND_WEM: u32 = 202;
const ZERO_WEM: u32 = 203;
const NONFINITE_WEM: u32 = 204;
const POSTED_WEM: u32 = 205;
const DEFAULT_SWITCH_WEM: u32 = 206;
const TRACK_CACHE_MASK: u32 = 0x5A5A_5A5A;
const ID_BYTES: usize = size_of::<u32>();
const TIMING_BYTES: usize = 4 * size_of::<f64>();
const PCM_DURATION_MS: f64 = 10.0;

#[test]
fn txtp_reports_music_clip_and_track_automation_omissions() -> TestResult {
    let fixture = fixture()?;
    let report = extract(&fixture, TIMED, &FLAGS, 0)?;
    assert_eq!(
        report.pointer("/txtp/files/0/approximations"),
        Some(&json!([TIMING_CODE, EVENT_CODE, AUTOMATION_CODE])),
        "the omitted clip and automation features are recorded",
    );
    check_plain_fields(&report, TIMING_CODE, TRACK, &["musicTrackClipPlayAt"])?;
    check_plain_fields(&report, EVENT_CODE, TRACK, &["musicTrackClipEventId"])?;
    check_plain_fields(
        &report,
        AUTOMATION_CODE,
        TRACK,
        &["musicTrackAutomationPointCount"],
    )?;
    let text = fs::read_to_string(fixture.out.join(format!("{TIMED}.txtp")))?;
    for code in [TIMING_CODE, EVENT_CODE, AUTOMATION_CODE] {
        let header = format!("# approximation: {code}");
        assert!(
            text.lines().any(|line| line == header),
            "header agrees with JSON for {code}"
        );
    }
    assert!(
        text.lines().any(|line| line == "group = -L2 #@layer-v"),
        "existing playable layer syntax is retained"
    );
    assert!(
        !text.contains(&POSTED_WEM.to_string()),
        "the target of the authored clip Event is omitted"
    );
    Ok(())
}

#[test]
fn txtp_keeps_zero_timing_music_bytes_and_distinguishes_nonfinite_words() -> TestResult {
    let fixture = fixture()?;
    let zero = extract(&fixture, ZERO, &FLAGS, 0)?;
    assert_eq!(
        zero.pointer("/txtp/files/0/approximations"),
        Some(&json!([])),
        "neutral clip timing and an empty automation curve need no omission code"
    );
    assert_eq!(
        fs::read_to_string(fixture.out.join(format!("{ZERO}.txtp")))?,
        format!("# P4KX: event {ZERO} of trigger {ZERO}\n{ZERO}.wav\n"),
        "a representable clip keeps the exact simple artifact",
    );
    let unknown = extract(&fixture, NONFINITE, &FLAGS, 0)?;
    assert_eq!(
        unknown.pointer("/txtp/files/0/approximations"),
        Some(&json!([NONFINITE_CODE])),
        "nonfinite authored timing has a specific diagnostic"
    );
    check_plain_fields(&unknown, NONFINITE_CODE, TRACK, &["musicTrackClipPlayAt"])?;
    let text = fs::read_to_string(fixture.out.join(format!("{NONFINITE}.txtp")))?;
    let header = format!("# approximation: {NONFINITE_CODE}");
    assert!(
        text.lines().any(|line| line == header),
        "header marks the original nonfinite word"
    );
    Ok(())
}

#[test]
fn txtp_reports_an_authored_track_default_without_memberships() -> TestResult {
    let fixture = fixture()?;
    let report = extract(&fixture, DEFAULT_SWITCH, &FLAGS, 0)?;
    assert_eq!(
        report.pointer("/txtp/files/0/approximations"),
        Some(&json!([TIMING_CODE, CONDITIONS_CODE, AUTOMATION_CODE])),
        "the authored switch default is omitted even without explicit memberships",
    );
    check_plain_fields(
        &report,
        CONDITIONS_CODE,
        TRACK,
        &["musicTrackDefaultSwitchId"],
    )?;
    let text = fs::read_to_string(fixture.out.join(format!("{DEFAULT_SWITCH}.txtp")))?;
    assert!(
        text.lines()
            .any(|line| line == "# approximation: omit-container-conditions"),
        "the TXTP header reports the same omission",
    );
    Ok(())
}

fn fixture() -> Result<Fixture, Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let archive = dir.path().join("Data.p4k");
    let out = dir.path().join("out");
    let mut atl = TestAtl::new("Music");
    for event in [TIMED, ZERO, NONFINITE, DEFAULT_SWITCH] {
        atl = atl.trigger(event, &[event]);
    }
    let mut builder = ArchiveBuilder::new()
        .entry(TestEntry::zstd(
            "Data/Libs/GameAudio/Music.xml",
            atl.to_cryxml(),
        ))
        .entry(TestEntry::stored("Data/Sounds/wwise/Music.bnk", bank()?));
    for id in [
        FIRST_WEM,
        SECOND_WEM,
        ZERO_WEM,
        NONFINITE_WEM,
        DEFAULT_SWITCH_WEM,
    ] {
        builder = builder.entry(TestEntry::stored(
            format!("Data/Sounds/wwise/Media/{id}.wem"),
            pcm(),
        ));
    }
    builder.write_to(&archive)?;
    Ok(Fixture {
        _dir: dir,
        archive,
        out,
    })
}

fn bank() -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let posted = wwise_short_id("Play_Posted_Clip_Event");
    let mut bank = TestBank::new(wwise_short_id("Music"))
        .version(154)
        .wem(POSTED_WEM, pcm())
        .object(TestHircObject::event(posted, &[4]))
        .object(TestHircObject::action(4, PLAY, 20))
        .object(TestHircObject::sound(20, POSTED_WEM, EMBEDDED, 0));
    for (event, action, track, sources) in [
        (TIMED, 1, 10, &[FIRST_WEM, SECOND_WEM][..]),
        (ZERO, 2, 11, &[ZERO_WEM][..]),
        (NONFINITE, 3, 12, &[NONFINITE_WEM][..]),
        (DEFAULT_SWITCH, 5, 13, &[DEFAULT_SWITCH_WEM][..]),
    ] {
        let object = match event {
            TIMED => TestHircObject::music_track(track, sources),
            DEFAULT_SWITCH => music_track_with_default_switch(track, sources)?,
            _ => music_track_with_empty_automation(track, sources)?,
        };
        bank = bank
            .object(TestHircObject::event(wwise_short_id(event), &[action]))
            .object(TestHircObject::action(action, PLAY, track))
            .object(object);
    }
    let mut bytes = bank.bytes();
    for (ordinal, media, event, words) in [
        (1, FIRST_WEM, posted, [1.0, 0.25, 0.5, PCM_DURATION_MS]),
        (2, SECOND_WEM, 0, [2.0, 0.0, 0.25, PCM_DURATION_MS]),
        (1, ZERO_WEM, 0, [0.0, -0.0, 0.0, PCM_DURATION_MS]),
        (1, NONFINITE_WEM, 0, [f64::NAN, 0.0, 0.0, PCM_DURATION_MS]),
    ] {
        clip_words(&mut bytes, ordinal, media, event, words)?;
    }
    Ok(bytes)
}

fn music_track_with_default_switch(
    id: u32,
    sources: &[u32],
) -> Result<TestHircObject, Box<dyn std::error::Error>> {
    const GROUP: u32 = 0x4200_0001;
    const DEFAULT: u32 = 0x4200_0002;
    let mut payload = hirc_payload(TestHircObject::music_switch_track(id, sources, GROUP, &[]))?;
    let group = payload
        .windows(ID_BYTES)
        .position(|word| word == GROUP.to_le_bytes())
        .ok_or("original switch group ID")?;
    let default = payload
        .get_mut(group + ID_BYTES..group + 2 * ID_BYTES)
        .ok_or("default switch follows the group ID")?;
    assert_eq!(
        default,
        0_u32.to_le_bytes(),
        "the synthetic switch starts with no default"
    );
    default.copy_from_slice(&DEFAULT.to_le_bytes());
    Ok(TestHircObject::raw(11, id, payload))
}

pub(super) fn music_track_with_empty_automation(
    id: u32,
    sources: &[u32],
) -> Result<TestHircObject, Box<dyn std::error::Error>> {
    const TRACK_TAIL_LEN: usize = 31 + size_of::<u8>() + size_of::<i32>();
    let mut body = hirc_payload(TestHircObject::music_track(id, sources))?;
    let authored = [1_u32, 0, 0, 2, 0, 0, 0, 0, 0, 0].map(u32::to_le_bytes);
    let end = body
        .len()
        .checked_sub(TRACK_TAIL_LEN)
        .ok_or("original plain node and track type after the automation")?;
    let start = end
        .checked_sub(size_of_val(&authored))
        .ok_or("original automation with two points")?;
    assert_eq!(
        body.get(start..end),
        Some(authored.as_flattened()),
        "replace the original two-point automation row"
    );
    let empty = [1_u32, 0, 0, 0];
    drop(body.splice(start..end, empty.into_iter().flat_map(u32::to_le_bytes)));
    Ok(TestHircObject::raw(11, id, body))
}

pub(super) fn clip_words(
    bytes: &mut [u8],
    track_id: u32,
    source: u32,
    posted: u32,
    timing: [f64; 4],
) -> Result<usize, Box<dyn std::error::Error>> {
    let authored_header = [
        track_id.to_le_bytes(),
        source.to_le_bytes(),
        (source ^ TRACK_CACHE_MASK).to_le_bytes(),
        0_u32.to_le_bytes(),
    ];
    let header = authored_header.as_flattened();
    let clip_start = bytes
        .windows(header.len())
        .position(|candidate| candidate == header)
        .ok_or("original clip header")?;
    let start = clip_start + header.len() - ID_BYTES;
    let fields = bytes
        .get_mut(start..start + ID_BYTES + TIMING_BYTES)
        .ok_or("original clip timing")?;
    let (event, words) = fields.split_at_mut(ID_BYTES);
    event.copy_from_slice(&posted.to_le_bytes());
    let (words, []) = words.as_chunks_mut::<{ size_of::<f64>() }>() else {
        return Err("original clip timing words".into());
    };
    for (word, value) in words.iter_mut().zip(timing) {
        *word = value.to_le_bytes();
    }
    Ok(clip_start)
}
