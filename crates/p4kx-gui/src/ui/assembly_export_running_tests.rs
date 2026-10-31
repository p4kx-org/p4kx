// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The progress window of a running Export as Blender package, rendered headless while the export
//! stays running, `P4KX_RENDER_DIR` names the folder of the PNG files.

use std::fs::File;
use std::io::BufWriter;
use std::panic::catch_unwind;
use std::path::PathBuf;

use eframe::egui_wgpu::RendererOptions;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use egui_kittest::wgpu::{WgpuTestRenderer, create_render_state};
use p4kx_testkit::wgpu_setup::software_wgpu_setup;

use crate::{AssemblyExport, AssemblyExportForm, AssemblyExportStage, Language, Model};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn save(name: &str, width: u32, height: u32, image: &[u8]) -> TestResult {
    let Some(folder) = std::env::var_os("P4KX_RENDER_DIR").map(PathBuf::from) else {
        return Ok(());
    };
    std::fs::create_dir_all(&folder)?;
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
fn renderer() -> Option<WgpuTestRenderer> {
    let state =
        catch_unwind(|| create_render_state(software_wgpu_setup(), RendererOptions::PREDICTABLE))
            .ok();
    if state.is_none() {
        eprintln!("no graphics adapter: the test checks nothing");
    }
    state.map(WgpuTestRenderer::from_render_state)
}

#[test]
fn the_package_progress_window_is_drawn_while_the_export_runs() -> TestResult {
    for (language, suffix, title, text) in [
        (
            Language::BritishEnglish,
            "en-GB",
            "Exporting Blender package",
            "Writing the package of TEST_Ship…",
        ),
        (
            Language::German,
            "de-DE",
            "Blender-Paket wird exportiert",
            "Das Paket von TEST_Ship wird geschrieben …",
        ),
    ] {
        let Some(renderer) = renderer() else {
            return Ok(());
        };
        let (export, _guard) = AssemblyExport::pending(
            ("TEST_Ship", AssemblyExportForm::Package),
            AssemblyExportStage::Writing,
        );
        let mut model = Model::new(language);
        model.show_export(export);
        let mut harness = Harness::builder()
            .with_context_setup(crate::theme::apply)
            .with_size([1280.0, 800.0])
            .renderer(renderer)
            .build_ui_state(
                |ui, model: &mut Model| {
                    crate::ui::show(ui, model);
                },
                model,
            );
        harness.run_steps(3);

        assert!(
            harness
                .state()
                .assembly_export()
                .is_some_and(AssemblyExport::is_running),
            "still running"
        );
        assert!(harness.query_by_label_contains(title).is_some(), "{title}");
        assert!(harness.query_by_label_contains(text).is_some(), "{text}");
        let image = harness.render()?;
        let (width, height) = (image.width(), image.height());
        save(
            &format!("package-running-{suffix}.png"),
            width,
            height,
            &image.into_raw(),
        )?;
    }
    Ok(())
}
