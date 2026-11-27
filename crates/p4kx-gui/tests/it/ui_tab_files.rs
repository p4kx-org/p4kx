// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The UI tab over GFx movies: the list, the search, the picture, its contents and its report,
//! driven headless over a synthetic Archive.

use egui::accesskit::Role;
use egui_kittest::kittest::Queryable;
use p4kx_gui::{Fact, Language, Model, UiDetail, UiFilesStatus, UiSize, UiText, WorkTab};
use p4kx_testkit::{TestEntry, test_key};

use crate::tagging::{archive, harness, wait_until};
use crate::ui_tabbing::{
    PANEL, PLAIN, TestResult, assert_clear_stage_origins, drawn, listed, opened, panel_movie,
    pick_file, position_of,
};

const WINDOW: [f32; 2] = [1280.0, 800.0];

pub(crate) fn pixel(model: &Model, x: usize, y: usize) -> Option<[u8; 4]> {
    let image = model.ui().rendered()?.image();
    image
        .pixels
        .get(y * image.size[0] + x)
        .map(egui::Color32::to_array)
}

#[test]
fn the_ui_tab_sits_next_to_the_others_and_asks_for_an_archive() {
    let mut harness = harness(Model::new(Language::BritishEnglish));
    harness.run_steps(2);

    harness.get_by_role_and_label(Role::Tab, "UI").click();
    harness.run_steps(2);

    assert_eq!(harness.state().work_tab(), WorkTab::Ui, "switched");
    assert!(
        harness.query_by_label("No Archive open").is_some(),
        "asks for an Archive"
    );
}

#[test]
fn the_movies_of_the_archive_are_listed_off_the_ui_thread() -> TestResult {
    let mut window = opened(Language::BritishEnglish, WINDOW)?;
    window.harness.step();
    assert!(
        matches!(
            window.harness.state().ui().files_status(),
            UiFilesStatus::Loading | UiFilesStatus::Ready(_)
        ),
        "the lookup started with the first frame"
    );

    wait_until(&mut window.harness, |model| {
        matches!(model.ui().files_status(), UiFilesStatus::Ready(_))
    })?;

    let UiFilesStatus::Ready(list) = window.harness.state().ui().files_status() else {
        return Err("the files are not listed".into());
    };
    assert_eq!(list.total(), 4, "the fonts, panel, plain and broken movies");
    assert!(
        window.harness.query_by_label("panel.gfx").is_some(),
        "a row per movie"
    );
    assert!(
        window.harness.query_by_label("readme.txt").is_none(),
        "no other file"
    );
    assert!(
        window.harness.query_by_label("4 files").is_some(),
        "the footer counts them"
    );
    assert!(
        window
            .harness
            .query_by_label("Pick a file to see its picture.")
            .is_some(),
        "asks to pick one"
    );
    Ok(())
}

#[test]
fn a_search_narrows_the_list_by_path() -> TestResult {
    let mut window = listed(Language::BritishEnglish, WINDOW)?;

    window.harness.get_by_role(Role::TextInput).click();
    window.harness.run_steps(2);
    window
        .harness
        .get_by_role(Role::TextInput)
        .type_text("PLAIN");
    window.harness.run_steps(3);

    assert_eq!(
        window.harness.state().ui().query(),
        "PLAIN",
        "the search text"
    );
    assert!(
        window.harness.query_by_label("1 of 4 files").is_some(),
        "the footer counts the matches"
    );
    assert!(
        window.harness.query_by_label("plain.swf").is_some(),
        "plain"
    );
    assert!(
        window.harness.query_by_label("panel.gfx").is_none(),
        "panel"
    );
    Ok(())
}

#[test]
fn a_picked_movie_is_drawn_off_the_ui_thread_with_its_contents() -> TestResult {
    let mut window = listed(Language::BritishEnglish, WINDOW)?;
    let at = position_of(window.harness.state(), "panel.gfx").ok_or("not listed")?;

    window.harness.state_mut().ui_mut().select_file(at);
    window.harness.step();
    assert!(
        window.harness.state().ui().is_rendering(),
        "drawing runs on a thread of its own"
    );
    wait_until(&mut window.harness, drawn)?;

    let model = window.harness.state();
    let rendered = model.ui().rendered().ok_or("no picture")?;
    assert_eq!((rendered.width(), rendered.height()), (64, 32), "the stage");
    assert_eq!(pixel(model, 8, 8), Some([255, 0, 0, 255]), "the red half");
    assert_eq!(pixel(model, 48, 8), Some([0, 0, 0, 0]), "the missing fill");
    let origins = rendered.origins();
    assert_eq!(
        origins.image.contains_encrypted_source(),
        None,
        "missing bitmap"
    );
    assert!(
        !origins.image.source_uses_refs().is_empty(),
        "the recorded paints still have origins"
    );
    assert_clear_stage_origins(origins);
    let contents = model.ui().contents().ok_or("no contents")?;
    assert!(
        contents
            .facts()
            .iter()
            .any(|(key, value)| *key == UiText::FactStage
                && *value == Fact::Text("64 × 32 px".to_owned())),
        "the stage: {:?}",
        contents.facts()
    );
    let kinds: Vec<UiText> = contents.rows().iter().map(|row| row.kind).collect();
    assert_eq!(
        kinds,
        [UiText::KindShape, UiText::KindShape, UiText::KindBitmap],
        "two shapes and the external bitmap"
    );
    assert!(
        window.harness.query_by_label("Contents 3").is_some(),
        "the contents tab counts its rows"
    );
    assert!(
        window.harness.query_by_label("64 × 32 px").is_some(),
        "the facts are shown"
    );
    Ok(())
}

#[test]
fn what_the_picture_leaves_out_is_reported_like_the_cli_does() -> TestResult {
    let mut window = listed(Language::BritishEnglish, WINDOW)?;
    pick_file(&mut window, "panel.gfx")?;

    let report = window
        .harness
        .state()
        .ui()
        .rendered()
        .ok_or("no picture")?
        .report();
    let lines: Vec<UiText> = report.lines().iter().map(|line| line.label).collect();
    assert!(lines.contains(&UiText::ReportBitmap), "{lines:?}");
    assert!(
        report
            .lines()
            .iter()
            .any(|line| line.detail.starts_with("bitmap 3:")),
        "{:?}",
        report.lines()
    );
    let count = report.len();
    assert_eq!(
        count, 2,
        "the fill that is skipped and the bitmap behind it"
    );
    window
        .harness
        .get_by_role_and_label(Role::Button, &format!("Left out {count}"))
        .click();
    window.harness.run_steps(3);
    assert_eq!(
        window.harness.state().ui().detail(),
        UiDetail::Report,
        "switched"
    );
    assert!(
        window.harness.query_by_label("Unreadable bitmap").is_some(),
        "the kind of the line is shown"
    );
    Ok(())
}

#[test]
fn a_movie_drawn_completely_says_so() -> TestResult {
    let mut window = listed(Language::BritishEnglish, WINDOW)?;
    pick_file(&mut window, PLAIN)?;
    window
        .harness
        .state_mut()
        .ui_mut()
        .set_detail(UiDetail::Report);
    window.harness.run_steps(3);

    let model = window.harness.state();
    assert!(
        model
            .ui()
            .rendered()
            .ok_or("no picture")?
            .report()
            .is_empty(),
        "nothing left out"
    );
    assert_eq!(pixel(model, 4, 4), Some([0, 0, 255, 255]), "blue");
    let origins = model.ui().rendered().ok_or("no picture")?.origins();
    assert_eq!(
        origins.image.contains_encrypted_source(),
        Some(false),
        "complete clear picture"
    );
    assert!(
        window
            .harness
            .query_by_label_contains("Unencrypted source data")
            .is_some(),
        "the displayed picture exposes its marker"
    );
    assert!(
        window
            .harness
            .query_by_label("Everything was drawn.")
            .is_some(),
        "said"
    );
    Ok(())
}

#[test]
fn a_movie_with_an_omission_retains_proved_encryption() -> TestResult {
    let folder = tempfile::tempdir()?;
    let path = archive(
        folder.path(),
        vec![TestEntry::stored(PANEL, panel_movie()).encrypted()],
    )?;
    let mut model = Model::new(Language::BritishEnglish);
    model.set_archive_key(Some(test_key()));
    let mut harness = harness(model);
    harness.state_mut().open(path, || {});
    wait_until(&mut harness, |model| model.browser().is_some())?;
    harness.state_mut().set_work_tab(WorkTab::Ui);
    wait_until(&mut harness, |model| {
        position_of(model, "panel.gfx").is_some()
    })?;
    let position = position_of(harness.state(), "panel.gfx").ok_or("movie is not listed")?;
    harness.state_mut().ui_mut().select_file(position);
    wait_until(&mut harness, drawn)?;

    let picture = harness.state().ui().rendered().ok_or("no picture")?;
    let origins = picture.origins();
    assert!(!origins.image.source_set_complete(), "an input is missing");
    assert_eq!(
        origins.image.contains_encrypted_source(),
        Some(true),
        "the actual encrypted movie still contributes"
    );
    assert_eq!(
        origins.report.contains_encrypted_source(),
        None,
        "aggregate report has its own evidence"
    );
    assert_eq!(
        pixel(harness.state(), 8, 8),
        Some([255, 0, 0, 255]),
        "known paint stays visible"
    );
    assert!(
        harness
            .query_by_label_contains("Includes encrypted source data")
            .is_some()
    );
    Ok(())
}

#[test]
fn a_file_that_is_no_movie_gives_the_reason_instead_of_a_picture() -> TestResult {
    let mut window = listed(Language::BritishEnglish, WINDOW)?;
    pick_file(&mut window, "broken.gfx")?;

    let model = window.harness.state();
    assert!(model.ui().rendered().is_none(), "no picture");
    assert!(
        matches!(
            model.ui().render_failure(),
            Some(p4kx_gui::UiRenderFailure::Failed(Some(_)))
        ),
        "{:?}",
        model.ui().render_failure()
    );
    assert!(
        window
            .harness
            .query_by_label_contains("The picture cannot be drawn")
            .is_some(),
        "the reason is shown"
    );
    Ok(())
}

#[test]
fn the_size_and_the_background_redraw_the_picture() -> TestResult {
    let mut window = listed(Language::BritishEnglish, WINDOW)?;
    pick_file(&mut window, "panel.gfx")?;

    window
        .harness
        .get_by_role_and_label(Role::Button, "1024 px wide")
        .click();
    wait_until(&mut window.harness, |model| {
        drawn(model)
            && model
                .ui()
                .rendered()
                .is_some_and(|rendered| rendered.width() == 1024)
    })?;
    let model = window.harness.state();
    assert_eq!(model.ui().settings().size, UiSize::Wide1024, "the choice");
    assert_eq!(
        model.ui().rendered().map(p4kx_gui::UiRendered::height),
        Some(512),
        "the aspect ratio of the stage is kept"
    );
    assert_eq!(pixel(model, 800, 100), Some([0, 0, 0, 0]), "transparent");
    let origins = model.ui().rendered().ok_or("no resized picture")?.origins();
    assert!(
        origins.width.source_uses_refs().is_empty(),
        "the width belongs to the new explicit setting"
    );
    assert!(
        !origins.height.source_uses_refs().is_empty(),
        "the height still uses the stage aspect ratio"
    );

    window
        .harness
        .get_by_role_and_label(Role::Button, "Black")
        .click();
    wait_until(&mut window.harness, |model| {
        drawn(model) && pixel(model, 800, 100) == Some([0, 0, 0, 255])
    })?;
    Ok(())
}

#[test]
fn the_labels_are_german_in_the_german_window() -> TestResult {
    let mut window = listed(Language::German, WINDOW)?;
    pick_file(&mut window, "panel.gfx")?;

    for label in ["Bühne", "Schwarz", "Inhalt 3", "Ausgelassen 2"] {
        assert!(window.harness.query_by_label(label).is_some(), "{label}");
    }
    assert!(
        window.harness.query_by_label("4 Dateien").is_some(),
        "count"
    );
    Ok(())
}
