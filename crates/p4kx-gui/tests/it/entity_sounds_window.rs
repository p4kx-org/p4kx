// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The Sounds tab in the window, driven headless over a synthetic Archive with an Entity, ATL
//! definitions and a soundbank.

use crate::entity_sounds::{TestResult, opened, wait_until};
use crate::sounding::FakeOutput;

use egui::accesskit::Role;
use egui_kittest::kittest::Queryable;
use p4kx_assembly::audio::ExportReport;
use p4kx_gui::{
    Language, ListStatus, Playable, SoundFormat, SoundsExportStatus, SoundsListStatus,
    SoundsStatus, SourceKind, WorkTab,
};

const SIZE: [f32; 2] = [1400.0, 800.0];

fn rows(harness: &egui_kittest::Harness<'_, p4kx_gui::Model>) -> Vec<String> {
    match harness.state().sounds().list_status() {
        ListStatus::Ready(list) => list.rows.iter().map(|row| row.name.clone()).collect(),
        _ => Vec::new(),
    }
}

fn listed(harness: &mut egui_kittest::Harness<'_, p4kx_gui::Model>) -> TestResult {
    wait_until(harness, |model| {
        matches!(model.sounds().list_status(), ListStatus::Ready(_))
    })
}

#[test]
fn the_sounds_tab_asks_for_an_archive() {
    let mut harness = egui_kittest::Harness::builder()
        .with_context_setup(p4kx_gui::theme::apply)
        .with_size(SIZE)
        .build_ui_state(
            |ui, model: &mut p4kx_gui::Model| {
                p4kx_gui::ui::show(ui, model);
            },
            p4kx_gui::Model::new(Language::BritishEnglish),
        );
    harness.run_steps(2);

    harness.get_by_role_and_label(Role::Tab, "Sounds").click();
    harness.run_steps(2);

    assert_eq!(harness.state().work_tab(), WorkTab::Sounds, "switched");
    assert!(
        harness.query_by_label("No Archive open").is_some(),
        "asks for an Archive"
    );
}

#[test]
fn the_audio_is_indexed_off_the_ui_thread_and_an_entity_lists_its_triggers() -> TestResult {
    let (_dir, mut harness) = opened(Language::BritishEnglish, SIZE, &FakeOutput::default())?;
    let SoundsStatus::Ready(info) = harness.state().sounds().status() else {
        return Err("not indexed".into());
    };
    assert_eq!((info.triggers, info.soundbanks), (3, 3), "the index");
    assert_eq!(info.languages, ["English(US)", "German"], "the languages");
    assert_eq!(
        info.default_language.as_deref(),
        Some("English(US)"),
        "the default language"
    );

    harness
        .state_mut()
        .sounds_mut()
        .set_entity_text("test_ship");
    assert!(harness.state_mut().sounds_mut().show_entity_text(), "asked");
    listed(&mut harness)?;

    assert_eq!(
        rows(&harness),
        ["Play_Single", "Play_Multi", "Play_Undefined"],
        "triggers in the order the Entity names them"
    );
    assert_eq!(
        harness.query_all_by_label("Resolved").count(),
        2,
        "status of the resolved triggers"
    );
    assert!(
        harness.query_by_label("No definition").is_some(),
        "status of an undefined trigger"
    );
    assert!(
        harness
            .query_by_label("TEST_Ship: 3 audio triggers, 2 with a soundbank")
            .is_some(),
        "count"
    );
    Ok(())
}

#[test]
fn a_trigger_lists_its_variants_with_their_source_details_and_plays_one() -> TestResult {
    let output = FakeOutput::default();
    let (_dir, mut harness) = opened(Language::BritishEnglish, SIZE, &output)?;
    harness.state_mut().sounds_mut().set_query("play_multi");
    listed(&mut harness)?;
    harness.state_mut().sounds_mut().select(0);
    wait_until(&mut harness, |model| {
        matches!(model.sounds().sounds_status(), SoundsListStatus::Ready(_))
    })?;

    let SoundsListStatus::Ready(list) = harness.state().sounds().sounds_status() else {
        return Err("not resolved".into());
    };
    let facts: Vec<_> = list
        .variants
        .iter()
        .map(|variant| (variant.wem_id, variant.kind, variant.playable()))
        .collect();
    assert_eq!(
        facts,
        [
            (Some(100), SourceKind::Embedded, Playable::Yes),
            (Some(103), SourceKind::Streamed, Playable::Yes),
            (Some(104), SourceKind::Streamed, Playable::Missing),
        ],
        "WEM, source kind and whether it plays"
    );
    assert_eq!(
        harness.query_all_by_label("Test.bnk").count(),
        3,
        "the soundbank is shown for every variant"
    );

    harness.state_mut().select_sound_variant(1, || {});
    wait_until(&mut harness, |model| {
        model
            .sounds()
            .audio()
            .is_some_and(|audio| audio.player().is_some())
    })?;
    harness.get_by_role_and_label(Role::Button, "Play").click();
    harness.run_steps(3);

    assert_eq!(output.started(), 1, "the loose WEM plays");
    harness.state_mut().select_sound_variant(0, || {});
    wait_until(&mut harness, |model| {
        model
            .sounds()
            .audio()
            .is_some_and(|audio| audio.player().is_some())
    })?;
    harness.state_mut().toggle_playback();
    assert_eq!(output.started(), 2, "the embedded WEM plays");
    Ok(())
}

#[test]
fn the_search_finds_triggers_and_events_by_name() -> TestResult {
    let (_dir, mut harness) = opened(Language::BritishEnglish, SIZE, &FakeOutput::default())?;

    harness.state_mut().sounds_mut().set_query("PLAY_S*");
    listed(&mut harness)?;
    assert_eq!(rows(&harness), ["Play_Single"], "a glob, ignoring case");

    harness.state_mut().sounds_mut().set_query("zzz");
    wait_until(&mut harness, |model| {
        matches!(
            model.sounds().list_status(),
            ListStatus::Ready(list) if list.rows.is_empty()
        )
    })?;
    assert!(
        harness
            .query_by_label("No trigger or event name matches the search.")
            .is_some(),
        "nothing matches"
    );
    Ok(())
}

#[test]
fn a_record_that_is_no_entity_is_reported() -> TestResult {
    let (_dir, mut harness) = opened(Language::BritishEnglish, SIZE, &FakeOutput::default())?;
    harness
        .state_mut()
        .sounds_mut()
        .set_entity_text("no_such_ship");
    harness.state_mut().sounds_mut().show_entity_text();

    wait_until(&mut harness, |model| {
        matches!(model.sounds().list_status(), ListStatus::Failed(_))
    })?;
    Ok(())
}

fn assert_sound_receipts(report: &ExportReport) -> TestResult {
    for sound in &report.sounds {
        let receipt = sound.origin_receipt.as_deref().ok_or("sound receipt")?;
        let expected = sound.path.with_extension("wav.p4kx-origin.jsonl");
        assert_eq!(
            std::fs::canonicalize(receipt)?,
            std::fs::canonicalize(expected)?,
            "the GUI result names the sibling receipt"
        );
        assert!(receipt.is_file(), "the published receipt exists");
    }
    Ok(())
}

#[test]
fn the_sounds_of_a_trigger_are_exported_with_progress_and_the_trigger_name() -> TestResult {
    let (dir, mut harness) = opened(Language::BritishEnglish, SIZE, &FakeOutput::default())?;
    harness.state_mut().sounds_mut().set_query("play_");
    listed(&mut harness)?;
    harness.state_mut().sounds_mut().select(0);
    wait_until(&mut harness, |model| {
        matches!(model.sounds().sounds_status(), SoundsListStatus::Ready(_))
    })?;

    assert!(harness.state_mut().begin_sounds_export(), "setup opens");
    let out = dir.path().join("out");
    let setup = harness
        .state_mut()
        .sounds_mut()
        .export_setup_mut()
        .ok_or("no setup")?;
    setup.set_folder(out.clone());
    setup.set_format(SoundFormat::Wav);
    assert!(harness.state_mut().submit_sounds_export(), "starts");

    wait_until(&mut harness, |model| {
        model
            .sounds()
            .export()
            .is_some_and(|export| !export.is_running())
    })?;
    let export = harness.state().sounds().export().ok_or("no export")?;
    let SoundsExportStatus::Finished {
        report,
        cancelled,
        undefined: _,
    } = export.status()
    else {
        return Err("not finished".into());
    };
    assert!(!cancelled, "not stopped");
    let mut names: Vec<_> = report
        .sounds
        .iter()
        .filter_map(|sound| sound.path.file_name()?.to_str().map(str::to_owned))
        .collect();
    names.sort();
    assert_eq!(
        names,
        ["Play_Multi_100.wav", "Play_Multi_103.wav"],
        "named after the trigger and the WEM"
    );
    assert!(out.join("Play_Multi_100.wav").is_file(), "written");
    assert_sound_receipts(report)?;
    assert_eq!(report.failures.len(), 1, "WEM 104 is missing");
    harness.run_steps(3);
    assert!(
        harness.query_by_label_contains("Sounds written").is_some(),
        "the summary is shown"
    );
    Ok(())
}

#[test]
fn the_tab_is_german_in_the_german_window() -> TestResult {
    let (_dir, harness) = opened(Language::German, SIZE, &FakeOutput::default())?;

    assert!(
        harness.query_by_label("Sounds anzeigen").is_some(),
        "German action"
    );
    Ok(())
}

#[test]
fn stopping_an_export_waits_for_it_and_clearing_a_search_keeps_the_entity_list() -> TestResult {
    let (dir, mut harness) = opened(Language::BritishEnglish, SIZE, &FakeOutput::default())?;
    harness
        .state_mut()
        .sounds_mut()
        .set_entity_text("TEST_Ship");
    harness.state_mut().sounds_mut().show_entity_text();
    harness.state_mut().sounds_mut().set_query("");
    listed(&mut harness)?;
    assert_eq!(
        rows(&harness).len(),
        3,
        "the Entity list survives clearing the search"
    );

    assert!(harness.state_mut().begin_sounds_export(), "setup opens");
    let setup = harness
        .state_mut()
        .sounds_mut()
        .export_setup_mut()
        .ok_or("no setup")?;
    setup.set_folder(dir.path().join("out"));
    assert!(harness.state_mut().submit_sounds_export(), "starts");
    harness.state_mut().sounds_mut().stop_export();

    let export = harness.state().sounds().export().ok_or("no export")?;
    assert!(!export.is_running(), "ended before stop_export returned");
    Ok(())
}
