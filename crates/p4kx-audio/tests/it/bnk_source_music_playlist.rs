// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::io::Cursor;

use p4kx_audio::{
    BankHirc, BankHircField, BankHircSemantic, BankMusicPlaylistFields, BankMusicPlaylistItem,
    BankMusicPlaylistRead, BankMusicPlaylistRow, BankObservedHeader, BankSource, HircValue,
    MusicPlayMode, MusicPlaylistItem, Soundbank,
};
use p4kx_testkit::{TestBank, TestHircObject, TestPlaylistItem};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

const TYPE: u8 = 13;
const PLAYLIST: u32 = 81;
const CHILD: u32 = 0xa453_91c2;
const PLAY_MODE: u32 = 0xe7d0_80f1;
const LOOP_COUNT: i16 = -7;
const WEIGHT_BYTE: u8 = 0x7f;
const SHUFFLE_BYTE: u8 = 0x81;
const ITEM_BYTES: usize = 30;
const RULE_BYTES: usize = 166;

#[test]
fn music_playlist_keeps_original_items_and_opaque_transition_rules() -> TestResult {
    let bytes = fixture(154)?;
    let slice = Soundbank::parse(bytes.as_slice())?;
    let HircValue::MusicPlaylist(legacy) =
        &slice.objects().first().ok_or("fixture playlist")?.value
    else {
        return Err("legacy playlist layout".into());
    };
    let mut source = open(&bytes)?;
    let (header, mut hirc) = parts(&mut source)?;
    let Some(BankHircSemantic::MusicPlaylist(playlist)) =
        hirc.next_semantic(&header).transpose()?
    else {
        return Err("checked playlist layout".into());
    };
    assert!(
        matches!(playlist.status, BankMusicPlaylistRead::Decoded),
        "known layout"
    );
    check_rules(&bytes, &playlist.fields)?;
    let mut items = Vec::new();
    let mut children = Vec::new();
    for row in playlist.rows {
        match row? {
            BankMusicPlaylistRow::Item { ordinal, value } => {
                if ordinal == 1 {
                    check_last_item(&bytes, &value)?;
                }
                items.push(value.value());
            }
            BankMusicPlaylistRow::Child { id, .. } => children.push(id.value),
            BankMusicPlaylistRow::Stinger { .. } => {}
            _ => return Err("unexpected playlist row".into()),
        }
    }
    assert_eq!(
        children,
        [CHILD, CHILD],
        "duplicate original child occurrences"
    );
    assert_eq!(
        items, legacy.items,
        "checked and legacy values agree in authored order"
    );
    assert_eq!(
        items.get(1).map(MusicPlaylistItem::mode),
        Some(MusicPlayMode::Other(PLAY_MODE)),
        "the legacy interpretation retains the original unknown play mode"
    );
    Ok(())
}

#[test]
fn music_playlist_replays_legacy_item_depth_with_the_exact_predecessor_words() -> TestResult {
    let bytes = fixture(154)?;
    let slice = Soundbank::parse(bytes.as_slice())?;
    let HircValue::MusicPlaylist(legacy) =
        &slice.objects().first().ok_or("fixture playlist")?.value
    else {
        return Err("legacy MusicPlaylist layout".into());
    };
    let mut source = open(&bytes)?;
    let (header, mut hirc) = parts(&mut source)?;
    let Some(BankHircSemantic::MusicPlaylist(playlist)) =
        hirc.next_semantic(&header).transpose()?
    else {
        return Err("checked MusicPlaylist layout".into());
    };
    let first_child_count = u64::try_from(bytes.len() - 2 * ITEM_BYTES + 2 * size_of::<u32>())?;
    let mut rows = playlist.rows;
    let mut depths = Vec::new();
    while let Some(row) = rows.next() {
        if let BankMusicPlaylistRow::Item { ordinal, .. } = row? {
            let observed = rows.current_item_depth().ok_or("current playlist depth")?;
            let expected = (ordinal != 0).then_some([first_child_count, 1, 30, 4]);
            assert_eq!(
                observed.preceding_child_counts(),
                expected.as_ref(),
                "only previous child count words select the depth"
            );
            depths.push(observed.value);
        }
    }
    assert_eq!(depths, [0, 1], "authored group and its child");
    assert_eq!(
        depths,
        legacy.item_depths().collect::<Vec<_>>(),
        "same nesting exposed by ObjectView"
    );
    assert!(
        rows.current_item_depth().is_none(),
        "no current item after exhaustion"
    );
    Ok(())
}

fn check_rules(bytes: &[u8], fields: &BankMusicPlaylistFields) -> TestResult {
    let rules = fields
        .transitions
        .rule_count
        .as_ref()
        .ok_or("transition count")?;
    assert_eq!(rules.value, 2, "both original rule records");
    let range = fields
        .transitions
        .rules_range
        .as_ref()
        .ok_or("checked transition bytes")?;
    let start = bytes.len() - 2 * ITEM_BYTES - size_of::<u32>() - RULE_BYTES;
    assert_eq!(
        range.clone(),
        u64::try_from(start)?..u64::try_from(start + RULE_BYTES)?,
        "the original two transition rules retain their exact byte extent"
    );
    Ok(())
}

#[test]
fn music_playlist_keeps_readable_item_prefix_and_gates_unknown_versions() -> TestResult {
    for version in [153, 154, 155] {
        let bytes = fixture(version)?;
        let input = if version == 154 {
            truncate(&bytes)?
        } else {
            bytes
        };
        let mut source = open(&input)?;
        let (header, mut hirc) = parts(&mut source)?;
        match hirc
            .next_semantic(&header)
            .transpose()?
            .ok_or("physical playlist")?
        {
            BankHircSemantic::Raw(_) if version == 153 => {}
            BankHircSemantic::MusicPlaylist(playlist) if version == 154 => {
                assert!(matches!(
                    playlist.status,
                    BankMusicPlaylistRead::Malformed(_)
                ));
                assert_eq!(
                    playlist.fields.item_count.as_ref().map(|count| count.value),
                    Some(2)
                );
                let mut indices = Vec::new();
                for row in playlist.rows {
                    if let BankMusicPlaylistRow::Item { ordinal, .. } = row? {
                        indices.push(ordinal);
                    }
                }
                assert_eq!(
                    indices,
                    [0],
                    "one complete item before the truncated second item"
                );
            }
            BankHircSemantic::MusicPlaylist(playlist) if version == 155 => {
                assert!(matches!(
                    playlist.status,
                    BankMusicPlaylistRead::Unsupported(_)
                ));
                assert!(
                    playlist.fields.item_count.is_none(),
                    "unknown layout supplies no guessed count"
                );
                assert_eq!(playlist.rows.count(), 0);
            }
            _ => return Err("wrong version-dependent playlist semantics".into()),
        }
    }
    Ok(())
}

fn check_last_item(bytes: &[u8], value: &BankMusicPlaylistItem) -> TestResult {
    let expected = bytes.len() - ITEM_BYTES..bytes.len();
    let actual = usize::try_from(value.range.start)?..usize::try_from(value.range.end)?;
    assert_eq!(actual, expected, "exact original item extent");
    check_word_ranges(value);
    assert_eq!(value.segment_id.value, CHILD, "original segment ID");
    assert_eq!(value.item_id.value, 2, "original item ID");
    assert_eq!(value.children.value, 0, "original child count");
    assert_eq!(
        value.play_mode.value, PLAY_MODE,
        "original unknown mode word"
    );
    assert_eq!(value.loops.value, LOOP_COUNT, "original signed loop count");
    assert_eq!(value.loop_min.value, 2, "original minimum loop count");
    assert_eq!(value.loop_max.value, 5, "original maximum loop count");
    assert_eq!(value.weight.value, 50_000, "original weight word");
    assert_eq!(
        value.avoid_repeat_count.value, 1,
        "original avoid-repeat count"
    );
    assert_eq!(
        original(bytes, &value.uses_weight_code)?,
        [WEIGHT_BYTE],
        "raw weight selector"
    );
    assert_eq!(
        original(bytes, &value.shuffle_code)?,
        [SHUFFLE_BYTE],
        "raw shuffle selector"
    );
    assert!(
        value.value().uses_weight && value.value().shuffle,
        "legacy nonzero semantics"
    );
    Ok(())
}

fn check_word_ranges(item: &BankMusicPlaylistItem) {
    let start = item.range.start;
    for (range, offset, width) in [
        (&item.segment_id.range, 0, 4),
        (&item.item_id.range, 4, 4),
        (&item.children.range, 8, 4),
        (&item.play_mode.range, 12, 4),
        (&item.loops.range, 16, 2),
        (&item.loop_min.range, 18, 2),
        (&item.loop_max.range, 20, 2),
        (&item.weight.range, 22, 4),
        (&item.avoid_repeat_count.range, 26, 2),
        (&item.uses_weight_code.range, 28, 1),
        (&item.shuffle_code.range, 29, 1),
    ] {
        let offset = start + offset;
        assert_eq!(range, &(offset..offset + width), "authored word extent");
    }
}

fn fixture(version: u32) -> TestResult<Vec<u8>> {
    let items = [
        TestPlaylistItem::new(0, 1),
        TestPlaylistItem::new(CHILD, 0)
            .play_mode(PLAY_MODE)
            .loops(LOOP_COUNT),
    ];
    let mut bytes = TestBank::new(1)
        .version(version)
        .object(TestHircObject::music_playlist_items(
            PLAYLIST,
            &[CHILD, CHILD],
            &items,
        ))
        .bytes();
    let flags = bytes
        .last_chunk_mut::<2>()
        .ok_or("last item's flag bytes")?;
    *flags = [WEIGHT_BYTE, SHUFFLE_BYTE];
    Ok(bytes)
}

fn truncate(bytes: &[u8]) -> TestResult<Vec<u8>> {
    let mut source = open(bytes)?;
    let (_, mut hirc) = parts(&mut source)?;
    let object = hirc
        .next()
        .transpose()?
        .ok_or("original physical playlist")?;
    let start = usize::try_from(object.body_offset())? + size_of::<u32>();
    let end = bytes.len() - 1;
    let partial = bytes.get(start..end).ok_or("short playlist tail")?;
    Ok(TestBank::new(1)
        .object(TestHircObject::raw(TYPE, PLAYLIST, partial))
        .bytes())
}

fn original<'a, T>(bytes: &'a [u8], field: &BankHircField<T>) -> TestResult<&'a [u8]> {
    let range = usize::try_from(field.range.start)?..usize::try_from(field.range.end)?;
    bytes
        .get(range)
        .ok_or_else(|| "field outside original fixture".into())
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
