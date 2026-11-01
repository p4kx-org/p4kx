// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The window with a DataForge file open and the Archive menu showing **Open DataForge file…**,
//! drawn by wgpu and rendered headless by `egui_kittest`.
//!
//! The test needs a graphics adapter, a software one is enough, and returns early without one
//! unless `P4KX_REQUIRE_SOFTWARE_ADAPTER` is set, which CI does. `P4KX_RENDER_DIR` names a folder
//! the rendered windows are written to as PNG files.

use crate::waiting::wait_for_model;

use std::error::Error;
use std::fs::{self, File};
use std::io::BufWriter;
use std::panic::catch_unwind;
use std::path::PathBuf;

use crate::software_wgpu::software_wgpu_setup;
use eframe::egui_wgpu::{RenderState, RendererOptions};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use egui_kittest::wgpu::{WgpuTestRenderer, create_render_state};
use p4kx_gui::{Language, Model, ui};
use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct};

type TestResult = Result<(), Box<dyn Error>>;

const SIZE: [f32; 2] = [1280.0, 800.0];

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
fn the_window_with_a_dataforge_file_and_the_archive_menu_are_drawn() -> TestResult {
    let dir = tempfile::tempdir()?;
    let file = dir.path().join("Game2.dcb");
    let database = DataForgeBuilder::new()
        .structure(TestStruct::new("Vehicle"))
        .record(TestRecord::new(
            1,
            "Vehicle",
            "Vehicle.Cart",
            "libs/foundry/records/vehicles/cart.xml",
        ))
        .build();
    fs::write(&file, database)?;

    for (language, tag, open) in [
        (Language::BritishEnglish, "en", "Open DataForge file…"),
        (Language::German, "de", "DataForge-Datei öffnen …"),
    ] {
        let Some(state) = adapter() else {
            return Ok(());
        };
        let mut model = Model::new(language);
        model.open(file.clone(), || {});
        let mut harness = Harness::builder()
            .with_context_setup(p4kx_gui::theme::apply)
            .with_size(SIZE)
            .renderer(WgpuTestRenderer::from_render_state(state))
            .build_ui_state(
                |ui, model: &mut Model| {
                    ui::show(ui, model);
                },
                model,
            );
        wait_for_model(&mut harness, "the file to open", |model| {
            model.browser().is_some()
        })?;
        harness.run_steps(3);
        let image = harness.render()?;
        save(
            &format!("dataforge-file-{tag}.png"),
            image.width(),
            image.height(),
            image.as_raw(),
        )?;

        let menu = if tag == "en" { "Archive" } else { "Archiv" };
        harness.get_by_label(menu).click();
        harness.run_steps(3);
        let _entry = harness.get_by_label(open);
        let image = harness.render()?;
        save(
            &format!("dataforge-file-menu-{tag}.png"),
            image.width(),
            image.height(),
            image.as_raw(),
        )?;
    }
    Ok(())
}
