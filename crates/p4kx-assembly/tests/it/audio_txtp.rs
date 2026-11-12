// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! TXTP files for the Wwise events of an audio trigger.

use p4kx_assembly::audio::{ExportState, wwise_id};
use p4kx_extraction::ConflictPolicy;
use p4kx_testkit::{TestContainerKind, TestHircObject};

use super::audio_txtp_support::{
    Case, EMBEDDED, PLAY, PREFETCH, TestResult, container, embedded, event, pcm, read, sound,
};

const PREFETCH_LEN: usize = 40;

#[test]
fn a_single_sound_is_one_file_line() -> TestResult {
    let wem = pcm().bytes();
    let size = wem.len();
    embedded(
        vec![(7001, wem)],
        vec![
            event(),
            TestHircObject::action(1, PLAY, 10),
            sound(10, 7001, EMBEDDED, size),
        ],
    )
    .run(|folder, report, _| {
        assert!(report.skipped.is_empty(), "{:?}", report.skipped);
        assert_eq!(
            read(folder, "Play_X.txtp")?,
            "# P4KX: event Play_X of trigger Play_X\nPlay_X.wav\n"
        );
        assert_eq!(report.written_count(), 1);
        Ok(())
    })
}

#[test]
fn a_random_container_is_a_selectable_group_over_its_sounds() -> TestResult {
    let wem = pcm().bytes();
    let size = wem.len();
    embedded(
        vec![(7001, wem), (7002, pcm().bytes()), (7003, pcm().bytes())],
        vec![
            event(),
            TestHircObject::action(1, PLAY, 20),
            container(TestContainerKind::Random, 20, &[10, 11, 12]),
            sound(10, 7001, EMBEDDED, size),
            sound(11, 7002, EMBEDDED, size),
            sound(12, 7003, EMBEDDED, size),
        ],
    )
    .run(|folder, _, _| {
        assert_eq!(
            read(folder, "Play_X.txtp")?,
            "# P4KX: event Play_X of trigger Play_X\n\
             # approximation: select-first-alternative\n\
             \x20 Play_X_7001.wav\n  Play_X_7002.wav\n  Play_X_7003.wav\n\
             # random 20: plays the first of 3, change >1 to pick another\n\
             group = -R3>1\n"
        );
        Ok(())
    })
}

#[test]
fn a_looping_sound_carries_the_loop_of_its_wem() -> TestResult {
    let wem = pcm().smpl_loop(0, 100, 399).bytes();
    let size = wem.len();
    embedded(
        vec![(7001, wem)],
        vec![
            event(),
            TestHircObject::action(1, PLAY, 10),
            sound(10, 7001, EMBEDDED, size),
        ],
    )
    .run(|folder, _, _| {
        assert_eq!(
            read(folder, "Play_X.txtp")?,
            "# P4KX: event Play_X of trigger Play_X\nPlay_X.wav #I 100 400\n"
        );
        Ok(())
    })
}

#[test]
fn several_actions_are_layers_and_a_sequence_nests_as_segments() -> TestResult {
    let wem = pcm().bytes();
    let size = wem.len();
    embedded(
        vec![(7001, wem), (7002, pcm().bytes()), (7003, pcm().bytes())],
        vec![
            TestHircObject::event(wwise_id("Play_X"), &[1, 2]),
            TestHircObject::action(1, PLAY, 10),
            TestHircObject::action(2, PLAY, 20),
            sound(10, 7001, EMBEDDED, size),
            container(TestContainerKind::Sequence, 20, &[11, 12]),
            sound(11, 7002, EMBEDDED, size),
            sound(12, 7003, EMBEDDED, size),
        ],
    )
    .run(|folder, _, _| {
        assert_eq!(
            read(folder, "Play_X.txtp")?,
            format!(
                "# P4KX: event Play_X of trigger Play_X\n\
                 \x20 Play_X_7001.wav\n\
                 \x20   Play_X_7002.wav\n    Play_X_7003.wav\n\
                 \x20 # sequence 20\n\
                 \x20 group = -S2\n\
                 # event {}\n\
                 group = -L2 #@layer-v\n",
                wwise_id("Play_X")
            )
        );
        Ok(())
    })
}

#[test]
fn an_event_with_a_sound_that_has_no_file_is_listed_not_written() -> TestResult {
    let wem = pcm().bytes();
    let size = wem.len();
    embedded(
        vec![(7001, wem)],
        vec![
            TestHircObject::event(wwise_id("Play_X"), &[1, 2]),
            TestHircObject::action(1, PLAY, 10),
            TestHircObject::action(2, PLAY, 11),
            sound(10, 7001, EMBEDDED, size),
            TestHircObject::plugin_sound(11, 9),
        ],
    )
    .run(|folder, report, _| {
        assert_eq!(report.files.len(), 0);
        let [skip] = report.skipped.as_slice() else {
            return Err("one event is skipped".into());
        };
        assert_eq!(
            (skip.trigger.as_str(), skip.event.as_str()),
            ("Play_X", "Play_X")
        );
        assert_eq!(skip.reason, "plug-in source 9 has no file");
        assert!(!folder.join("Play_X.txtp").exists());
        Ok(())
    })
}

#[test]
fn a_prefetched_start_of_a_streamed_sound_is_no_file_to_play() -> TestResult {
    let whole = pcm().bytes();
    let start = whole.get(..PREFETCH_LEN).ok_or("long enough")?.to_vec();
    embedded(
        vec![(7001, start)],
        vec![
            event(),
            TestHircObject::action(1, PLAY, 10),
            sound(10, 7001, PREFETCH, PREFETCH_LEN),
        ],
    )
    .run(|_, report, _| {
        assert_eq!(report.files.len(), 0);
        let [skip] = report.skipped.as_slice() else {
            return Err("one event is skipped".into());
        };
        assert_eq!(
            skip.reason,
            "prefetched streamed source 7001: only its start was written, the rest is streamed"
        );
        Ok(())
    })
}

#[test]
fn the_conflict_policy_decides_about_an_existing_file() -> TestResult {
    let wem = pcm().bytes();
    let size = wem.len();
    embedded(
        vec![(7001, wem)],
        vec![
            event(),
            TestHircObject::action(1, PLAY, 10),
            sound(10, 7001, EMBEDDED, size),
        ],
    )
    .run(|folder, report, again| {
        assert_eq!(
            report.files.first().map(|file| file.state),
            Some(ExportState::Written)
        );
        assert_eq!(again(ConflictPolicy::Skip).kept_count(), 1);
        assert_eq!(again(ConflictPolicy::IfChanged).kept_count(), 1);
        assert_eq!(again(ConflictPolicy::Overwrite).written_count(), 1);
        assert!(folder.join("Play_X.txtp").is_file());
        Ok(())
    })
}

#[test]
fn a_music_track_layers_its_sources_below_a_playlist_and_segment() -> TestResult {
    Case {
        loose: vec![(7001, pcm().bytes()), (7002, pcm().bytes())],
        objects: vec![
            event(),
            TestHircObject::action(1, PLAY, 30),
            TestHircObject::music_playlist(30, &[31]),
            TestHircObject::music_segment(31, &[32], &[]),
            TestHircObject::music_track(32, &[7001, 7002]),
        ],
        ..Case::default()
    }
    .run(|folder, _, _| {
        assert_eq!(
            read(folder, "Play_X.txtp")?,
            "# P4KX: event Play_X of trigger Play_X\n\
             \x20 Play_X_7001.wav\n  Play_X_7002.wav\n\
             # music_track 32\n\
             group = -L2 #@layer-v\n"
        );
        Ok(())
    })
}
