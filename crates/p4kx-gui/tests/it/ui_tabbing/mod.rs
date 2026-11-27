// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Helpers of the UI tab tests: a synthetic Archive with two GFx movies, the screens of a ship,
//! the fonts and the texts of two languages, opened in a headless window.

use std::cell::Cell;
use std::error::Error;
use std::path::PathBuf;
use std::rc::Rc;

use egui_kittest::Harness;
use egui_kittest::wgpu::WgpuTestRenderer;
use p4kx_gui::ui::Dialog;
use p4kx_gui::{Language, Model, UiExportStatus, UiPreviewOrigins, WorkTab, ui};
use p4kx_testkit::{
    GlobalIniBuilder, TestEntry, TestFill, TestMovie, TestPlace, TestShape, TestShapeRecord,
    TestSignature, external_image_body, screens_font_movie, screens_sample,
};
use tempfile::TempDir;

use crate::tagging::{archive, wait_until};

pub(crate) type TestResult = Result<(), Box<dyn Error>>;

/// The movie with a red rectangle and a bitmap fill the Archive cannot supply.
pub(crate) const PANEL: &str = "Data\\UI\\Test\\panel.gfx";
/// The movie with a blue rectangle that is drawn completely.
pub(crate) const PLAIN: &str = "Data\\UI\\Test\\plain.swf";
/// A file with the extension of a movie that is none.
pub(crate) const BROKEN: &str = "Data\\UI\\Test\\broken.gfx";

fn rectangle(id: u16, fill: TestFill, (width, height): (i32, i32)) -> TestShape {
    TestShape::new(id, 3, [0, width, 0, height])
        .fill(fill)
        .record(TestShapeRecord::Move {
            to: (0, 0),
            fill0: 0,
            fill1: 1,
            line: 0,
        })
        .record(TestShapeRecord::Line(width, 0))
        .record(TestShapeRecord::Line(0, height))
        .record(TestShapeRecord::Line(-width, 0))
        .record(TestShapeRecord::Line(0, -height))
}

/// Returns a 64 x 32 movie whose left half is red and whose right half is filled with a texture
/// that is not in the Archive.
pub(crate) fn panel_movie() -> Vec<u8> {
    let red = rectangle(1, TestFill::Solid([255, 0, 0, 255]), (640, 640));
    let missing = rectangle(
        2,
        TestFill::ScaledBitmap {
            id: 3,
            scale: 20,
            at: (0, 0),
            repeating: false,
            smoothed: false,
        },
        (640, 640),
    );
    TestMovie::new(TestSignature::Cfx)
        .stage(64, 32)
        .tag(
            1009,
            &external_image_body(3, 2, 14, (8, 4), ("gone", "../Textures/gone.dds")),
        )
        .tag(red.code(), &red.body())
        .tag(missing.code(), &missing.body())
        .tag(26, &TestPlace::new(1, 1).body2())
        .tag(26, &TestPlace::new(2, 2).at(640, 0).body2())
        .tag(1, &[])
        .bytes()
}

/// Returns a 32 x 16 movie that is blue all over.
pub(crate) fn plain_movie() -> Vec<u8> {
    let blue = rectangle(1, TestFill::Solid([0, 0, 255, 255]), (640, 320));
    TestMovie::new(TestSignature::Fws)
        .stage(32, 16)
        .tag(blue.code(), &blue.body())
        .tag(26, &TestPlace::new(1, 1).body2())
        .tag(1, &[])
        .bytes()
}

/// Returns the Entries of the Archive.
pub(crate) fn entries() -> Vec<TestEntry> {
    let texts = |label: &str| GlobalIniBuilder::new().text("test_label", label).build();
    vec![
        TestEntry::zstd("Data\\Game2.dcb", screens_sample().build()),
        TestEntry::stored(
            "Data\\UI\\fonts\\Shared\\fonts_en.gfx",
            screens_font_movie(),
        ),
        TestEntry::stored("Data\\Localization\\english\\global.ini", texts("AB")),
        TestEntry::stored(
            "Data\\Localization\\german_(germany)\\global.ini",
            texts("BA"),
        ),
        TestEntry::stored(PANEL, panel_movie()),
        TestEntry::stored(PLAIN, plain_movie()),
        TestEntry::stored(BROKEN, "this is no movie"),
        TestEntry::stored("Data\\readme.txt", "no movie"),
    ]
}

/// A headless window on the UI tab of an open Archive.
pub(crate) struct Window {
    pub(crate) dir: TempDir,
    pub(crate) archive: PathBuf,
    pub(crate) harness: Harness<'static, Model>,
    pub(crate) asked: Rc<Cell<Option<Dialog>>>,
}

pub(crate) fn opened(language: Language, size: [f32; 2]) -> Result<Window, Box<dyn Error>> {
    open_with(language, size, None)
}

/// Opens the window like [`opened`], drawing with `renderer` when there is one.
pub(crate) fn open_with(
    language: Language,
    size: [f32; 2],
    renderer: Option<WgpuTestRenderer>,
) -> Result<Window, Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let path = archive(dir.path(), entries())?;
    let asked = Rc::new(Cell::new(None));
    let sink = Rc::clone(&asked);
    let mut builder = Harness::builder()
        .with_context_setup(p4kx_gui::theme::apply)
        .with_size(size);
    if let Some(renderer) = renderer {
        builder = builder.renderer(renderer);
    }
    let mut harness = builder.build_ui_state(
        move |ui, model: &mut Model| {
            if let Some(dialog) = ui::show(ui, model).dialog {
                sink.set(Some(dialog));
            }
        },
        Model::new(language),
    );
    harness.state_mut().open(path.clone(), || {});
    wait_until(&mut harness, |model| model.browser().is_some())?;
    harness.state_mut().set_work_tab(WorkTab::Ui);
    harness.run_steps(2);
    Ok(Window {
        dir,
        archive: path,
        harness,
        asked,
    })
}

/// Opens the window and waits until the UI files are listed.
pub(crate) fn listed(language: Language, size: [f32; 2]) -> Result<Window, Box<dyn Error>> {
    listed_with(language, size, None)
}

/// Opens the window like [`listed`], drawing with `renderer` when there is one.
pub(crate) fn listed_with(
    language: Language,
    size: [f32; 2],
    renderer: Option<WgpuTestRenderer>,
) -> Result<Window, Box<dyn Error>> {
    let mut window = open_with(language, size, renderer)?;
    wait_until(&mut window.harness, |model| {
        !matches!(
            model.ui().files_status(),
            p4kx_gui::UiFilesStatus::Idle | p4kx_gui::UiFilesStatus::Loading
        )
    })?;
    Ok(window)
}

/// Returns whether a picture or the reason for none is there and no drawing is under way.
pub(crate) const fn drawn(model: &Model) -> bool {
    let tab = model.ui();
    !tab.is_rendering() && (tab.rendered().is_some() || tab.render_failure().is_some())
}

/// Asserts that each preview dimension uses the clear source stage.
pub(crate) fn assert_clear_stage_origins(origins: &UiPreviewOrigins) {
    for dimension in [&origins.width, &origins.height] {
        assert_eq!(
            dimension.contains_encrypted_source(),
            Some(false),
            "clear movie stage"
        );
        assert!(
            origins
                .inventory
                .iter_uses()
                .any(|usage| dimension.source_uses_refs().contains(usage.id())),
            "the stage dimension resolves in this preview's inventory"
        );
    }
}

/// Returns whether the export ended.
pub(crate) fn exported(model: &Model) -> bool {
    model
        .ui()
        .export()
        .is_some_and(|export| !matches!(export.status(), UiExportStatus::Running))
}

/// Returns the position of the shown file whose path ends with `name`.
pub(crate) fn position_of(model: &Model, name: &str) -> Option<usize> {
    let p4kx_gui::UiFilesStatus::Ready(list) = model.ui().files_status() else {
        return None;
    };
    (0..list.shown_len()).find(|&at| list.get(at).is_some_and(|path| path.ends_with(name)))
}

/// Picks the file whose path ends with `name` and waits until its picture is drawn.
pub(crate) fn pick_file(window: &mut Window, name: &str) -> TestResult {
    let position = position_of(window.harness.state(), name).ok_or("the file is not listed")?;
    window.harness.state_mut().ui_mut().select_file(position);
    wait_until(&mut window.harness, |model| {
        drawn(model) && model.ui().selected_file().is_some()
    })
}

/// Switches to the ships, picks the first one and waits until its first screen is drawn.
pub(crate) fn pick_ship(window: &mut Window) -> TestResult {
    window
        .harness
        .state_mut()
        .ui_mut()
        .set_mode(p4kx_gui::UiMode::Ships);
    wait_until(&mut window.harness, |model| model.ui().ships().is_some())?;
    window.harness.state_mut().select_ui_ship(0, || {});
    wait_until(&mut window.harness, |model| {
        model
            .ui()
            .ship()
            .and_then(p4kx_gui::ShipPick::screen)
            .is_some()
            && drawn(model)
    })
}
