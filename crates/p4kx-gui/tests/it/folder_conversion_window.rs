// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The Convert folder entry of the Archive menu and its windows, driven headless through
//! `egui_kittest` and drawn by wgpu in both languages.
//!
//! The render test needs a graphics adapter, a software one is enough, and returns early without
//! one unless `P4KX_REQUIRE_SOFTWARE_ADAPTER` is set. `P4KX_RENDER_DIR` names a folder the
//! rendered windows are written to as PNG files.

use std::error::Error;
use std::fs::{self, File};
use std::io::BufWriter;
use std::panic::catch_unwind;
use std::path::{Path, PathBuf};

use crate::software_wgpu::software_wgpu_setup;
use crate::waiting::{RunQuiet, wait_for};
use eframe::egui_wgpu::{RenderState, RendererOptions};
use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use egui_kittest::wgpu::{WgpuTestRenderer, create_render_state};
use p4kx_gui::ui::{self, Dialog};
use p4kx_gui::{ExtractionStatus, Language, Model};
use p4kx_testkit::{CryXmlByteOrder, CryXmlSignature, TestElement};
use tempfile::TempDir;

type TestResult = Result<(), Box<dyn Error>>;

struct Window {
    model: Model,
    dialog: Option<Dialog>,
}

fn folders() -> Result<(TempDir, PathBuf, PathBuf), Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let input = dir.path().join("raw");
    fs::create_dir_all(input.join("Data"))?;
    let ship = TestElement::new("Ship")
        .attribute("name", "Gladius")
        .to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Little);
    fs::write(input.join("Data").join("gladius.xml"), ship)?;
    fs::write(input.join("Data").join("readme.bin"), b"plain")?;
    let output = dir.path().join("converted");
    Ok((dir, input, output))
}

fn convert(model: &mut Model, input: &Path, output: &Path) {
    model.begin_folder_conversion(input.to_path_buf());
    assert!(
        model.start_folder_conversion(output.to_path_buf(), || {}),
        "started"
    );
}

fn harness(model: Model, size: [f32; 2]) -> Harness<'static, Window> {
    let mut harness = Harness::builder()
        .with_context_setup(p4kx_gui::theme::apply)
        .with_size(size)
        .build_ui_state(
            |ui, window: &mut Window| {
                let requests = ui::show(ui, &mut window.model);
                if requests.dialog.is_some() {
                    window.dialog = requests.dialog;
                }
            },
            Window {
                model,
                dialog: None,
            },
        );
    harness.run_steps(2);
    harness
}

fn wait_for_the_end(harness: &mut Harness<'_, Window>) -> TestResult {
    wait_for(harness, "the conversion to end", |harness| {
        harness
            .state()
            .model
            .extraction()
            .is_some_and(|run| !run.is_running())
    })?;
    harness.run_quiet();
    Ok(())
}

#[test]
fn the_archive_menu_asks_for_the_input_folder() {
    let mut harness = harness(Model::new(Language::BritishEnglish), [1400.0, 900.0]);

    harness
        .get_by_role_and_label(Role::Button, "Archive")
        .click();
    harness.run_quiet();
    harness.get_by_label("Convert folder…").click();
    harness.run_quiet();

    assert_eq!(harness.state().dialog, Some(Dialog::ConversionInput));
}

#[test]
fn a_finished_conversion_shows_its_counts_and_the_folders() -> TestResult {
    let (_dir, input, output) = folders()?;
    let mut model = Model::new(Language::BritishEnglish);
    convert(&mut model, &input, &output);
    let mut harness = harness(model, [1400.0, 900.0]);

    wait_for_the_end(&mut harness)?;

    harness.get_by_label("Folder converted");
    harness.get_by_label(
        "Written 1 (1 converted), kept 0, failed 0, cancelled 0, left out without a conversion 1",
    );
    harness.get_by_label(&format!("Input folder: {}", input.display()));
    assert!(
        output.join("Data").join("gladius.xml").is_file(),
        "converted"
    );
    assert!(!output.join("Data").join("readme.bin").exists(), "left out");
    harness.get_by_label("Close").click();
    harness.run_quiet();
    assert!(harness.state().model.extraction().is_none(), "dismissed");
    Ok(())
}

#[test]
fn an_output_inside_the_input_is_reported_as_failed() -> TestResult {
    let (_dir, input, _) = folders()?;
    let mut model = Model::new(Language::BritishEnglish);
    convert(&mut model, &input, &input.join("inside"));
    let mut harness = harness(model, [1400.0, 900.0]);

    wait_for_the_end(&mut harness)?;

    harness.get_by_label("Conversion failed");
    harness.get_by_label_contains("overlap");
    assert!(
        matches!(
            harness.state().model.extraction().map(|run| run.status()),
            Some(ExtractionStatus::Failed(Some(_)))
        ),
        "failed with a reason"
    );
    assert!(!input.join("inside").exists(), "nothing was created");
    Ok(())
}

fn save(name: &str, width: u32, height: u32, image: &[u8]) -> TestResult {
    let Some(folder) = std::env::var_os("P4KX_RENDER_DIR").map(PathBuf::from) else {
        return Ok(());
    };
    fs::create_dir_all(&folder)?;
    let mut encoder = png::Encoder::new(
        BufWriter::new(File::create(folder.join(name))?),
        width,
        height,
    );
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header()?.write_image_data(image)?;
    Ok(())
}

#[expect(
    clippy::print_stderr,
    reason = "tells the test run why a test checked nothing"
)]
fn adapter() -> Option<RenderState> {
    let state =
        catch_unwind(|| create_render_state(software_wgpu_setup(), RendererOptions::PREDICTABLE))
            .ok();
    if state.is_none() {
        assert!(
            std::env::var_os("P4KX_REQUIRE_SOFTWARE_ADAPTER").is_none(),
            "a software adapter is required"
        );
        eprintln!("no graphics adapter: the test checks nothing");
    }
    state
}

#[test]
fn the_summary_is_drawn_in_both_languages() -> TestResult {
    for (language, tag, close) in [
        (Language::BritishEnglish, "en", "Close"),
        (Language::German, "de", "Schließen"),
    ] {
        let Some(state) = adapter() else {
            return Ok(());
        };
        let (_dir, input, output) = folders()?;
        let mut model = Model::new(language);
        convert(&mut model, &input, &output);
        let mut harness = Harness::builder()
            .with_context_setup(p4kx_gui::theme::apply)
            .with_size([1280.0, 800.0])
            .renderer(WgpuTestRenderer::from_render_state(state))
            .build_ui_state(
                |ui, model: &mut Model| {
                    ui::show(ui, model);
                },
                model,
            );
        wait_for(&mut harness, "the conversion to end", |harness| {
            harness
                .state()
                .extraction()
                .is_some_and(|run| !run.is_running())
        })?;
        harness.run_steps(3);

        let button = harness.get_by_label(close).rect();
        assert!(
            egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 800.0))
                .contains_rect(button),
            "`{close}` lies inside the window"
        );
        let image = harness.render()?;
        save(
            &format!("folder-conversion-summary-{tag}.png"),
            image.width(),
            image.height(),
            image.as_raw(),
        )?;
    }
    Ok(())
}
