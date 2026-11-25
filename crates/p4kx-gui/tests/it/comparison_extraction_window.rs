// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Extracting the changes of one Comparison side from the window, drawn and driven headless over
//! a pair of synthetic Archives.

use crate::comparing;
use crate::extraction_outputs::assert_receipted_outputs;
use crate::waiting::wait_for_model;

use std::error::Error;
use std::fs;
use std::path::Path;

use comparing::{Pair, TestResult, pair};
use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use p4kx_gui::{ComparisonSide, ComparisonStatus, Language, Model, WorkTab, ui};

fn harness(pair: &Pair, language: Language, out: &Path) -> Harness<'static, Model> {
    let mut model = Model::new(language);
    model.set_work_tab(WorkTab::Comparison);
    model.set_extraction_folder(Some(out.to_path_buf()));
    let tab = model.comparison_mut();
    tab.set_archive(ComparisonSide::Old, pair.old.clone());
    tab.set_archive(ComparisonSide::New, pair.new.clone());
    Harness::builder()
        .with_context_setup(p4kx_gui::theme::apply)
        .with_size([1400.0, 900.0])
        .build_ui_state(
            |ui, model: &mut Model| {
                ui::show(ui, model);
            },
            model,
        )
}

fn settle(harness: &mut Harness<'_, Model>, done: impl Fn(&Model) -> bool) -> TestResult {
    wait_for_model(harness, "the window to settle", done)?;
    harness.step();
    harness.step();
    Ok(())
}

fn finished(model: &Model) -> bool {
    matches!(model.comparison().status(), ComparisonStatus::Finished(view) if !view.is_filtering())
}

fn ended(model: &Model) -> bool {
    model
        .extraction()
        .is_some_and(|extraction| !extraction.is_running())
}

fn compared(
    pair: &Pair,
    language: Language,
    out: &Path,
) -> Result<Harness<'static, Model>, Box<dyn Error>> {
    let mut harness = harness(pair, language, out);
    harness.step();
    let start = if language == Language::German {
        "Vergleichen"
    } else {
        "Compare"
    };
    harness.get_by_label(start).click();
    settle(&mut harness, finished)?;
    Ok(harness)
}

#[test]
fn both_sides_offer_their_changes_as_ghost_actions_with_counts() -> TestResult {
    let pair = pair()?;
    let out = pair.dir.path().join("out");

    let harness = compared(&pair, Language::BritishEnglish, &out)?;

    for label in ["Extract old (2)…", "Extract new (3)…"] {
        assert!(
            !harness.get_by_label(label).accesskit_node().is_disabled(),
            "{label} is enabled"
        );
    }
    Ok(())
}

#[test]
fn the_new_side_extracts_through_the_setup_and_shows_the_summary() -> TestResult {
    let pair = pair()?;
    let out = pair.dir.path().join("out");
    fs::create_dir_all(&out)?;
    let mut harness = compared(&pair, Language::BritishEnglish, &out)?;

    harness.get_by_label("Extract new (3)…").click();
    settle(&mut harness, |model| model.extraction_setup().is_some())?;
    harness.get_by_label("Added and changed Entries of the new Archive (3)");
    harness.get_by_label("Existing folder").click();
    harness.step();
    harness.step();
    harness.get_by_label("Extract").click();
    harness.step();
    harness.step();
    settle(&mut harness, ended)?;

    harness.get_by_label("Extraction finished");
    harness.get_by_label("Extracted 3, skipped 0, failed 0, cancelled 0");
    assert_receipted_outputs(
        &out,
        &[
            "Data\\Libs\\crc.xml",
            "Data\\Libs\\size.xml",
            "Engine\\added.cfg",
        ],
    )
}

#[test]
fn the_old_side_never_extracts_the_removed_entry() -> TestResult {
    let pair = pair()?;
    let out = pair.dir.path().join("out");
    fs::create_dir_all(&out)?;
    let mut harness = compared(&pair, Language::BritishEnglish, &out)?;

    harness.get_by_label("Extract old (2)…").click();
    settle(&mut harness, |model| model.extraction_setup().is_some())?;
    harness.get_by_label("Changed Entries of the old Archive, in their old version (2)");
    harness.get_by_label("Existing folder").click();
    harness.step();
    harness.step();
    harness.get_by_label("Extract").click();
    harness.step();
    harness.step();
    settle(&mut harness, ended)?;

    assert_receipted_outputs(&out, &["Data\\Libs\\crc.xml", "Data\\Libs\\size.xml"])
}

#[test]
fn the_filter_narrows_the_actions_and_the_setup_names_it() -> TestResult {
    let pair = pair()?;
    let out = pair.dir.path().join("out");
    let mut harness = compared(&pair, Language::BritishEnglish, &out)?;

    harness.get_by_role(Role::TextInput).click();
    harness.step();
    harness.get_by_role(Role::TextInput).type_text("engine");
    settle(&mut harness, |model| {
        finished(model)
            && model
                .comparison()
                .view()
                .is_some_and(|view| view.filter_text() == "engine")
    })?;

    assert!(
        harness
            .get_by_label("Extract old (0)…")
            .accesskit_node()
            .is_disabled(),
        "no changed Entry of the old side matches"
    );
    harness.get_by_label("Extract new (1)…").click();
    settle(&mut harness, |model| model.extraction_setup().is_some())?;
    harness.get_by_label("Added and changed Entries of the new Archive (1)");
    harness.get_by_label("Only the changes matching the Filter engine");
    Ok(())
}

fn top_and_bottom(harness: &Harness<'_, Model>, label: &str) -> Result<(f64, f64), Box<dyn Error>> {
    let node = harness.get_by_label(label);
    let rect = node.accesskit_node().bounding_box().ok_or("no bounds")?;
    Ok((rect.y0, rect.y1))
}

fn is_focused(harness: &Harness<'_, Model>, label: &str) -> bool {
    harness.get_by_label(label).accesskit_node().is_focused()
}

#[test]
fn at_900_px_the_actions_wrap_below_the_search_and_keep_their_focus() -> TestResult {
    let pair = pair()?;
    let out = pair.dir.path().join("out");
    let mut harness = compared(&pair, Language::BritishEnglish, &out)?;
    harness.get_by_label("Extract new (3)…").focus();
    harness.step();
    let (wide_top, _) = top_and_bottom(&harness, "Extract new (3)…")?;
    let (_, wide_save_bottom) = top_and_bottom(&harness, "Save report…")?;

    harness.set_size(egui::vec2(900.0, 900.0));
    for _ in 0..4 {
        harness.step();
    }

    assert!(
        wide_top < wide_save_bottom,
        "beside the search field at 1400 px"
    );
    let (narrow_top, _) = top_and_bottom(&harness, "Extract new (3)…")?;
    let (_, save_bottom) = top_and_bottom(&harness, "Save report…")?;
    assert!(narrow_top >= save_bottom, "on a row of its own at 900 px");
    assert!(
        is_focused(&harness, "Extract new (3)…"),
        "the focus survives the wrap"
    );
    Ok(())
}

#[test]
fn the_toolbar_tabs_in_reading_order() -> TestResult {
    let pair = pair()?;
    let out = pair.dir.path().join("out");
    let mut harness = compared(&pair, Language::BritishEnglish, &out)?;
    harness.get_by_role(Role::TextInput).focus();
    harness.step();

    let mut order = Vec::new();
    for label in ["Extract old (2)…", "Extract new (3)…", "Save report…"] {
        harness.key_press(egui::Key::Tab);
        harness.step();
        order.push(is_focused(&harness, label));
    }

    assert_eq!(
        order,
        [true, true, true],
        "left to right after the search field"
    );
    Ok(())
}

#[test]
fn the_german_window_speaks_german() -> TestResult {
    let pair = pair()?;
    let out = pair.dir.path().join("out");
    let mut harness = compared(&pair, Language::German, &out)?;

    harness.get_by_label("Alt extrahieren (2) …");
    harness.get_by_label("Neu extrahieren (3) …").click();
    settle(&mut harness, |model| model.extraction_setup().is_some())?;

    harness.get_by_label("Hinzugefügte und geänderte Einträge des neuen Archivs (3)");
    Ok(())
}
