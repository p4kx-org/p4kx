// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Extraction from the window driven headless: scope and target.

use crate::common;
use crate::extraction_outputs::assert_receipted_outputs;

use crate::waiting::wait_polling;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use common::{TestResult, check, extract_into, finished, setup};
use p4kx_gui::{ExtractionScope, Model, TargetKind};

fn only_folder(dir: &Path) -> Result<PathBuf, Box<dyn Error>> {
    let mut folders = fs::read_dir(dir)?.collect::<Result<Vec<_>, _>>()?;
    match (folders.pop(), folders.is_empty()) {
        (Some(folder), true) => Ok(folder.path()),
        _ => Err("expected exactly one folder".into()),
    }
}

#[test]
fn selected_entries_go_into_a_new_sc_export_folder_named_like_the_cli() -> TestResult {
    let (_dir, mut model, out) = setup()?;
    check(&mut model, &["ship.cgf", "readme.txt"])?;

    let started = extract_into(
        &mut model,
        ExtractionScope::Selected,
        TargetKind::NewFolder,
        &out,
    )?;
    let summary = finished(&mut model)?;

    assert!(started, "a partial Extraction starts without confirmation");
    let target = only_folder(&out)?;
    let name = target.file_name().and_then(|n| n.to_str()).unwrap_or("");
    assert!(
        name.starts_with("SC-EXPORT-LIVE-4.3.1.10203-") && name.ends_with('Z'),
        "{name}"
    );
    assert_eq!(summary.target(), target, "the summary names the target");
    assert_eq!(summary.extracted(), 2, "two Entries were checked");
    assert_eq!(
        summary.file_origin_receipts().len(),
        2,
        "the summary lists each committed receipt"
    );
    assert!(
        summary
            .file_origin_receipts()
            .iter()
            .all(|receipt| target.join(receipt).is_file()),
        "every listed receipt exists under the extraction target"
    );
    assert_receipted_outputs(&target, &["Data\\Objects\\ship.cgf", "readme.txt"])?;
    assert_eq!(fs::read(target.join("readme.txt"))?, b"readme", "data");
    Ok(())
}

fn wait_for_search(model: &mut Model) -> TestResult {
    wait_polling(
        model,
        "the search to end",
        |model| {
            if let Some(browser) = model.browser_mut() {
                browser.poll();
            }
        },
        |model| !model.browser().is_some_and(p4kx_gui::Browser::is_searching),
    )
}

#[test]
fn shown_entries_are_those_of_the_search() -> TestResult {
    let (_dir, mut model, out) = setup()?;
    model
        .browser_mut()
        .ok_or("no Archive")?
        .search("*.cgf", || {});
    wait_for_search(&mut model)?;

    extract_into(
        &mut model,
        ExtractionScope::Shown,
        TargetKind::ExistingFolder,
        &out,
    )?;
    let summary = finished(&mut model)?;

    assert_eq!(summary.extracted(), 2, "both matches");
    assert_receipted_outputs(
        &out,
        &["Data\\Objects\\anchor.cgf", "Data\\Objects\\ship.cgf"],
    )
}

#[test]
fn every_scope_with_entries_starts_and_an_empty_one_does_not() -> TestResult {
    let (_dir, mut model, out) = setup()?;

    let selected = extract_into(
        &mut model,
        ExtractionScope::Selected,
        TargetKind::ExistingFolder,
        &out,
    )?;

    assert!(!selected, "nothing is checked");
    assert_eq!(
        model.scope_len(ExtractionScope::Selected),
        0,
        "no checked Entry"
    );
    assert_eq!(
        model.scope_len(ExtractionScope::Shown),
        4,
        "the root shows all"
    );
    assert_eq!(model.scope_len(ExtractionScope::All), 4, "every Entry");
    Ok(())
}

#[test]
fn a_new_folder_ignores_the_conflict_policy_and_never_reuses_a_folder() -> TestResult {
    let (_dir, mut model, out) = setup()?;
    check(&mut model, &["readme.txt"])?;

    for _ in 0..2 {
        extract_into(
            &mut model,
            ExtractionScope::Selected,
            TargetKind::NewFolder,
            &out,
        )?;
        finished(&mut model)?;
    }

    assert_eq!(fs::read_dir(&out)?.count(), 2, "a second folder");
    Ok(())
}

#[test]
fn the_setup_starts_with_the_folder_chosen_last() -> TestResult {
    let (_dir, mut model, out) = setup()?;
    check(&mut model, &["readme.txt"])?;
    extract_into(
        &mut model,
        ExtractionScope::Selected,
        TargetKind::ExistingFolder,
        &out,
    )?;
    finished(&mut model)?;

    model.begin_extraction(ExtractionScope::Shown);

    assert_eq!(model.extraction_folder(), Some(out.as_path()), "remembered");
    assert_eq!(
        model
            .extraction_setup()
            .and_then(p4kx_gui::ExtractionSetup::folder),
        Some(out.as_path()),
        "preset"
    );
    assert!(model.extraction().is_none(), "the summary is dismissed");
    Ok(())
}

#[test]
fn without_a_folder_nothing_starts() -> TestResult {
    let (_dir, mut model, _out) = setup()?;

    model.begin_extraction(ExtractionScope::Shown);
    let started = model.submit_extraction(|| {});

    assert!(!started, "no folder chosen");
    assert!(model.extraction_setup().is_some(), "the setup stays open");
    Ok(())
}

#[test]
fn shown_entries_cannot_be_extracted_while_a_search_runs() -> TestResult {
    let (_dir, mut model, out) = setup()?;
    let browser = model.browser_mut().ok_or("no Archive")?;
    browser.search("*.cgf", || {});
    let searching = browser.is_searching();

    let started = extract_into(
        &mut model,
        ExtractionScope::Shown,
        TargetKind::ExistingFolder,
        &out,
    )?;

    assert!(searching, "the search waits for the next poll");
    assert!(!started, "the shown Entries are about to change");
    assert_eq!(model.scope_len(ExtractionScope::Shown), 0, "none offered");
    Ok(())
}
