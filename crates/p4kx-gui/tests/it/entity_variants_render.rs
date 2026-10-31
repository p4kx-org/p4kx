// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The Entity picker with folded and expanded variants drawn by wgpu, rendered headless by
//! `egui_kittest`.
//!
//! The test needs a graphics adapter, a software one is enough, and returns early without one.
//! `P4KX_RENDER_DIR` names a folder the rendered windows are written to as PNG files.

use crate::entity_picking::{TestResult, listed};
use crate::entity_variants::entries;
use crate::waiting::act_until;

use std::fs::File;
use std::io::BufWriter;
use std::panic::catch_unwind;
use std::path::PathBuf;

use eframe::egui_wgpu::RendererOptions;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use egui_kittest::wgpu::{WgpuTestRenderer, create_render_state};
use p4kx_gui::{Language, Model, WorkTab, ui};
use p4kx_testkit::wgpu_setup::software_wgpu_setup;

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

fn render(harness: &mut Harness<'_, Model>, name: &str) -> TestResult {
    harness.run_steps(3);
    let image = harness.render()?;
    let (width, height) = (image.width(), image.height());
    save(name, width, height, &image.into_raw())
}

fn list_variants(harness: &mut Harness<'_, Model>, (show, hide): (&str, &str)) -> TestResult {
    act_until(
        harness,
        "listing the variants",
        |harness| harness.get_by_label(show).click(),
        |model| crate::entity_variants::rows(model).len() == 6,
    )?;
    harness.get_by_label(hide).focus();
    harness.run_steps(2);
    harness.hover_at(egui::pos2(300.0, 150.0));
    harness.event(egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta: egui::vec2(0.0, -200.0),
        phase: egui::TouchPhase::Move,
        modifiers: egui::Modifiers::NONE,
        source: egui::MouseWheelSource::Wheel,
    });
    harness.run_steps(3);
    harness.hover_at(egui::pos2(600.0, 5.0));
    harness.run_steps(3);
    Ok(())
}

fn drawn(size: [f32; 2], tag: &str) -> TestResult {
    for (language, suffix, show, hide) in [
        (
            Language::BritishEnglish,
            "en-GB",
            "Show the variants of ACME_Rifle",
            "Hide the variants of ACME_Rifle",
        ),
        (
            Language::German,
            "de-DE",
            "Varianten von ACME_Rifle anzeigen",
            "Varianten von ACME_Rifle ausblenden",
        ),
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
        crate::tagging::wait_until(&mut harness, |model| !model.entities().is_grouping())?;
        render(
            &mut harness,
            &format!("entity-variants-folded-{tag}{suffix}.png"),
        )?;

        list_variants(&mut harness, (show, hide))?;
        render(
            &mut harness,
            &format!("entity-variants-open-{tag}{suffix}.png"),
        )?;

        harness.state_mut().entities_mut().set_query("blue");
        render(
            &mut harness,
            &format!("entity-variants-search-{tag}{suffix}.png"),
        )?;
    }
    Ok(())
}

#[test]
fn the_folded_and_expanded_picker_is_drawn() -> TestResult {
    drawn([1280.0, 800.0], "")
}

#[test]
fn the_folded_and_expanded_picker_fits_the_smallest_window() -> TestResult {
    drawn([640.0, 400.0], "small-")
}
