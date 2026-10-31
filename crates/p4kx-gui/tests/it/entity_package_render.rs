// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The export dialog of an Entity as a Blender package, rendered headless at 1280 by 800.

use std::fs::File;
use std::io::BufWriter;
use std::panic::catch_unwind;
use std::path::PathBuf;

use eframe::egui_wgpu::RendererOptions;
use egui_kittest::Harness;
use egui_kittest::wgpu::{WgpuTestRenderer, create_render_state, default_wgpu_setup};
use p4kx_gui::{AssemblyExportForm, Language, Model, WorkTab, ui};
use p4kx_testkit::{ENTITY_INTERIOR_CLASS, entity_interior_entries};

use crate::entity_picking::{TestResult, listed, select};

pub(crate) fn save(name: &str, width: u32, height: u32, image: &[u8]) -> TestResult {
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
pub(crate) fn renderer() -> Option<WgpuTestRenderer> {
    let state =
        catch_unwind(|| create_render_state(default_wgpu_setup(), RendererOptions::PREDICTABLE))
            .ok();
    if state.is_none() {
        eprintln!("no graphics adapter: the test checks nothing");
    }
    state.map(WgpuTestRenderer::from_render_state)
}

pub(crate) fn render(harness: &mut Harness<'_, Model>, name: &str) -> TestResult {
    harness.run_steps(12);
    let image = harness.render()?;
    let (width, height) = (image.width(), image.height());
    save(name, width, height, &image.into_raw())
}

#[test]
#[expect(clippy::too_many_lines, reason = "one render per state of the dialog")]
fn the_dialog_the_progress_and_the_summary_are_drawn_at_1280_by_800() -> TestResult {
    for (language, suffix) in [
        (Language::BritishEnglish, "en-GB"),
        (Language::German, "de-DE"),
    ] {
        let Some(renderer) = renderer() else {
            return Ok(());
        };
        let dir = tempfile::tempdir()?;
        let path = crate::tagging::archive(dir.path(), entity_interior_entries())?;
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
        select(&mut harness, ENTITY_INTERIOR_CLASS)?;

        harness.state_mut().begin_assembly_export();
        render(&mut harness, &format!("package-glb-interior-{suffix}.png"))?;

        let folder = dir.path().join("hull");
        let setup = harness
            .state_mut()
            .assembly_export_setup_mut()
            .ok_or("no setup")?;
        setup.set_interior(true);
        setup.set_form(AssemblyExportForm::Package);
        render(&mut harness, &format!("package-empty-{suffix}.png"))?;

        let setup = harness
            .state_mut()
            .assembly_export_setup_mut()
            .ok_or("no setup")?;
        setup.set_package_folder(folder.clone());
        render(&mut harness, &format!("package-chosen-{suffix}.png"))?;

        assert!(harness.state_mut().submit_assembly_export(|| {}));
        harness.run_steps(2);
        crate::entity_package_window::finished(&mut harness)?;
        render(&mut harness, &format!("package-summary-{suffix}.png"))?;

        harness.state_mut().begin_assembly_export();
        let setup = harness
            .state_mut()
            .assembly_export_setup_mut()
            .ok_or("no setup")?;
        setup.set_form(AssemblyExportForm::Package);
        setup.set_package_folder(folder);
        render(&mut harness, &format!("package-replaces-{suffix}.png"))?;

        let notes = dir.path().join("notes");
        std::fs::create_dir(&notes)?;
        std::fs::write(notes.join("a.txt"), b"x")?;
        let setup = harness
            .state_mut()
            .assembly_export_setup_mut()
            .ok_or("no setup")?;
        setup.set_package_folder(notes);
        render(&mut harness, &format!("package-refused-{suffix}.png"))?;
    }
    Ok(())
}
