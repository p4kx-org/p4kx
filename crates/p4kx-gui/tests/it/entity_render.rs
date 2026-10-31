// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The Entities tab, the choice of a replacement and the export dialog drawn by wgpu, rendered
//! headless by `egui_kittest`.
//!
//! The test needs a graphics adapter, a software one is enough, and returns early without one.
//! `P4KX_RENDER_DIR` names a folder the rendered windows are written to as PNG files.

use crate::entity_picking;

use std::fs::File;
use std::io::BufWriter;
use std::panic::catch_unwind;
use std::path::PathBuf;

use eframe::egui_wgpu::RendererOptions;
use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use egui_kittest::wgpu::{WgpuTestRenderer, create_render_state};
use entity_picking::{TestResult, entries, listed, select, settle};
use p4kx_gui::{Language, Model, WorkTab, ui};
use p4kx_testkit::wgpu_setup::software_wgpu_setup;

const PRIMARY: [u8; 3] = [0xE7, 0x7E, 0x23];

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

fn render(
    harness: &mut Harness<'_, Model>,
    name: &str,
) -> Result<usize, Box<dyn std::error::Error>> {
    harness.run_steps(3);
    let image = harness.render()?;
    let (width, height) = (image.width(), image.height());
    let data = image.into_raw();
    save(name, width, height, &data)?;
    Ok(data
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|pixel| pixel.get(..3) == Some(&PRIMARY[..]))
        .count())
}

#[test]
fn the_picker_the_loadout_tree_the_replacement_and_the_export_are_drawn() -> TestResult {
    drawn([1280.0, 640.0], "")
}

#[test]
fn the_replacement_and_the_export_fit_the_smallest_window() -> TestResult {
    drawn([640.0, 400.0], "small-")
}

fn drawn(size: [f32; 2], tag: &str) -> TestResult {
    for (language, suffix, replace) in [
        (Language::BritishEnglish, "en-GB", "Replace…"),
        (Language::German, "de-DE", "Ersetzen …"),
    ] {
        let Some(renderer) = renderer() else {
            return Ok(());
        };
        let dir = tempfile::tempdir()?;
        let path = crate::tagging::archive(dir.path(), entries())?;
        let mut harness = Harness::builder()
            .with_context_setup(p4kx_gui::theme::apply)
            .with_size(size)
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
        select(&mut harness, "TEST_Ship")?;
        let orange = render(&mut harness, &format!("entities-{tag}{suffix}.png"))?;
        assert!(
            orange > 100,
            "the rule and the selected row bar: {orange} pixels"
        );

        harness
            .get_all_by_role_and_label(Role::Button, replace)
            .next()
            .ok_or("no replace action")?
            .click();
        render(&mut harness, &format!("entities-replace-{tag}{suffix}.png"))?;
        harness.state_mut().entities_mut().close_replace();
        harness.run_steps(2);

        settle(&mut harness)?;
        harness
            .state_mut()
            .entities_mut()
            .loadout_mut()
            .ok_or("no session")?
            .set_replacement("hardpoint_weapon_nose", "TEST_Shield")?;
        settle(&mut harness)?;
        render(
            &mut harness,
            &format!("entities-mismatch-{tag}{suffix}.png"),
        )?;
        harness.state_mut().begin_assembly_export();
        render(&mut harness, &format!("entities-export-{tag}{suffix}.png"))?;
    }
    Ok(())
}
