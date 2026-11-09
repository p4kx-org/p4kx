// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The Channel picker driven headless: detected Channels, manual entries, recent Archives.

use crate::waiting::{RunQuiet, wait_for};
use std::error::Error;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use p4kx_channels::SearchPaths;
use p4kx_gui::ui::{self, Dialog, Requests};
use p4kx_gui::{ChannelList, Language, Model};
use p4kx_settings::{ManualChannels, RecentArchives};
use p4kx_testkit::{ArchiveBuilder, TestEntry, write_build_manifest};
use tempfile::TempDir;

type TestResult = Result<(), Box<dyn Error>>;
type State = (Model, Requests);

#[derive(Debug)]
struct Dropped(PathBuf);

impl egui::DroppedFile for Dropped {
    fn path(&self) -> &Path {
        &self.0
    }

    fn bytes(&self) -> Result<Vec<u8>, String> {
        Err("a folder has no bytes".to_owned())
    }
}

fn channel(library: &Path, name: &str, version: Option<&str>) -> Result<PathBuf, Box<dyn Error>> {
    let folder = library.join(name);
    std::fs::create_dir_all(&folder)?;
    let archive = folder.join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data\\readme.txt", vec![1; 4]))
        .write_to(&archive)?;
    if let Some(version) = version {
        write_build_manifest(
            &folder,
            &format!("sc-alpha-{name}"),
            version,
            "9876543",
            "123",
        )?;
    }
    Ok(archive)
}

fn library() -> Result<TempDir, Box<dyn Error>> {
    let library = tempfile::tempdir()?;
    channel(library.path(), "LIVE", Some("4.3.1-live.9876543"))?;
    channel(library.path(), "PTU", None)?;
    Ok(library)
}

fn harness(library: &Path) -> Harness<'static, State> {
    let mut model = Model::new(Language::BritishEnglish);
    model.set_channels(ChannelList::new(
        SearchPaths {
            default_library: Some(library.to_path_buf()),
            ..SearchPaths::default()
        },
        ManualChannels::default(),
    ));
    Harness::builder()
        .with_context_setup(p4kx_gui::theme::apply)
        .with_size([1200.0, 800.0])
        .build_ui_state(
            |ui, (model, requests): &mut State| {
                let frame = ui::show(ui, model);
                requests.dialog = requests.dialog.or(frame.dialog);
                requests.settings_changed |= frame.settings_changed;
            },
            (model, Requests::default()),
        )
}

fn wait_until_open(harness: &mut Harness<'_, State>) -> TestResult {
    wait_for(harness, "the Archive to open", |harness| {
        harness.state().0.browser().is_some()
    })?;
    harness.run_quiet();
    Ok(())
}

fn open_archive(harness: &Harness<'_, State>) -> Option<PathBuf> {
    let browser = harness.state().0.browser()?;
    Some(browser.index().path().to_path_buf())
}

#[test]
fn the_detected_channels_are_listed_with_their_build() -> TestResult {
    let library = library()?;
    let mut harness = harness(library.path());

    harness.run_quiet();

    assert!(harness.query_by_label("LIVE").is_some(), "LIVE is listed");
    assert!(harness.query_by_label("PTU").is_some(), "PTU is listed");
    assert!(
        harness.query_by_label("Build 4.3.1-live.9876543").is_some(),
        "the Build of LIVE"
    );
    assert!(
        harness.query_by_label("Build unknown").is_some(),
        "PTU has no build manifest"
    );
    Ok(())
}

#[test]
fn selecting_a_channel_opens_its_archive_and_remembers_it() -> TestResult {
    let library = library()?;
    let live = library.path().join("LIVE").join("Data.p4k");
    let mut harness = harness(library.path());
    harness.run_quiet();

    harness.get_by_label("LIVE").click();
    harness.step();
    wait_until_open(&mut harness)?;

    let (model, requests) = harness.state();
    assert_eq!(open_archive(&harness), Some(live.clone()), "LIVE is open");
    assert_eq!(
        model.recent().iter().next(),
        Some(&std::path::absolute(&live)?),
        "LIVE is the latest recent Archive"
    );
    assert!(requests.settings_changed, "the recent Archives are stored");
    Ok(())
}

#[test]
fn dropping_a_folder_adds_it_as_a_manual_entry() -> TestResult {
    let library = library()?;
    let extra = tempfile::tempdir()?;
    channel(extra.path(), "HOTFIX", Some("4.3.2-hotfix.1"))?;
    let mut harness = harness(library.path());
    harness.run_quiet();

    harness
        .input_mut()
        .dropped_files
        .push(Arc::new(Dropped(extra.path().to_path_buf())));
    wait_for(&mut harness, "the dropped manual Channel", |harness| {
        harness.query_by_label("HOTFIX").is_some()
    })?;
    harness.run_quiet();

    let (model, requests) = harness.state();
    assert_eq!(
        model.channels().manual().folders,
        [std::path::absolute(extra.path())?],
        "the folder is a manual entry"
    );
    assert!(requests.settings_changed, "the manual entry is stored");
    assert!(
        harness.query_by_label("HOTFIX").is_some(),
        "HOTFIX is listed"
    );
    assert!(model.browser().is_none(), "nothing is opened");
    Ok(())
}

#[test]
fn an_added_archive_is_listed_as_a_channel() -> TestResult {
    let library = library()?;
    let extra = tempfile::tempdir()?;
    let archive = channel(extra.path(), "EPTU", None)?;
    let mut harness = harness(library.path());

    let added = harness.state_mut().0.add_manual(archive.clone());
    let again = harness.state_mut().0.add_manual(archive.clone());
    wait_for(&mut harness, "the added manual Archive", |harness| {
        harness.query_by_label("EPTU").is_some()
    })?;
    harness.run_quiet();

    assert!(added, "added");
    assert!(!again, "already added");
    assert_eq!(
        harness.state().0.channels().manual().archives,
        [std::path::absolute(&archive)?],
        "one manual Archive"
    );
    assert!(harness.query_by_label("EPTU").is_some(), "EPTU is listed");
    Ok(())
}

#[test]
fn the_archive_menu_asks_for_the_add_folder_dialog() -> TestResult {
    let library = library()?;
    let mut harness = harness(library.path());
    harness.run_quiet();

    harness.get_by_label("Archive").click();
    harness.run_quiet();
    harness.get_by_label("Add folder…").click();
    harness.run_quiet();

    assert_eq!(harness.state().1.dialog, Some(Dialog::AddFolder), "asked");
    Ok(())
}

#[test]
fn a_recent_archive_opens_from_the_archive_menu() -> TestResult {
    let library = library()?;
    let ptu = std::path::absolute(library.path().join("PTU").join("Data.p4k"))?;
    let mut harness = harness(library.path());
    let mut recent = RecentArchives::default();
    recent.push(ptu.clone());
    harness.state_mut().0.set_recent(recent);
    harness.run_quiet();

    harness.get_by_label("Archive").click();
    harness.run_quiet();
    harness.get_by_label("Recent Archives").click();
    harness.run_quiet();
    harness.get_by_label(&ptu.display().to_string()).click();
    harness.step();
    wait_until_open(&mut harness)?;

    assert_eq!(open_archive(&harness), Some(ptu), "PTU is open");
    Ok(())
}
