// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! How far a `HIRC` object is decoded, and the parent and children it reports.

use std::collections::HashSet;

use p4kx_audio::{
    HircKind, HircLayout, HircObject, HircValue, MusicGroupKind, MusicPlayMode, MusicPlaylistItem,
    Soundbank,
};
use p4kx_testkit::{TestBank, TestContainerKind, TestHircObject, TestNodeBase, TestPlaylistItem};

type TestResult = Result<(), Box<dyn std::error::Error>>;

const PARENT: u32 = 0x2C21_1B2E;

fn layouts(bank: &TestBank) -> Result<Vec<HircLayout>, Box<dyn std::error::Error>> {
    let bytes = bank.bytes();
    let bank = Soundbank::parse(bytes.as_slice())?;
    Ok(bank.objects().iter().map(HircObject::layout).collect())
}

#[test]
fn decoded_undecoded_unsupported_and_malformed_objects_are_told_apart() -> TestResult {
    let bank = TestBank::new(1)
        .object(TestHircObject::event(1, &[2]))
        .object(TestHircObject::raw(7, 3, [0; 8]))
        .object(TestHircObject::raw(2, 4, [0; 3]));
    assert_eq!(
        layouts(&bank)?,
        [
            HircLayout::Decoded,
            HircLayout::Undecoded,
            HircLayout::Malformed
        ]
    );
    let old = TestBank::new(1)
        .version(140)
        .object(TestHircObject::sound(5, 6, 0, 1));
    assert_eq!(layouts(&old)?, [HircLayout::Unsupported], "sound below 150");
    let new = TestBank::new(1)
        .version(160)
        .object(TestHircObject::event(1, &[2]));
    assert_eq!(layouts(&new)?, [HircLayout::Unsupported], "above 154");
    Ok(())
}

#[test]
fn parent_and_children_follow_the_hierarchy() -> TestResult {
    let bytes = TestBank::new(1)
        .object(TestHircObject::event(1, &[10, 11]))
        .object(TestHircObject::sound(2, 99, 0, 1))
        .object(TestHircObject::container(
            TestContainerKind::Switch,
            3,
            &[2, 8],
            TestNodeBase::Plain,
        ))
        .object(TestHircObject::music_segment(4, &[5], &[]))
        .object(TestHircObject::music_track(5, &[70, 71]))
        .object(TestHircObject::music_playlist(6, &[4]))
        .object(TestHircObject::raw(7, 7, [0; 8]))
        .bytes();
    let bank = Soundbank::parse(bytes.as_slice())?;
    let report = |id| -> Result<(Option<u32>, Vec<u32>), Box<dyn std::error::Error>> {
        let object = bank.object(id).ok_or("object")?;
        Ok((object.parent_id(), object.children().collect()))
    };

    assert_eq!(report(1)?, (None, vec![10, 11]), "event");
    assert_eq!(report(2)?.1, Vec::<u32>::new(), "sound");
    assert_eq!(report(3)?.1, [2, 8], "container");
    assert_eq!(report(4)?, (Some(PARENT), vec![5]), "segment");
    assert_eq!(report(5)?, (Some(PARENT), vec![70, 71]), "track sources");
    assert_eq!(report(6)?.1, [4], "playlist");
    assert_eq!(report(7)?, (None, vec![]), "undecoded");
    Ok(())
}

#[test]
fn playlist_items_report_their_nesting_depth() -> TestResult {
    let bytes = TestBank::new(1)
        .object(TestHircObject::music_playlist_tree(
            6,
            &[4, 5],
            &[(0, 2), (4, 0), (0, 1), (5, 0), (4, 0)],
        ))
        .bytes();
    let bank = Soundbank::parse(bytes.as_slice())?;

    let Some(HircValue::MusicPlaylist(playlist)) = bank.object(6).map(|object| &object.value)
    else {
        return Err("a decoded playlist".into());
    };

    assert_eq!(playlist.item_depths().collect::<Vec<_>>(), [0, 1, 1, 2, 0]);
    Ok(())
}

#[test]
fn a_declared_child_count_beyond_the_items_does_not_grow_the_depth_stack() -> TestResult {
    let bytes = TestBank::new(1)
        .object(TestHircObject::music_playlist_tree(
            6,
            &[],
            &[(0, u32::MAX), (0, u32::MAX), (4, 0)],
        ))
        .bytes();
    let bank = Soundbank::parse(bytes.as_slice())?;

    let Some(HircValue::MusicPlaylist(playlist)) = bank.object(6).map(|object| &object.value)
    else {
        return Err("a decoded playlist".into());
    };

    assert_eq!(playlist.item_depths().collect::<Vec<_>>(), [0, 1, 2]);
    Ok(())
}

#[test]
fn play_modes_and_group_kinds_are_named() -> TestResult {
    let bytes = TestBank::new(1)
        .object(TestHircObject::music_playlist_items(
            6,
            &[],
            &[0, 1, 2, 3, 9].map(|mode| TestPlaylistItem::new(4, 0).play_mode(mode)),
        ))
        .object(TestHircObject::music_switch_track(7, &[1], 5, &[1]))
        .object(TestHircObject::music_state_track(8, &[1], 5, &[1]))
        .bytes();
    let bank = Soundbank::parse(bytes.as_slice())?;

    let Some(HircValue::MusicPlaylist(playlist)) = bank.object(6).map(|object| &object.value)
    else {
        return Err("a decoded playlist".into());
    };
    let modes: Vec<_> = playlist.items.iter().map(MusicPlaylistItem::mode).collect();
    assert_eq!(
        modes,
        [
            MusicPlayMode::Random,
            MusicPlayMode::SequenceContinuous,
            MusicPlayMode::RandomStep,
            MusicPlayMode::SequenceStep,
            MusicPlayMode::Other(9)
        ]
    );
    let names: Vec<_> = modes.iter().map(ToString::to_string).collect();
    assert_eq!(
        names,
        [
            "random",
            "sequence continuous",
            "random step",
            "sequence step",
            "play mode 9"
        ]
    );
    let kind = |id| -> Result<_, Box<dyn std::error::Error>> {
        match bank.object(id).map(|object| &object.value) {
            Some(HircValue::MusicTrack(track)) => {
                Ok(track.switch.as_ref().ok_or("a switch track")?.group_kind())
            }
            other => Err(format!("{other:?}").into()),
        }
    };
    assert_eq!(kind(7)?, MusicGroupKind::Switch);
    assert_eq!(kind(8)?, MusicGroupKind::State);
    assert_eq!(MusicGroupKind::Other(7).to_string(), "group type 7");
    Ok(())
}

#[test]
fn a_decoder_exists_for_exactly_the_kinds_the_reader_decodes() -> TestResult {
    let decoded = [
        TestHircObject::sound(1, 2, 0, 3),
        TestHircObject::action(2, 0x0403, 1),
        TestHircObject::event(3, &[2]),
        TestHircObject::container(TestContainerKind::Random, 4, &[1], TestNodeBase::Plain),
        TestHircObject::container(TestContainerKind::Switch, 5, &[1], TestNodeBase::Plain),
        TestHircObject::container(TestContainerKind::Blend, 6, &[1], TestNodeBase::Plain),
        TestHircObject::music_segment(7, &[8], &[]),
        TestHircObject::music_track(8, &[1]),
        TestHircObject::music_switch(9, &[7], 1, &[7]),
        TestHircObject::music_playlist(10, &[7]),
    ];
    let standalone_state = TestHircObject::raw(1, 11, [0; 2]);
    let undecoded: Vec<TestHircObject> = (2_u8..=22)
        .filter(|byte| ![2, 3, 4, 5, 6, 9, 10, 11, 12, 13].contains(byte))
        .zip(100_u32..)
        .map(|(byte, id)| TestHircObject::raw(byte, id, [0; 8]))
        .collect();
    assert_eq!(undecoded.len(), 11, "the kinds without a decoder");
    let bank = |version: u32| {
        decoded
            .iter()
            .chain(std::iter::once(&standalone_state))
            .chain(&undecoded)
            .cloned()
            .fold(TestBank::new(1).version(version), TestBank::object)
    };

    for (version, known) in [(154, HircLayout::Decoded), (160, HircLayout::Unsupported)] {
        let mut expected = vec![known; decoded.len()];
        expected.push(HircLayout::Unsupported);
        expected.extend([HircLayout::Undecoded; 11]);
        assert_eq!(
            layouts(&bank(version))?,
            expected,
            "State and raw kinds in v{version}"
        );
    }
    let older_state = TestBank::new(1).version(145).object(standalone_state);
    assert_eq!(
        layouts(&older_state)?,
        [HircLayout::Decoded],
        "empty standalone State v145"
    );
    Ok(())
}

#[test]
fn every_known_kind_has_its_own_slug() {
    let slugs: HashSet<&str> = (1..=22)
        .map(|byte| HircKind::from_byte(byte).slug())
        .collect();
    assert_eq!(slugs.len(), 22);
    assert_eq!(HircKind::Other(99).slug(), "other");
}
