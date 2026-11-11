// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::io::{Cursor, Read, Seek};

use p4kx_audio::{
    BankHirc, BankHircField, BankHircSemantic, BankMusicTrackRead, BankMusicTrackRow,
    BankMusicTrackRows, BankMusicTrackSource, BankObservedHeader, BankSource, HircValue,
    MediaStorage, MusicTrackKind, SoundSource, Soundbank,
};
use p4kx_testkit::{TestBank, TestHircObject};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

const TRACK_TYPE: u8 = 11;
const TRACK_ID: u32 = 71;
const PLUGIN_ID: u32 = 0x0065_0002;
const SOURCE_PREFIX_BYTES: usize = 18;
const SOURCE_FLAGS: u8 = 0x85;
const STORAGE_CODE: u8 = 0xa5;
const TRACK_CODE: u8 = 0xe7;
const PARAMETERS: [u8; 3] = [0x79, 0xf2, 0x80];
const SOURCE_IDS: [u32; 2] = [0x3100_0001, 0x3100_0001];
const MEMBERS: [u32; 2] = [0x5200_004a, 0x5200_004a];

#[test]
fn music_track_replays_original_source_and_clip_words_and_unknown_kind() -> TestResult {
    let bytes = source_fixture()?;
    let legacy = Soundbank::parse(bytes.as_slice())?;
    let HircValue::MusicTrack(expected) = &legacy.objects().first().ok_or("original track")?.value
    else {
        return Err("slice reader decoded the MusicTrack".into());
    };
    let mut bank = open(&bytes)?;
    let (header, mut hirc) = parts(&mut bank)?;
    let Some(BankHircSemantic::MusicTrack(track)) = hirc.next_semantic(&header).transpose()? else {
        return Err("seek reader decoded the MusicTrack".into());
    };
    assert!(
        matches!(track.status, BankMusicTrackRead::Decoded),
        "layout fits the original"
    );
    assert_eq!(
        track.fields.kind(),
        Some(MusicTrackKind::Other(TRACK_CODE)),
        "known projection keeps the unknown code"
    );
    let kind = track.fields.kind_code.as_ref().ok_or("original kind")?;
    assert_eq!(
        original(&bytes, kind)?,
        [TRACK_CODE],
        "original kind range moved after the plug-in"
    );
    assert_eq!(
        track.fields.kind(),
        Some(expected.kind),
        "slice and seek agree"
    );
    assert_eq!(
        track
            .fields
            .parent
            .parent_id
            .as_ref()
            .map(|word| word.value),
        Some(expected.parent_id),
        "original node parent"
    );
    assert!(
        track.fields.switch.is_none(),
        "non-switch code selects no switch block"
    );

    check_rows(&bytes, track.rows, &expected.sources)
}

fn check_rows<R: Read + Seek>(
    bytes: &[u8],
    rows: BankMusicTrackRows<'_, R>,
    expected_sources: &[SoundSource],
) -> TestResult {
    let mut sources = Vec::new();
    let mut clips = Vec::new();
    for row in rows {
        match row? {
            BankMusicTrackRow::Source { ordinal, value } => {
                if ordinal == 0 {
                    check_source(bytes, &value)?;
                }
                sources.push((ordinal, value.value));
            }
            BankMusicTrackRow::Clip { ordinal, value } => {
                assert_eq!(
                    original(bytes, &value.begin_trim)?,
                    0.5_f64.to_le_bytes(),
                    "raw clip trim word"
                );
                clips.push((ordinal, value.source_id.value, value.begin_trim.value));
            }
            _ => return Err("unexpected row in the non-switch track".into()),
        }
    }
    assert_eq!(
        sources.iter().map(|(index, _)| *index).collect::<Vec<_>>(),
        [0, 1],
        "ordered repeated physical source rows"
    );
    assert_eq!(
        sources
            .into_iter()
            .map(|(_, value)| value)
            .collect::<Vec<_>>(),
        expected_sources,
        "all SoundSource projections match legacy"
    );
    let [first, second] = SOURCE_IDS;
    assert_eq!(
        clips,
        [
            (0, first, 0.5_f64.to_bits()),
            (1, second, 0.5_f64.to_bits())
        ],
        "original duplicate clip IDs survive"
    );
    Ok(())
}

#[test]
fn music_track_keeps_switch_members_when_the_final_word_is_truncated() -> TestResult {
    let object = TestHircObject::music_state_track(TRACK_ID, &SOURCE_IDS, 0x4100_0001, &MEMBERS);
    let valid = TestBank::new(1).object(object).bytes();
    let body = content(&valid)?;
    let truncated = body
        .get(..body.len() - 1)
        .ok_or("truncate final track word")?;
    let bytes = TestBank::new(1)
        .object(TestHircObject::raw(TRACK_TYPE, TRACK_ID, truncated))
        .bytes();
    let mut bank = open(&bytes)?;
    let (header, mut hirc) = parts(&mut bank)?;
    let Some(BankHircSemantic::MusicTrack(track)) = hirc.next_semantic(&header).transpose()? else {
        return Err("physical malformed track".into());
    };
    assert!(
        matches!(track.status, BankMusicTrackRead::Malformed(_)),
        "truncated tail remains malformed"
    );
    let switch = track
        .fields
        .switch
        .as_ref()
        .ok_or("authored switch branch")?;
    assert_eq!(
        switch.group_type.as_ref().map(|word| word.value),
        Some(1),
        "original state group"
    );
    assert_eq!(
        switch.member_count.as_ref().map(|word| word.value),
        Some(2),
        "original switch member count"
    );
    let mut members = Vec::new();
    for row in track.rows {
        if let BankMusicTrackRow::Switch { ordinal, id } = row? {
            assert_eq!(
                original(&bytes, &id)?,
                id.value.to_le_bytes(),
                "each switch row retains its original word"
            );
            members.push((ordinal, id.value));
        }
    }
    let [first, second] = MEMBERS;
    assert_eq!(
        members,
        [(0, first), (1, second)],
        "earlier duplicate member rows remain available"
    );
    Ok(())
}

#[test]
fn music_track_layout_is_not_reused_for_other_bank_versions() -> TestResult {
    for version in [153, 155] {
        let bytes = TestBank::new(1)
            .version(version)
            .object(TestHircObject::music_track(TRACK_ID, &SOURCE_IDS))
            .bytes();
        let mut bank = open(&bytes)?;
        let (header, mut hirc) = parts(&mut bank)?;
        let actual = hirc
            .next_semantic(&header)
            .transpose()?
            .ok_or("physical track object")?;
        match actual {
            BankHircSemantic::Raw(_) if version == 153 => {}
            BankHircSemantic::MusicTrack(track) if version == 155 => {
                assert!(
                    matches!(track.status, BankMusicTrackRead::Unsupported(_)),
                    "newer version is unsupported"
                );
                assert!(
                    track.fields.kind_code.is_none(),
                    "unknown layout supplies no guessed kind"
                );
                assert_eq!(
                    track.rows.count(),
                    0,
                    "unknown layout has no typed children"
                );
            }
            _ => return Err("wrong version-dependent track layout".into()),
        }
    }
    Ok(())
}

fn check_source(bytes: &[u8], source: &BankMusicTrackSource) -> TestResult {
    assert_eq!(
        source.value.storage,
        MediaStorage::Other(STORAGE_CODE),
        "unknown storage stays typed"
    );
    let plugin = source
        .fields
        .plugin_id
        .as_ref()
        .ok_or("original plug-in word")?;
    let storage = source
        .fields
        .storage_code
        .as_ref()
        .ok_or("original storage byte")?;
    let flags = source.fields.source_bits.as_ref().ok_or("original flags")?;
    let cache = source
        .fields
        .cache_id
        .as_ref()
        .ok_or("original cache word")?;
    assert_eq!(
        original(bytes, plugin)?,
        PLUGIN_ID.to_le_bytes(),
        "plug-in word range"
    );
    assert_eq!(
        original(bytes, storage)?,
        [STORAGE_CODE],
        "storage byte range"
    );
    assert_eq!(
        original(bytes, flags)?,
        [SOURCE_FLAGS],
        "source flag byte range"
    );
    let [first, _] = SOURCE_IDS;
    assert_eq!(
        original(bytes, cache)?,
        (first ^ 0x5a5a_5a5a).to_le_bytes(),
        "cache word range"
    );
    let parameters = source
        .plugin_parameters
        .as_ref()
        .ok_or("original plug-in bytes")?;
    let span = usize::try_from(parameters.start)?..usize::try_from(parameters.end)?;
    assert_eq!(
        bytes.get(span),
        Some(PARAMETERS.as_slice()),
        "checked plug-in extent"
    );
    Ok(())
}

fn source_fixture() -> TestResult<Vec<u8>> {
    let base = TestBank::new(1)
        .object(TestHircObject::music_track(TRACK_ID, &SOURCE_IDS))
        .bytes();
    let mut body = content(&base)?.to_vec();
    let start = size_of::<u32>();
    let plugin = body
        .get_mut(start..start + size_of::<u32>())
        .ok_or("fixture plug-in word")?;
    plugin.copy_from_slice(&PLUGIN_ID.to_le_bytes());
    *body
        .get_mut(start + size_of::<u32>())
        .ok_or("fixture storage byte")? = STORAGE_CODE;
    let end = start + SOURCE_PREFIX_BYTES;
    *body.get_mut(end - 1).ok_or("fixture flags")? = SOURCE_FLAGS;
    let kind = body
        .len()
        .checked_sub(size_of::<i32>() + 1)
        .ok_or("fixture non-switch kind")?;
    *body.get_mut(kind).ok_or("fixture kind byte")? = TRACK_CODE;
    let length = u32::try_from(PARAMETERS.len())?;
    body.splice(end..end, length.to_le_bytes().into_iter().chain(PARAMETERS));
    Ok(TestBank::new(1)
        .object(TestHircObject::raw(TRACK_TYPE, TRACK_ID, body))
        .bytes())
}

fn content(bytes: &[u8]) -> TestResult<&[u8]> {
    let bank = Soundbank::parse(bytes)?;
    let object = bank.objects().first().ok_or("original fixture object")?;
    let mut source = open(bytes)?;
    let (_, mut hirc) = parts(&mut source)?;
    let physical = hirc.next().transpose()?.ok_or("physical fixture object")?;
    let start = usize::try_from(physical.body_offset())?;
    let end = start + bank.object_body(object).len();
    bytes
        .get(start + size_of::<u32>()..end)
        .ok_or_else(|| "fixture object content".into())
}

fn original<'a, T>(bytes: &'a [u8], word: &BankHircField<T>) -> TestResult<&'a [u8]> {
    let range = usize::try_from(word.range.start)?..usize::try_from(word.range.end)?;
    bytes
        .get(range)
        .ok_or_else(|| "original scalar outside fixture".into())
}

fn open(bytes: &[u8]) -> TestResult<BankSource<Cursor<&[u8]>>> {
    Ok(BankSource::open(
        Cursor::new(bytes),
        u64::try_from(bytes.len())?,
    )?)
}

fn parts<'bank, 'bytes>(
    bank: &'bank mut BankSource<Cursor<&'bytes [u8]>>,
) -> TestResult<(BankObservedHeader, BankHirc<'bank, Cursor<&'bytes [u8]>>)> {
    let mut sections = bank.section_cursor();
    let header = bank
        .next_section(&mut sections)
        .transpose()?
        .ok_or("BKHD")?;
    let hirc = bank
        .next_section(&mut sections)
        .transpose()?
        .ok_or("HIRC")?;
    Ok((bank.read_observed_header_for(&header)?, bank.hirc(&hirc)?))
}
