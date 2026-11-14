// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The Reload and Extract selected entries of the Archive menu and their shortcuts, driven headless.

use crate::waiting::{RunQuiet, click_shown_once, wait_for_model};
use std::error::Error;
use std::path::PathBuf;

use egui::os::OperatingSystem;
use egui::{Key, Modifiers};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use p4kx_gui::{Language, Model, ui};
use p4kx_testkit::{ArchiveBuilder, TestEntry};
use tempfile::TempDir;

type TestResult = Result<(), Box<dyn Error>>;

fn opened() -> Result<(TempDir, Harness<'static, Model>), Box<dyn Error>> {
    opened_on(OperatingSystem::from_target_os())
}

fn opened_on(os: OperatingSystem) -> Result<(TempDir, Harness<'static, Model>), Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let path: PathBuf = dir.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data\\config.xml", "config"))
        .entry(TestEntry::stored("readme.txt", "readme"))
        .write_to(&path)?;
    let mut harness = Harness::builder()
        .with_os(os)
        .with_context_setup(p4kx_gui::theme::apply)
        .with_size([1400.0, 900.0])
        .build_ui_state(
            |ui, model: &mut Model| {
                ui::show(ui, model);
            },
            Model::new(Language::BritishEnglish),
        );
    harness.state_mut().open(path, || {});
    wait_until(&mut harness, |model| model.browser().is_some())?;
    Ok((dir, harness))
}

fn wait_until(harness: &mut Harness<'_, Model>, done: impl Fn(&Model) -> bool) -> TestResult {
    wait_for_model(harness, "the window to settle", done)?;
    harness.run_quiet();
    Ok(())
}

fn check_first(harness: &mut Harness<'_, Model>) -> TestResult {
    let browser = harness.state_mut().browser_mut().ok_or("no Archive")?;
    let id = *browser.shown().first().ok_or("no Entry")?;
    browser.set_checked(id, true);
    harness.run_quiet();
    Ok(())
}

fn checked(harness: &Harness<'_, Model>) -> usize {
    harness
        .state()
        .browser()
        .map_or(0, p4kx_gui::Browser::checked_count)
}

fn reopened(model: &Model) -> bool {
    model.opening_path().is_none() && model.browser().is_some_and(|b| b.checked_count() == 0)
}

#[test]
fn ctrl_e_opens_the_extraction_of_the_selected_entries() -> TestResult {
    let (_dir, mut harness) = opened()?;

    harness.key_press_modifiers(Modifiers::COMMAND, Key::E);
    harness.run_quiet();
    let without_selection = harness.state().extraction_setup().is_some();
    check_first(&mut harness)?;
    harness.key_press_modifiers(Modifiers::COMMAND, Key::E);
    harness.run_quiet();

    assert!(!without_selection, "nothing selected, nothing opens");
    assert!(
        harness.state().extraction_setup().is_some(),
        "the setup opens"
    );
    Ok(())
}

#[test]
fn the_menu_entry_extracts_with_the_platform_shortcut_label() -> TestResult {
    for (os, hint) in [
        (OperatingSystem::Nix, "Ctrl+E"),
        (OperatingSystem::Mac, "Cmd+E"),
    ] {
        let (_dir, mut harness) = opened_on(os)?;
        check_first(&mut harness)?;
        harness.get_by_label("Archive").click();
        harness.run_quiet();
        click_shown_once(&mut harness, hint)?;
        harness.run_quiet();
        assert!(
            harness.state().extraction_setup().is_some(),
            "the setup opens"
        );
    }
    Ok(())
}

#[test]
fn macos_command_e_opens_extraction_for_checked_entries() -> TestResult {
    let (_dir, mut harness) = opened_on(OperatingSystem::Mac)?;
    check_first(&mut harness)?;
    let command = Modifiers {
        mac_cmd: true,
        command: true,
        ..Modifiers::NONE
    };
    harness.key_press_modifiers(command, Key::E);
    harness.run_quiet();
    assert!(
        harness.state().extraction_setup().is_some(),
        "Command+E opens the setup"
    );
    Ok(())
}

#[test]
fn f5_opens_the_archive_again() -> TestResult {
    let (_dir, mut harness) = opened()?;
    check_first(&mut harness)?;

    harness.key_press(Key::F5);
    wait_until(&mut harness, reopened)?;

    assert_eq!(checked(&harness), 0, "a fresh index");
    Ok(())
}

#[test]
fn the_menu_entry_opens_the_archive_again() -> TestResult {
    let (_dir, mut harness) = opened()?;
    check_first(&mut harness)?;

    harness.get_by_label("Archive").click();
    harness.run_quiet();
    click_shown_once(&mut harness, "F5")?;
    wait_until(&mut harness, reopened)?;

    assert_eq!(checked(&harness), 0, "a fresh index");
    Ok(())
}

#[test]
fn f5_does_nothing_while_the_extraction_setup_is_open() -> TestResult {
    let (_dir, mut harness) = opened()?;
    check_first(&mut harness)?;
    harness.key_press_modifiers(Modifiers::COMMAND, Key::E);
    harness.run_quiet();

    harness.key_press(Key::F5);
    harness.run_quiet();

    assert!(
        harness.state().opening_path().is_none(),
        "the Archive is not opened again"
    );
    assert_eq!(checked(&harness), 1, "the selection stays");
    Ok(())
}
