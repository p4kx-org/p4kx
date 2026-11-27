// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The Comparison tab's state over a pair of synthetic Archives, without drawing.

#[path = "comparison_summary_provenance.rs"]
mod summary_provenance;

use crate::comparing;
use crate::waiting::wait_polling_described;

use std::error::Error;

use comparing::{Pair, TestResult, pair};
use p4kx_comparison::{Archive, ChangeKind, CompareOptions, Filter, Levels, compare};
use p4kx_gui::{
    ChangeColumn, ComparisonLevels, ComparisonSide, ComparisonStatus, ComparisonTab,
    ComparisonView, Language, Model, ReportStatus, SortDirection,
};

/// Returns a window state with both Archives of `pair` picked.
pub(crate) fn picked(pair: &Pair, language: Language) -> Model {
    let mut model = Model::new(language);
    let tab = model.comparison_mut();
    tab.set_archive(ComparisonSide::Old, pair.old.clone());
    tab.set_archive(ComparisonSide::New, pair.new.clone());
    model
}

/// Polls `tab` until `done` holds.
pub(crate) fn wait(tab: &mut ComparisonTab, done: impl Fn(&ComparisonTab) -> bool) -> TestResult {
    wait_polling_described(
        tab,
        "the Comparison to settle",
        |tab| {
            tab.poll();
        },
        done,
        |tab| format!("{:?}", tab.status()),
    )?;
    Ok(())
}

/// Starts the Comparison and waits for its view.
pub(crate) fn compared(model: &mut Model) -> Result<&mut ComparisonView, Box<dyn Error>> {
    let tab = model.comparison_mut();
    if !tab.start(|| {}) {
        return Err("the Comparison did not start".into());
    }
    wait(tab, |tab| {
        !matches!(tab.status(), ComparisonStatus::Running { .. })
    })?;
    if tab.view().is_none() {
        return Err(format!("no result: {:?}", tab.status()).into());
    }
    tab.view_mut().ok_or_else(|| "no result".into())
}

/// Waits until the view applied its Filter.
pub(crate) fn filtered(model: &mut Model) -> TestResult {
    wait(model.comparison_mut(), |tab| {
        tab.view().is_some_and(|view| !view.is_filtering())
    })
}

/// Returns the paths of the shown changes in list order.
pub(crate) fn shown_paths(view: &ComparisonView) -> Vec<String> {
    view.shown()
        .iter()
        .filter_map(|&id| view.change(id))
        .map(|change| change.path().to_owned())
        .collect()
}

#[test]
fn the_comparison_lists_every_change_in_path_order_with_its_counts() -> TestResult {
    let pair = pair()?;
    let mut model = picked(&pair, Language::BritishEnglish);

    let view = compared(&mut model)?;

    assert_eq!(
        shown_paths(view),
        [
            "Data\\Libs\\crc.xml",
            "Data\\Libs\\size.xml",
            "Data\\removed.txt",
            "Engine\\added.cfg"
        ],
        "every change, unchanged Entries left out"
    );
    let summary = view.summary();
    assert_eq!(
        (
            summary.counts.added,
            summary.counts.removed,
            summary.counts.changed
        ),
        (1, 1, 2),
        "counts per change kind"
    );
    let directories: Vec<(&str, usize, usize, usize)> = summary
        .directories
        .iter()
        .map(|d| {
            (
                d.directory.as_str(),
                d.counts.added,
                d.counts.removed,
                d.counts.changed,
            )
        })
        .collect();
    assert_eq!(
        directories,
        [("Data", 0, 1, 2), ("Engine", 1, 0, 0)],
        "counts per top-level directory"
    );
    Ok(())
}

#[test]
fn both_sides_carry_their_channel_and_build() -> TestResult {
    let pair = pair()?;
    let mut model = picked(&pair, Language::BritishEnglish);

    let view = compared(&mut model)?;

    let comparison = view.comparison();
    let (old, new) = (comparison.old_side(), comparison.new_side());
    assert_eq!(old.channel(), Some("LIVE"), "old Channel");
    assert_eq!(new.channel(), Some("PTU"), "new Channel");
    assert_eq!(
        old.build().map(p4kx_comparison::Build::version),
        Some("4.3.1.10203"),
        "old Build"
    );
    assert_eq!(
        new.build().map(p4kx_comparison::Build::version),
        Some("4.3.2.10400"),
        "new Build"
    );
    Ok(())
}

#[test]
fn the_list_filters_with_the_browsing_rules_and_recounts() -> TestResult {
    let pair = pair()?;
    let mut model = picked(&pair, Language::BritishEnglish);
    compared(&mut model)?.set_filter("*.XML", || {});

    filtered(&mut model)?;

    let view = model.comparison().view().ok_or("no view")?;
    assert_eq!(
        shown_paths(view),
        ["Data\\Libs\\crc.xml", "Data\\Libs\\size.xml"],
        "a glob ignoring case"
    );
    assert_eq!(view.summary().counts.total(), 2, "counts follow the Filter");
    assert_eq!(view.summary().directories.len(), 1, "only Data");
    Ok(())
}

#[test]
fn the_list_sorts_by_every_column_both_ways() -> TestResult {
    let pair = pair()?;
    let mut model = picked(&pair, Language::BritishEnglish);
    let view = compared(&mut model)?;

    view.sort_by(ChangeColumn::NewSize);
    let ascending = shown_paths(view);
    view.sort_by(ChangeColumn::NewSize);
    let descending = shown_paths(view);
    view.sort_by(ChangeColumn::Change);
    let by_kind: Vec<ChangeKind> = view
        .shown()
        .iter()
        .filter_map(|&id| view.change(id))
        .map(|change| change.kind())
        .collect();

    assert_eq!(
        ascending,
        [
            "Data\\removed.txt",
            "Data\\Libs\\crc.xml",
            "Engine\\added.cfg",
            "Data\\Libs\\size.xml"
        ],
        "no new size first, then 40, 60, 3000 bytes"
    );
    assert_eq!(
        descending.iter().rev().collect::<Vec<_>>(),
        ascending.iter().collect::<Vec<_>>(),
        "reversed"
    );
    assert_eq!(
        view.sort().direction,
        SortDirection::Ascending,
        "a new column sorts ascending"
    );
    assert_eq!(
        by_kind,
        [
            ChangeKind::Added,
            ChangeKind::Removed,
            ChangeKind::Changed,
            ChangeKind::Changed
        ],
        "grouped by change kind"
    );
    Ok(())
}

#[test]
fn nothing_starts_without_both_archives_or_without_a_level() -> TestResult {
    let pair = pair()?;
    let mut model = Model::new(Language::BritishEnglish);
    let tab = model.comparison_mut();
    tab.set_archive(ComparisonSide::Old, pair.old.clone());
    let one_side = tab.start(|| {});
    tab.set_archive(ComparisonSide::New, pair.new);
    tab.set_levels(ComparisonLevels {
        entries: false,
        records: false,
    });
    let no_level = tab.start(|| {});

    assert!(!one_side, "needs both Archives");
    assert!(!no_level, "needs a level");
    assert!(matches!(tab.status(), ComparisonStatus::Idle), "still idle");
    Ok(())
}

#[test]
fn the_record_level_fails_with_the_reason_when_an_archive_has_no_dataforge() -> TestResult {
    let pair = pair()?;
    let mut model = picked(&pair, Language::BritishEnglish);
    let tab = model.comparison_mut();
    tab.set_levels(ComparisonLevels {
        entries: false,
        records: true,
    });

    assert!(tab.start(|| {}), "the Record level alone starts");
    wait(tab, |tab| !tab.is_running())?;

    match tab.status() {
        ComparisonStatus::Failed(Some(reason)) => {
            assert!(reason.contains("Data.p4k"), "names the Archive: {reason}");
        }
        other => return Err(format!("not failed: {other:?}").into()),
    }
    Ok(())
}
#[test]
fn cancelling_stops_the_comparison_at_once() -> TestResult {
    let pair = pair()?;
    let mut model = picked(&pair, Language::BritishEnglish);
    let tab = model.comparison_mut();
    assert!(tab.start(|| {}), "started");

    tab.cancel();
    wait(tab, |tab| {
        !matches!(tab.status(), ComparisonStatus::Running { .. })
    })?;

    assert!(
        matches!(tab.status(), ComparisonStatus::Cancelled),
        "cancelled"
    );
    assert!(tab.view().is_none(), "no result");
    Ok(())
}

#[test]
fn a_missing_archive_fails_with_its_reason() -> TestResult {
    let pair = pair()?;
    let mut model = picked(&pair, Language::BritishEnglish);
    let tab = model.comparison_mut();
    tab.set_archive(ComparisonSide::New, pair.dir.path().join("missing.p4k"));
    assert!(tab.start(|| {}), "started");

    wait(tab, |tab| {
        !matches!(tab.status(), ComparisonStatus::Running { .. })
    })?;

    match tab.status() {
        ComparisonStatus::Failed(Some(reason)) => assert!(!reason.is_empty(), "a reason"),
        other => return Err(format!("not failed: {other:?}").into()),
    }
    Ok(())
}

#[test]
fn the_saved_report_is_the_report_of_the_cli() -> TestResult {
    let pair = pair()?;
    let mut model = picked(&pair, Language::BritishEnglish);
    compared(&mut model)?.set_filter("Data", || {});
    filtered(&mut model)?;
    let saved = pair.dir.path().join("window.json");
    let expected = pair.dir.path().join("cli.json");

    model
        .comparison_mut()
        .view_mut()
        .ok_or("no view")?
        .save_report(saved.clone(), || {});
    wait(model.comparison_mut(), |tab| {
        tab.view()
            .is_some_and(|view| !matches!(view.report_status(), Some(ReportStatus::Saving(_))))
    })?;
    let (old, new) = (
        Archive::open_with_key(&pair.old, Some(p4kx_testkit::test_key()))?,
        Archive::open_with_key(&pair.new, Some(p4kx_testkit::test_key()))?,
    );
    let filter = Filter::new("Data");
    let options = CompareOptions {
        levels: Levels::ENTRIES,
        filter: Some(&filter),
        ..CompareOptions::default()
    };
    compare(&old, &new, None, &options)?.save_report(&expected)?;

    let status = model
        .comparison()
        .view()
        .and_then(|view| view.report_status());
    assert!(
        matches!(status, Some(ReportStatus::Saved(_))),
        "saved: {status:?}"
    );
    assert_eq!(
        std::fs::read(&saved)?,
        std::fs::read(&expected)?,
        "byte for byte"
    );
    Ok(())
}
