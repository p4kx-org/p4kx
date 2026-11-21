// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The line diff of the selected change, made off the UI thread, without drawing.

use crate::diffing;
use crate::waiting::wait_polling;

use std::error::Error;

use diffing::{ADDED, BINARY, BROKEN, CRYXML, EQUAL, FAR, Pair, TEXT, TOO_LARGE, TestResult, pair};
use p4kx_comparison::LineKind;
use p4kx_gui::{
    ComparisonSide, ComparisonStatus, ComparisonTab, ComparisonView, DiffCell, DiffDocument,
    DiffRow, DiffStatus, Language, Model,
};
use p4kx_testkit::{ArchiveBuilder, TestEntry};

fn wait(tab: &mut ComparisonTab, done: impl Fn(&ComparisonTab) -> bool) -> TestResult {
    wait_polling(
        tab,
        "the Comparison tab to settle",
        |tab| {
            tab.poll();
        },
        done,
    )?;
    Ok(())
}

fn compared(pair: &Pair) -> Result<Model, Box<dyn Error>> {
    let mut model = Model::new(Language::BritishEnglish);
    let tab = model.comparison_mut();
    tab.set_archive(ComparisonSide::Old, pair.old.clone());
    tab.set_archive(ComparisonSide::New, pair.new.clone());
    if !tab.start(|| {}) {
        return Err("the Comparison did not start".into());
    }
    wait(tab, |tab| {
        !matches!(tab.status(), ComparisonStatus::Running(_))
    })?;
    Ok(model)
}

fn id_of(view: &ComparisonView, path: &str) -> Result<u32, Box<dyn Error>> {
    view.shown()
        .iter()
        .copied()
        .find(|&id| view.change(id).is_some_and(|change| change.path() == path))
        .ok_or_else(|| format!("no change {path}").into())
}

fn loaded(tab: &ComparisonTab) -> bool {
    tab.view()
        .and_then(ComparisonView::diff)
        .is_some_and(|diff| !matches!(diff.status(), DiffStatus::Loading))
}

/// Selects the change at `path`, waits for its line diff and returns the tab.
fn diff_of<'m>(model: &'m mut Model, path: &str) -> Result<&'m mut ComparisonTab, Box<dyn Error>> {
    let tab = model.comparison_mut();
    let view = tab.view_mut().ok_or("no result")?;
    let id = id_of(view, path)?;
    view.select(id);
    view.load_diff(|| {});
    wait(tab, loaded)?;
    Ok(tab)
}

fn document(tab: &ComparisonTab) -> Result<&DiffDocument, Box<dyn Error>> {
    match tab
        .view()
        .and_then(ComparisonView::diff)
        .map(|diff| diff.status())
    {
        Some(DiffStatus::Ready(document)) => Ok(document),
        other => Err(format!("no document: {other:?}").into()),
    }
}

fn text(document: &DiffDocument, side: ComparisonSide, cell: Option<DiffCell>) -> String {
    cell.map_or_default(|cell| String::from_utf8_lossy(document.line(side, cell.line)).into_owned())
}

/// Returns each row as the old and the new line with its kind, or the hunk number.
fn rendered(document: &DiffDocument) -> Vec<String> {
    document
        .rows()
        .iter()
        .map(|row| match *row {
            DiffRow::Hunk(index) => format!("hunk {index}"),
            DiffRow::Lines { old, new } => format!(
                "{:?} {:?} | {:?} {:?}",
                old.map(|cell| cell.kind),
                text(document, ComparisonSide::Old, old).trim_end(),
                new.map(|cell| cell.kind),
                text(document, ComparisonSide::New, new).trim_end(),
            ),
        })
        .collect()
}

#[test]
fn a_changed_text_entry_shows_both_sides_line_by_line() -> TestResult {
    let (_dir, pair) = pair()?;
    let mut model = compared(&pair)?;

    let tab = diff_of(&mut model, TEXT)?;
    let document = document(tab)?;

    let context = Some(LineKind::Context);
    let (removed, added) = (Some(LineKind::Removed), Some(LineKind::Added));
    assert_eq!(
        rendered(document),
        [
            "hunk 0".to_owned(),
            format!("{context:?} \"a\" | {context:?} \"a\""),
            format!("{removed:?} \"b\" | {added:?} \"B\""),
            format!("{context:?} \"c\" | {context:?} \"c\""),
            format!("{context:?} \"d\" | {context:?} \"d\""),
            format!("None \"\" | {added:?} \"e\""),
        ],
        "changed lines side by side, the added one alone"
    );
    let numbers: Vec<_> = document
        .rows()
        .iter()
        .filter_map(|row| match row {
            DiffRow::Lines {
                new: Some(cell), ..
            } => Some(cell.line),
            _ => None,
        })
        .collect();
    assert_eq!(numbers, [0, 1, 2, 3, 4], "zero-based new lines");
    Ok(())
}

#[test]
fn a_changed_cryxml_entry_is_diffed_as_converted_xml() -> TestResult {
    let (_dir, pair) = pair()?;
    let mut model = compared(&pair)?;

    let tab = diff_of(&mut model, CRYXML)?;
    let rows = rendered(document(tab)?);

    let (removed, added) = (Some(LineKind::Removed), Some(LineKind::Added));
    assert!(
        rows.contains(&format!(
            "{removed:?} \"  <Part damageMax=\\\"2500\\\"/>\" | {added:?} \"  <Part damageMax=\\\"3000\\\"/>\""
        )),
        "the changed attribute side by side: {rows:#?}"
    );
    Ok(())
}

#[test]
fn a_changed_binary_entry_has_no_line_diff() -> TestResult {
    let (_dir, pair) = pair()?;
    let mut model = compared(&pair)?;

    let tab = diff_of(&mut model, BINARY)?;
    let status = tab
        .view()
        .and_then(ComparisonView::diff)
        .map(|diff| diff.status());

    assert!(
        matches!(status, Some(DiffStatus::Binary)),
        "binary, got {status:?}"
    );
    Ok(())
}

#[test]
fn navigation_steps_through_the_hunks_and_stops_at_both_ends() -> TestResult {
    let (_dir, pair) = pair()?;
    let mut model = compared(&pair)?;
    let tab = diff_of(&mut model, FAR)?;
    let hunks = document(tab)?.hunks().len();
    let diff = tab
        .view_mut()
        .and_then(ComparisonView::diff_mut)
        .ok_or("no diff")?;

    let first = diff.current_hunk();
    diff.next_hunk();
    let second = diff.current_hunk();
    diff.next_hunk();
    let past_end = diff.current_hunk();
    diff.previous_hunk();
    diff.previous_hunk();

    assert_eq!(hunks, 2, "two changes far apart");
    assert_eq!(
        (first, second, past_end, diff.current_hunk()),
        (Some(0), Some(1), Some(1), Some(0)),
        "first, next, stays at the last, back to the first"
    );
    Ok(())
}

#[test]
fn selecting_another_change_replaces_the_diff_being_made() -> TestResult {
    let (_dir, pair) = pair()?;
    let mut model = compared(&pair)?;
    let tab = model.comparison_mut();
    let view = tab.view_mut().ok_or("no result")?;
    let (binary, text) = (id_of(view, BINARY)?, id_of(view, TEXT)?);

    view.select(binary);
    view.load_diff(|| {});
    view.select(text);
    view.load_diff(|| {});
    wait(tab, loaded)?;

    let diff = tab.view().and_then(ComparisonView::diff).ok_or("no diff")?;
    assert_eq!(diff.change(), text, "the diff of the last selection");
    assert!(
        matches!(diff.status(), DiffStatus::Ready(_)),
        "its text diff"
    );
    Ok(())
}

#[test]
fn a_diff_of_a_hundred_thousand_lines_is_laid_out_and_navigable() -> TestResult {
    let dir = tempfile::tempdir()?;
    let (mut old, mut new) = (String::new(), String::new());
    for line in 0..100_000 {
        let text = format!("<Element name=\"item{line}\" value=\"{line}\"/>\n");
        old.push_str(&text);
        new.push_str(if line % 50 == 0 {
            "<Changed/>\n"
        } else {
            &text
        });
    }
    let mut paths = Vec::new();
    for (channel, body) in [("LIVE", old), ("PTU", new)] {
        let folder = dir.path().join(channel);
        std::fs::create_dir_all(&folder)?;
        let path = folder.join("Data.p4k");
        ArchiveBuilder::new()
            .entry(TestEntry::deflated("Data\\big.xml", body))
            .write_to(&path)?;
        paths.push(path);
    }
    let [old, new] = <[_; 2]>::try_from(paths).map_err(|_| "two Archives")?;
    let mut model = compared(&Pair { old, new })?;

    let tab = diff_of(&mut model, "Data\\big.xml")?;
    let document = document(tab)?;
    let (hunks, rows) = (document.hunks().len(), document.rows().len());
    let diff = tab
        .view_mut()
        .and_then(ComparisonView::diff_mut)
        .ok_or("no diff")?;
    for _ in 0..hunks {
        diff.next_hunk();
    }

    assert_eq!(hunks, 2000, "one hunk per changed line");
    assert_eq!(
        rows,
        2000 * 8 - 3,
        "a head, three lines before, the change and three after; none before the first"
    );
    assert_eq!(diff.current_hunk(), Some(1999), "the last hunk");
    Ok(())
}

#[test]
fn a_new_selection_asks_the_list_once_to_reveal_its_row() -> TestResult {
    let (_dir, pair) = pair()?;
    let mut model = compared(&pair)?;
    let view = model.comparison_mut().view_mut().ok_or("no result")?;
    let far = id_of(view, FAR)?;
    let position = view.shown().iter().position(|&id| id == far);

    view.select(far);
    let first = view.take_reveal();
    let second = view.take_reveal();
    view.select(far);
    let same = view.take_reveal();

    assert_eq!(first, position, "the row of the selection");
    assert_eq!((second, same), (None, None), "once per new selection");
    Ok(())
}

fn status_of(path: &str) -> Result<String, Box<dyn Error>> {
    let (_dir, pair) = pair()?;
    let mut model = compared(&pair)?;
    let tab = diff_of(&mut model, path)?;
    let diff = tab.view().and_then(ComparisonView::diff).ok_or("no diff")?;
    Ok(match diff.status() {
        DiffStatus::Ready(document) => format!("ready, differs {}", document.has_differences()),
        DiffStatus::Failed(reason) => format!("failed {}", reason.is_some()),
        other => format!("{other:?}"),
    })
}

#[test]
fn an_entry_with_too_many_lines_is_too_large() -> TestResult {
    assert_eq!(status_of(TOO_LARGE)?, "TooLarge", "too large");
    Ok(())
}

#[test]
fn a_corrupt_cryxml_side_fails_with_a_reason() -> TestResult {
    assert_eq!(status_of(BROKEN)?, "failed true", "failed with a reason");
    Ok(())
}

#[test]
fn equal_lines_of_changed_bytes_give_a_diff_without_differences() -> TestResult {
    assert_eq!(status_of(EQUAL)?, "ready, differs false", "no line differs");
    Ok(())
}

#[test]
fn an_added_entry_shows_every_line_added_beside_an_empty_old_side() -> TestResult {
    let (_dir, pair) = pair()?;
    let mut model = compared(&pair)?;

    let tab = diff_of(&mut model, ADDED)?;

    let added = Some(LineKind::Added);
    assert_eq!(
        rendered(document(tab)?),
        [
            "hunk 0".to_owned(),
            format!("None \"\" | {added:?} \"x\""),
            format!("None \"\" | {added:?} \"y\""),
        ],
        "only the new side"
    );
    Ok(())
}
