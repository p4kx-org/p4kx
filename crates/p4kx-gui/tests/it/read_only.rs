// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The window never offers a target inside a Star Citizen installation and warns while the game
//! runs.

use crate::waiting::wait_for_model;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use p4kx_gui::{Language, Model, ui};
use p4kx_testkit::{ArchiveBuilder, TestEntry};
use tempfile::TempDir;

type TestResult = Result<(), Box<dyn Error>>;

fn installation(dir: &Path) -> Result<PathBuf, Box<dyn Error>> {
    let channel = dir.join("StarCitizen").join("LIVE");
    fs::create_dir_all(channel.join("Bin64"))?;
    let path = channel.join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data\\good.txt", "good"))
        .write_to(&path)?;
    Ok(path)
}

fn opened(
    language: Language,
    folder: impl FnOnce(&Path, &Path) -> PathBuf,
) -> Result<(TempDir, Harness<'static, Model>), Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let path = installation(dir.path())?;
    let mut model = Model::new(language);
    model.set_extraction_folder(Some(folder(dir.path(), &path)));
    let mut harness = Harness::builder()
        .with_context_setup(p4kx_gui::theme::apply)
        .with_size([1400.0, 900.0])
        .build_ui_state(
            |ui, model: &mut Model| {
                ui::show(ui, model);
            },
            model,
        );
    harness.state_mut().open(path, || {});
    wait_until(&mut harness, |model| model.browser().is_some())?;
    Ok((dir, harness))
}

fn wait_until(harness: &mut Harness<'_, Model>, done: impl Fn(&Model) -> bool) -> TestResult {
    wait_for_model(harness, "the window to settle", done)?;
    harness.run_steps(2);
    Ok(())
}

#[test]
fn a_folder_inside_the_installation_cannot_be_started() -> TestResult {
    let (_dir, mut harness) = opened(Language::BritishEnglish, |_, archive| {
        archive.with_file_name("Bin64").join("..").join("export")
    })?;

    harness.get_by_label("Extract all…").click();
    harness.run_steps(2);

    harness.get_by_label_contains("lies inside the Star Citizen installation");
    assert!(
        harness
            .get_by_label("Extract")
            .accesskit_node()
            .is_disabled(),
        "Start is not offered"
    );
    Ok(())
}

#[test]
fn the_notice_is_in_german_too() -> TestResult {
    let (_dir, mut harness) = opened(Language::German, |_, archive| {
        archive.with_file_name("export")
    })?;

    harness.get_by_label("Alle extrahieren …").click();
    harness.run_steps(2);

    harness.get_by_label_contains("liegt in der Star-Citizen-Installation");
    Ok(())
}

#[test]
fn a_folder_elsewhere_is_fine() -> TestResult {
    let (_dir, mut harness) = opened(Language::BritishEnglish, |dir, _| dir.join("elsewhere"))?;

    harness.get_by_label("Extract all…").click();
    harness.run_steps(2);

    assert!(
        harness
            .query_by_label_contains("lies inside the Star Citizen installation")
            .is_none(),
        "no notice for a folder outside the installation"
    );
    Ok(())
}

#[test]
fn warns_while_the_game_may_run_and_keeps_the_window_usable() -> TestResult {
    for (language, notice, extract) in [
        (
            Language::BritishEnglish,
            "Star Citizen may be running",
            "Extract all…",
        ),
        (
            Language::German,
            "Star Citizen läuft möglicherweise",
            "Alle extrahieren …",
        ),
    ] {
        let (_dir, mut harness) = opened(language, |dir, _| dir.join("elsewhere"))?;
        harness.state_mut().probe_game_with(|| true);
        harness.state_mut().watch_game(true);
        wait_until(&mut harness, Model::game_running)?;

        harness.get_by_label_contains(notice);
        harness.get_by_label(extract);
        assert!(
            harness.state().browser().is_some(),
            "the Archive stays open and readable"
        );
    }
    Ok(())
}

#[test]
fn stays_quiet_while_the_game_is_closed() -> TestResult {
    let (_dir, mut harness) = opened(Language::BritishEnglish, |dir, _| dir.join("elsewhere"))?;

    harness.state_mut().probe_game_with(|| false);
    harness.state_mut().watch_game(true);
    let deadline = Instant::now() + Duration::from_secs(3);
    while Instant::now() < deadline {
        harness.run_steps(2);
        std::thread::sleep(Duration::from_millis(20));
    }

    assert!(
        harness
            .query_by_label_contains("Star Citizen may be running")
            .is_none(),
        "no warning without the game"
    );
    Ok(())
}
