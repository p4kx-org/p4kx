// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The tags option of the Record export and the tag path in the Record preview, drawn by wgpu and
//! rendered headless by `egui_kittest`.
//!
//! The tests need a graphics adapter, a software one is enough, and return early without one.
//! `P4KX_RENDER_DIR` names a folder the rendered windows are written to as PNG files.

use crate::exporting;
use crate::record_tags::{GEAR_NODE, forge};

use std::fs::File;
use std::io::BufWriter;
use std::panic::catch_unwind;
use std::path::PathBuf;

use crate::software_wgpu::software_wgpu_setup;
use eframe::egui_wgpu::RendererOptions;
use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use egui_kittest::wgpu::{WgpuTestRenderer, create_render_state};
use exporting::{TestResult, opened, select_node, wait_until};
use p4kx_gui::{Browser, Language, Model, PreviewTab, RecordForm, RecordPreviewStatus, ui};

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

fn window(
    renderer: WgpuTestRenderer,
    size: [f32; 2],
    language: Language,
    model: Model,
) -> Harness<'static, Model> {
    let mut model = model;
    model.set_language(language);
    let mut harness = Harness::builder()
        .with_context_setup(p4kx_gui::theme::apply)
        .with_size(size)
        .renderer(renderer)
        .build_ui_state(
            |ui, model: &mut Model| {
                ui::show(ui, model);
            },
            model,
        );
    harness.run_steps(3);
    harness
}

fn render(harness: &mut Harness<'_, Model>, name: &str) -> TestResult {
    harness.run_steps(3);
    let image = harness.render()?;
    let (width, height) = (image.width(), image.height());
    save(name, width, height, &image.into_raw())
}

#[test]
fn the_tags_option_is_drawn_in_the_record_export_setup_in_both_languages() -> TestResult {
    let flat = RecordForm::FlatCsv;
    for (language, tag, label) in [
        (
            Language::BritishEnglish,
            "en-GB",
            "Write the tag path next to every tag Reference (JSON and NDJSON only)",
        ),
        (
            Language::German,
            "de-DE",
            "Tag-Pfad neben jeder Tag-Referenz schreiben (nur JSON und NDJSON)",
        ),
    ] {
        let Some(renderer) = renderer() else {
            return Ok(());
        };
        let (_dir, mut model, _out) = opened(&forge())?;
        select_node(&mut model, GEAR_NODE)?;
        model.begin_record_export();
        let mut harness = window(renderer, [1100.0, 760.0], language, model);

        harness.get_by_role_and_label(Role::CheckBox, label).click();
        harness.run_steps(3);
        assert!(
            harness
                .state()
                .record_export_setup()
                .is_some_and(p4kx_gui::RecordExportSetup::tags),
            "{tag}: the checkbox switches the option on"
        );
        render(&mut harness, &format!("record-export-tags-{tag}.png"))?;

        harness
            .state_mut()
            .record_export_setup_mut()
            .ok_or("no setup")?
            .set_form(flat);
        render(&mut harness, &format!("record-export-tags-flat-{tag}.png"))?;
        let checkbox = harness.get_by_role_and_label(Role::CheckBox, label);
        assert!(
            checkbox.accesskit_node().is_disabled(),
            "{tag}: disabled for a flat form"
        );
        checkbox.click();
        harness.run_steps(3);
        assert!(
            harness
                .state()
                .record_export_setup()
                .is_some_and(p4kx_gui::RecordExportSetup::tags),
            "{tag}: a click on the disabled checkbox changes nothing"
        );
    }
    Ok(())
}

#[test]
fn the_tag_path_is_drawn_after_a_tag_reference_in_the_record_preview() -> TestResult {
    for (language, tag) in [
        (Language::BritishEnglish, "en-GB"),
        (Language::German, "de-DE"),
    ] {
        let Some(renderer) = renderer() else {
            return Ok(());
        };
        let (_dir, mut model, _out) = opened(&forge())?;
        select_node(&mut model, GEAR_NODE)?;
        model
            .browser_mut()
            .ok_or("no Archive")?
            .load_record_preview(|| {});
        wait_until(&mut model, |model| {
            model
                .browser()
                .and_then(Browser::record_preview)
                .is_some_and(|preview| matches!(preview.status(), RecordPreviewStatus::Ready(_)))
        })?;
        model.set_preview_tab(PreviewTab::Record);
        let mut harness = window(renderer, [1400.0, 800.0], language, model);

        render(&mut harness, &format!("record-preview-tag-{tag}.png"))?;
    }
    Ok(())
}
