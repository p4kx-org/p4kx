// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! TXTP files of events with broken links, odd names, other formats and long paths.

use p4kx_assembly::audio::{SoundFormat, wwise_id};
use p4kx_extraction::check_part;
use p4kx_testkit::{TestContainerKind, TestHircObject};

use super::audio_txtp_support::{
    Case, EMBEDDED, PLAY, TestResult, container, embedded, pcm, read, sound,
};

const LONG_EVENT_LEN: usize = 118;
const LONG_FOLDER_LEN: usize = 140;

/// The objects of the events `events`, which all play the sound 10 of the WEM 7001 through the
/// action 1.
fn one_sound(events: &[&str], wem: &[u8]) -> Vec<TestHircObject> {
    let mut objects: Vec<_> = events
        .iter()
        .map(|name| TestHircObject::event(wwise_id(name), &[1]))
        .collect();
    objects.push(TestHircObject::action(1, PLAY, 10));
    objects.push(sound(10, 7001, EMBEDDED, wem.len()));
    objects
}

#[test]
fn an_event_with_a_broken_link_is_written_as_incomplete() -> TestResult {
    let wem = pcm().bytes();
    let size = wem.len();
    embedded(
        vec![(7001, wem)],
        vec![
            TestHircObject::event(wwise_id("Play_X"), &[1, 2]),
            TestHircObject::action(1, PLAY, 10),
            TestHircObject::action(2, PLAY, 99),
            sound(10, 7001, EMBEDDED, size),
        ],
    )
    .run(|folder, report, _| {
        assert_eq!(report.files.first().map(|file| file.breaks), Some(1));
        assert_eq!(
            read(folder, "Play_X.txtp")?,
            "# P4KX: event Play_X of trigger Play_X\n\
             # incomplete: 1 broken links; the objects behind them are not played here\n\
             Play_X.wav\n"
        );
        Ok(())
    })
}

#[test]
fn a_switch_container_is_a_selectable_group() -> TestResult {
    let wem = pcm().bytes();
    let size = wem.len();
    embedded(
        vec![(7001, wem), (7002, pcm().bytes())],
        vec![
            TestHircObject::event(wwise_id("Play_X"), &[1]),
            TestHircObject::action(1, PLAY, 20),
            container(TestContainerKind::Switch, 20, &[10, 11]),
            sound(10, 7001, EMBEDDED, size),
            sound(11, 7002, EMBEDDED, size),
        ],
    )
    .run(|folder, _, _| {
        assert_eq!(
            read(folder, "Play_X.txtp")?,
            "# P4KX: event Play_X of trigger Play_X\n\
             # approximation: select-first-alternative\n\
             \x20 Play_X_7001.wav\n  Play_X_7002.wav\n\
             # switch 20: plays the first of 2, change >1 to pick another\n\
             group = -R2>1\n"
        );
        Ok(())
    })
}

#[test]
fn events_whose_names_differ_only_in_case_get_numbered_files() -> TestResult {
    let wem = pcm().bytes();
    Case {
        events: vec!["Play_A".to_owned(), "play_a".to_owned()],
        objects: one_sound(&["Play_A"], &wem),
        wems: vec![(7001, wem)],
        ..Case::default()
    }
    .run(|folder, report, _| {
        let names: Vec<_> = report
            .files
            .iter()
            .filter_map(|file| file.path.file_name()?.to_str())
            .collect();
        assert_eq!(names, ["Play_A.txtp", "play_a_2.txtp"]);
        assert!(folder.join("play_a_2.txtp").is_file());
        Ok(())
    })
}

#[test]
fn a_control_character_in_an_event_name_cannot_start_a_line() -> TestResult {
    let name = "Play\nX";
    let wem = pcm().bytes();
    Case {
        events: vec![name.to_owned()],
        objects: one_sound(&[name], &wem),
        wems: vec![(7001, wem)],
        ..Case::default()
    }
    .run(|folder, report, _| {
        assert_eq!(report.files.len(), 1, "{:?}", report.skipped);
        assert_eq!(
            read(folder, "Play%0AX.txtp")?,
            "# P4KX: event Play_X of trigger Play_X\nPlay_X.wav\n"
        );
        Ok(())
    })
}

/// The names of the TXTP files of the events `events`, which all play one sound.
fn txtp_names(events: &[&str]) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let wem = pcm().bytes();
    let names = std::cell::RefCell::new(Vec::new());
    Case {
        events: events.iter().map(|name| (*name).to_owned()).collect(),
        objects: one_sound(events, &wem),
        wems: vec![(7001, wem)],
        ..Case::default()
    }
    .run(|folder, report, _| {
        assert!(report.skipped.is_empty(), "{:?}", report.skipped);
        for file in &report.files {
            let name = file.path.file_name().and_then(|name| name.to_str());
            let name = name.ok_or("a file name")?.to_owned();
            assert_eq!(check_part(&name), Ok(()), "{name:?} passes ADR-0248");
            assert!(folder.join(&name).is_file(), "{name:?} is written");
            names.borrow_mut().push(name);
        }
        Ok(())
    })?;
    Ok(names.into_inner())
}

#[test]
fn a_file_line_equals_the_written_file_for_names_with_a_hash_or_a_leading_space() -> TestResult {
    for (trigger, file) in [
        ("Play #1", "Play %231.wav"),
        ("#x", "%23x.wav"),
        (" x", "%20x.wav"),
    ] {
        let wem = pcm().bytes();
        Case {
            trigger: Some(trigger.to_owned()),
            objects: one_sound(&["Play_X"], &wem),
            wems: vec![(7001, wem)],
            ..Case::default()
        }
        .run(|folder, report, _| {
            assert!(report.skipped.is_empty(), "{:?}", report.skipped);
            assert!(folder.join(file).is_file(), "{file:?} is written");
            let text = read(folder, "Play_X.txtp")?;
            assert_eq!(text.lines().last(), Some(file), "{trigger:?}");
            Ok(())
        })?;
    }
    Ok(())
}

#[test]
fn a_device_name_or_a_forbidden_character_in_an_event_name_is_escaped() -> TestResult {
    let names = txtp_names(&["NUL", "Play|X:Y", "a."])?;
    assert_eq!(names, ["%4EUL.txtp", "Play%7CX%3AY.txtp", "a%2E.txtp"]);
    Ok(())
}

#[test]
fn event_names_that_differ_only_in_non_ascii_case_stay_distinct() -> TestResult {
    let names = txtp_names(&["Ärger", "ärger"])?;
    assert_eq!(names, ["Ärger.txtp", "ärger_2.txtp"]);
    Ok(())
}

#[test]
fn an_event_name_too_long_for_a_file_name_becomes_event() -> TestResult {
    let long = "E".repeat(300);
    let names = txtp_names(&[&long, "Play_X"])?;
    assert_eq!(names, ["event.txtp", "Play_X.txtp"]);
    Ok(())
}

#[test]
fn the_raw_format_keeps_the_loop_in_the_txtp() -> TestResult {
    let wem = pcm().smpl_loop(0, 100, 399).bytes();
    Case {
        format: Some(SoundFormat::Raw),
        objects: one_sound(&["Play_X"], &wem),
        wems: vec![(7001, wem)],
        ..Case::default()
    }
    .run(|folder, _, _| {
        assert_eq!(
            read(folder, "Play_X.txtp")?,
            "# P4KX: event Play_X of trigger Play_X\nPlay_X.wem #I 100 400\n"
        );
        Ok(())
    })
}

#[test]
fn a_kept_sound_file_keeps_its_loop_in_the_txtp() -> TestResult {
    let wem = pcm().smpl_loop(0, 100, 399).bytes();
    Case {
        reexport: true,
        objects: one_sound(&["Play_X"], &wem),
        wems: vec![(7001, wem)],
        ..Case::default()
    }
    .run(|folder, report, _| {
        assert_eq!(report.skipped.len(), 0);
        assert_eq!(
            read(folder, "Play_X.txtp")?,
            "# P4KX: event Play_X of trigger Play_X\nPlay_X.wav #I 100 400\n"
        );
        Ok(())
    })
}

#[test]
fn a_path_over_the_limit_is_reported_for_its_event_and_the_run_goes_on() -> TestResult {
    let long = "E".repeat(LONG_EVENT_LEN);
    let wem = pcm().bytes();
    Case {
        events: vec![long.clone(), "Play_X".to_owned()],
        folder: Some("F".repeat(LONG_FOLDER_LEN)),
        objects: one_sound(&[&long, "Play_X"], &wem),
        wems: vec![(7001, wem)],
        ..Case::default()
    }
    .run(|_, report, _| {
        let [skip] = report.skipped.as_slice() else {
            return Err(format!("one event is skipped: {:?}", report.skipped).into());
        };
        assert_eq!(skip.event, long);
        let names: Vec<_> = report
            .files
            .iter()
            .filter_map(|file| file.path.file_name()?.to_str())
            .collect();
        assert_eq!(names, ["Play_X.txtp"]);
        Ok(())
    })
}
