// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The progress of the Export of an Entity while a step without a measure runs, rendered headless
//! at 1280 by 800.

use std::fs::File;
use std::io::BufWriter;
use std::panic::catch_unwind;
use std::path::PathBuf;

use eframe::egui_wgpu::RendererOptions;
use egui_kittest::Harness;
use egui_kittest::wgpu::{WgpuTestRenderer, create_render_state};
use p4kx_gui::{
    AssemblyExport, AssemblyExportForm, AssemblyExportStage, AssemblyExportStatus, Language, Model,
    WorkTab, ui,
};
use p4kx_testkit::wgpu_setup::software_wgpu_setup;

use crate::entity_picking::{TestResult, listed, select};

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
fn a_step_without_a_measure_shows_a_sweeping_block_at_1280_by_800() -> TestResult {
    for (language, suffix) in [
        (Language::BritishEnglish, "en-GB"),
        (Language::German, "de-DE"),
    ] {
        let Some(renderer) = renderer() else {
            return Ok(());
        };
        let dir = tempfile::tempdir()?;
        let path = crate::tagging::archive(dir.path(), p4kx_testkit::entity_interior_entries())?;
        let mut harness = Harness::builder()
            .with_context_setup(p4kx_gui::theme::apply)
            .with_size([1280.0, 800.0])
            .renderer(renderer)
            .build_ui_state(
                |ui, model: &mut Model| {
                    ui::show(ui, model);
                },
                Model::new(language),
            );
        harness.state_mut().open(path, || {});
        crate::tagging::wait_until(&mut harness, |model| model.browser().is_some())?;
        harness.state_mut().set_work_tab(WorkTab::Entities);
        harness.run_steps(2);
        crate::tagging::wait_until(&mut harness, listed)?;
        select(&mut harness, p4kx_testkit::ENTITY_INTERIOR_CLASS)?;

        let hold = harness.state_mut().hold_interior_export();
        harness.state_mut().begin_assembly_export();
        let setup = harness
            .state_mut()
            .assembly_export_setup_mut()
            .ok_or("no setup")?;
        setup.set_form(AssemblyExportForm::Package);
        setup.set_package_folder(dir.path().join("hull"));
        assert!(harness.state_mut().submit_assembly_export(|| {}));
        crate::waiting::wait_for(&mut harness, "the reading step", |harness| {
            matches!(
                harness.state().assembly_export().map(AssemblyExport::status),
                Some(AssemblyExportStatus::Running(progress))
                    if progress.stage == AssemblyExportStage::Reading
            )
        })?;

        for (moment, name) in [(0, "a"), (7, "b")] {
            harness.run_steps(moment + 2);
            let image = harness.render()?;
            let (width, height) = (image.width(), image.height());
            save(
                &format!("progress-reading-{name}-{suffix}.png"),
                width,
                height,
                &image.into_raw(),
            )?;
        }
        drop(hold);
        harness.state().cancel_assembly_export();
    }
    Ok(())
}
