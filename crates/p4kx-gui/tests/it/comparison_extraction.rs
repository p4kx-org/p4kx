// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Extracting the added and changed Entries of one side of a Comparison, without drawing.

use crate::comparing;
use crate::extraction_outputs::assert_receipted_outputs;
use crate::waiting::wait_polling_described;

use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use comparing::{Pair, TestResult, pair};
use p4kx_extraction::ConflictPolicy;
use p4kx_gui::{
    ComparisonSide, ComparisonStatus, Extraction, ExtractionScope, ExtractionStatus,
    ExtractionSummary, Language, Model, TargetKind,
};
use p4kx_testkit::{ArchiveBuilder, TestEntry};

pub(crate) fn poll_until(model: &mut Model, done: impl Fn(&Model) -> bool) -> TestResult {
    wait_polling_described(
        model,
        "the work to finish",
        |model| {
            model.comparison_mut().poll();
            model.poll_extraction();
        },
        done,
        |model| format!("{:?}", model.comparison().status()),
    )?;
    Ok(())
}

fn settled(model: &Model) -> bool {
    matches!(model.comparison().status(), ComparisonStatus::Finished(view) if !view.is_filtering())
}

pub(crate) fn compared(pair: &Pair, filter: &str) -> Result<Model, Box<dyn Error>> {
    let mut model = Model::new(Language::BritishEnglish);
    let tab = model.comparison_mut();
    tab.set_archive(ComparisonSide::Old, pair.old.clone());
    tab.set_archive(ComparisonSide::New, pair.new.clone());
    if !tab.start(|| {}) {
        return Err("the Comparison did not start".into());
    }
    poll_until(&mut model, settled)?;
    if let Some(view) = model.comparison_mut().view_mut() {
        view.set_filter(filter, || {});
    }
    poll_until(&mut model, settled)?;
    Ok(model)
}

fn extract(
    model: &mut Model,
    side: ComparisonSide,
    out: &Path,
    (policy, convert): (ConflictPolicy, bool),
) -> Result<ExtractionSummary, Box<dyn Error>> {
    model.begin_extraction(ExtractionScope::Changes(side));
    let setup = model.extraction_setup_mut().ok_or("no setup")?;
    setup.set_kind(TargetKind::ExistingFolder);
    setup.set_folder(out.to_path_buf());
    setup.set_policy(policy);
    setup.set_convert(convert);
    if !model.submit_extraction(|| {}) {
        return Err("the Extraction did not start".into());
    }
    poll_until(model, |model| {
        !model.extraction().is_some_and(Extraction::is_running)
    })?;
    match model.extraction().map(Extraction::status) {
        Some(ExtractionStatus::Finished(summary)) => Ok(summary.clone()),
        other => Err(format!("no summary: {other:?}").into()),
    }
}

/// Returns the file an Entry path with backslashes is written to below `out`.
fn at(out: &Path, path: &str) -> PathBuf {
    path.split('\\')
        .fold(out.to_path_buf(), |dir, part| dir.join(part))
}

fn assert_outputs_with_lengths(out: &Path, files: &[(&str, u64)]) -> TestResult {
    let names: Vec<_> = files.iter().map(|&(name, _)| name).collect();
    assert_receipted_outputs(out, &names)?;
    for &(name, length) in files {
        assert_eq!(
            fs::metadata(at(out, name))?.len(),
            length,
            "the selected Archive supplies the expected length for {name}"
        );
    }
    Ok(())
}

#[test]
fn the_new_side_yields_its_added_and_changed_entries() -> TestResult {
    let pair = pair()?;
    let mut model = compared(&pair, "")?;
    let out = pair.dir.path().join("out-new");

    assert_eq!(
        model.scope_len(ExtractionScope::Changes(ComparisonSide::New)),
        3,
        "one added and two changed"
    );
    let summary = extract(
        &mut model,
        ComparisonSide::New,
        &out,
        (ConflictPolicy::Skip, false),
    )?;

    assert_eq!(summary.extracted(), 3, "every selected Entry written");
    assert_outputs_with_lengths(
        &out,
        &[
            ("Data\\Libs\\crc.xml", 40),
            ("Data\\Libs\\size.xml", 3000),
            ("Engine\\added.cfg", 60),
        ],
    )?;
    assert_eq!(
        fs::read(at(&out, "Data\\Libs\\crc.xml"))?,
        vec![9; 40],
        "new bytes"
    );
    Ok(())
}

#[test]
fn the_old_side_yields_its_changed_entries_but_never_the_removed_ones() -> TestResult {
    let pair = pair()?;
    let mut model = compared(&pair, "")?;
    let out = pair.dir.path().join("out-old");

    assert_eq!(
        model.scope_len(ExtractionScope::Changes(ComparisonSide::Old)),
        2,
        "the two changed Entries only"
    );
    extract(
        &mut model,
        ComparisonSide::Old,
        &out,
        (ConflictPolicy::Skip, false),
    )?;

    assert_outputs_with_lengths(
        &out,
        &[("Data\\Libs\\crc.xml", 40), ("Data\\Libs\\size.xml", 30)],
    )?;
    assert_eq!(
        fs::read(at(&out, "Data\\Libs\\crc.xml"))?,
        vec![4; 40],
        "old bytes"
    );
    Ok(())
}

#[test]
fn the_current_filter_narrows_the_selection() -> TestResult {
    let pair = pair()?;
    let mut model = compared(&pair, "*.xml")?;
    let out = pair.dir.path().join("out-filtered");

    assert_eq!(
        model.scope_len(ExtractionScope::Changes(ComparisonSide::New)),
        2,
        "the added .cfg is filtered out"
    );
    extract(
        &mut model,
        ComparisonSide::New,
        &out,
        (ConflictPolicy::Skip, false),
    )?;

    assert_receipted_outputs(&out, &["Data\\Libs\\crc.xml", "Data\\Libs\\size.xml"])
}

#[test]
fn a_filter_matching_only_removed_entries_offers_nothing() -> TestResult {
    let pair = pair()?;
    let mut model = compared(&pair, "removed")?;

    model.begin_extraction(ExtractionScope::Changes(ComparisonSide::Old));
    if let Some(setup) = model.extraction_setup_mut() {
        setup.set_folder(pair.dir.path().join("out-none"));
    }

    assert_eq!(
        model.scope_len(ExtractionScope::Changes(ComparisonSide::Old)),
        0,
        "removed Entries are never selected"
    );
    assert!(!model.submit_extraction(|| {}), "nothing to extract");
    assert!(model.extraction().is_none(), "nothing runs");
    Ok(())
}

#[test]
fn the_conflict_policy_of_the_setup_applies() -> TestResult {
    let pair = pair()?;
    let mut model = compared(&pair, "")?;
    let out = pair.dir.path().join("out-policy");
    fs::create_dir_all(out.join("Engine"))?;
    fs::write(at(&out, "Engine\\added.cfg"), b"mine")?;

    let summary = extract(
        &mut model,
        ComparisonSide::New,
        &out,
        (ConflictPolicy::Skip, false),
    )?;

    assert_eq!(summary.skipped(), ["Engine\\added.cfg"], "kept");
    assert_eq!(
        fs::read(at(&out, "Engine\\added.cfg"))?,
        b"mine",
        "untouched"
    );
    Ok(())
}

#[test]
fn the_conversion_switch_of_the_setup_applies_and_is_remembered() -> TestResult {
    let pair = pair()?;
    let mut model = compared(&pair, "")?;
    let out = pair.dir.path().join("out-converted");

    let summary = extract(
        &mut model,
        ComparisonSide::New,
        &out,
        (ConflictPolicy::Skip, true),
    )?;

    assert_eq!(summary.converted(), Some(0), "Conversion was on");
    assert!(
        model.extraction_conversion(),
        "the next setup starts with it"
    );
    Ok(())
}

#[test]
fn a_cancelled_extraction_accounts_for_every_selected_entry() -> TestResult {
    let pair = pair()?;
    let mut model = compared(&pair, "")?;
    let out = pair.dir.path().join("out-cancelled");

    model.begin_extraction(ExtractionScope::Changes(ComparisonSide::New));
    let setup = model.extraction_setup_mut().ok_or("no setup")?;
    setup.set_kind(TargetKind::ExistingFolder);
    setup.set_folder(out);
    if !model.submit_extraction(|| {}) {
        return Err("the Extraction did not start".into());
    }
    model.cancel_extraction();
    poll_until(&mut model, |model| {
        !model.extraction().is_some_and(Extraction::is_running)
    })?;

    let Some(ExtractionStatus::Finished(summary)) = model.extraction().map(Extraction::status)
    else {
        return Err("no summary".into());
    };
    assert_eq!(
        summary.extracted() + summary.skipped().len() + summary.cancelled().len(),
        3,
        "written or reported as cancelled"
    );
    Ok(())
}

#[test]
fn a_side_that_changed_on_disk_stops_the_extraction() -> TestResult {
    let pair = pair()?;
    let mut model = compared(&pair, "")?;
    let out = pair.dir.path().join("out-changed");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data\\patched.txt", vec![7; 70]))
        .write_to(&pair.new)?;

    let summary = extract(
        &mut model,
        ComparisonSide::New,
        &out,
        (ConflictPolicy::Skip, false),
    )?;

    assert!(summary.archive_changed(), "the summary says why");
    assert!(
        model
            .extraction()
            .is_some_and(Extraction::is_comparison_side),
        "the summary advises comparing again"
    );
    assert_eq!(summary.extracted(), 0, "nothing stale written");
    Ok(())
}

#[test]
fn a_new_folder_is_named_after_the_chosen_side() -> TestResult {
    let pair = pair()?;
    let mut model = compared(&pair, "")?;
    let parent = pair.dir.path().join("out-parent");
    fs::create_dir_all(&parent)?;

    model.begin_extraction(ExtractionScope::Changes(ComparisonSide::Old));
    let setup = model.extraction_setup_mut().ok_or("no setup")?;
    setup.set_folder(parent);
    if !model.submit_extraction(|| {}) {
        return Err("the Extraction did not start".into());
    }
    poll_until(&mut model, |model| {
        !model.extraction().is_some_and(Extraction::is_running)
    })?;

    let Some(ExtractionStatus::Finished(summary)) = model.extraction().map(Extraction::status)
    else {
        return Err("no summary".into());
    };
    let name = summary
        .target()
        .file_name()
        .map_or_default(|name| name.to_string_lossy().into_owned());
    assert!(
        name.starts_with("SC-EXPORT-LIVE-4.3.1.10203-"),
        "named after the old side: {name}"
    );
    Ok(())
}

#[test]
fn nothing_is_offered_before_a_comparison_finished() {
    let mut model = Model::new(Language::BritishEnglish);

    model.begin_extraction(ExtractionScope::Changes(ComparisonSide::New));

    assert_eq!(
        model.scope_len(ExtractionScope::Changes(ComparisonSide::New)),
        0,
        "no Comparison"
    );
    assert!(model.extraction_setup().is_none(), "no setup opens");
}
