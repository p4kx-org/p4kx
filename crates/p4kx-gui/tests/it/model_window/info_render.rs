// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The Info tab, the Export as GLB setup and its summary drawn by wgpu, rendered headless by
//! `egui_kittest`.
//!
//! The tests need a graphics adapter, a software one is enough, and return early without one.
//! `P4KX_RENDER_DIR` names a folder the rendered windows are written to as PNG files.

use crate::waiting::RunQuiet;
use std::fs::File;
use std::io::BufWriter;
use std::panic::catch_unwind;
use std::path::PathBuf;

use crate::software_wgpu::software_wgpu_setup;
use crate::viewing::{Site, TestResult, opened_at, opened_with, select, wait_until};
use eframe::egui_wgpu::RendererOptions;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use egui_kittest::wgpu::{WgpuTestRenderer, create_render_state};
use p4kx_gui::{Language, Model, Unavailable, ViewerSupport};
use p4kx_settings::ModelMaterials;
use p4kx_testkit::{TestEntry, TestModel, TestMtl, TestMtlMaterial};

#[expect(
    clippy::print_stderr,
    reason = "tells the test run why a test checked nothing"
)]
pub(super) fn renderer() -> Option<WgpuTestRenderer> {
    let state =
        catch_unwind(|| create_render_state(software_wgpu_setup(), RendererOptions::PREDICTABLE))
            .ok();
    if state.is_none() {
        eprintln!("no graphics adapter: the test checks nothing");
    }
    state.map(WgpuTestRenderer::from_render_state)
}

pub(super) fn save(name: &str, harness: &mut Harness<'_, Model>) -> TestResult {
    harness.run_steps(2);
    let image = harness.render()?;
    let (width, height) = (image.width(), image.height());
    let lit = image
        .pixels()
        .filter(|pixel| pixel.0.iter().take(3).any(|&channel| channel > 0x30))
        .count();
    assert!(lit > 1000, "{name}: the window has content");
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
    encoder
        .write_header()?
        .write_image_data(image.into_raw().as_slice())?;
    Ok(())
}

#[test]
fn the_info_tab_and_the_export_windows_render_in_both_languages() -> TestResult {
    for (language, suffix, info, export) in [
        (Language::BritishEnglish, "en-GB", "Info", "Export as GLB…"),
        (Language::German, "de-DE", "Info", "Als GLB exportieren …"),
    ] {
        let Some(renderer) = renderer() else {
            return Ok(());
        };
        let support = ViewerSupport::Unavailable(Unavailable::NoDevice);
        let (folder, mut harness) =
            opened_at(language, support, crate::info_models::site(), |builder| {
                builder.renderer(renderer)
            })?;
        select(&mut harness, "extra.cgf")?;

        harness.get_by_label(info).click();
        harness.run_quiet();
        save(&format!("model-info-{suffix}.png"), &mut harness)?;

        harness.get_by_label(export).click();
        harness.run_quiet();
        save(&format!("model-export-setup-{suffix}.png"), &mut harness)?;

        let setup = harness
            .state_mut()
            .model_export_setup_mut()
            .ok_or("no setup")?;
        setup.set_file(folder.path().join("extra.glb"));
        harness.run_quiet();
        save(&format!("model-export-ready-{suffix}.png"), &mut harness)?;
        assert!(harness.state_mut().submit_model_export(|| {}), "started");
        wait_until(&mut harness, |model| {
            model.model_export().is_some_and(|run| !run.is_running())
        })?;
        save(&format!("model-export-summary-{suffix}.png"), &mut harness)?;
        harness.state_mut().close_model_export();
    }
    Ok(())
}

#[test]
fn the_info_of_a_plain_model_renders_without_the_chunk_table() -> TestResult {
    let Some(renderer) = renderer() else {
        return Ok(());
    };
    let support = ViewerSupport::Unavailable(Unavailable::NoDevice);
    let (_folder, mut harness) = opened_with(Language::BritishEnglish, support, |builder| {
        builder.renderer(renderer)
    })?;
    select(&mut harness, "helpers.cgf")?;

    harness.get_by_label("Info").click();
    harness.run_quiet();

    save("model-info-helpers-en-GB.png", &mut harness)
}

#[test]
fn the_animation_choice_and_its_summary_render_in_both_languages() -> TestResult {
    for (language, suffix, export) in [
        (Language::BritishEnglish, "en-GB", "Export as GLB…"),
        (Language::German, "de-DE", "Als GLB exportieren …"),
    ] {
        let Some(renderer) = renderer() else {
            return Ok(());
        };
        let support = ViewerSupport::Unavailable(Unavailable::NoDevice);
        let (folder, mut harness) =
            opened_at(language, support, super::animations::site(), |builder| {
                builder.renderer(renderer)
            })?;
        select(&mut harness, "door.cga")?;

        harness.get_by_label("Info").click();
        harness.run_quiet();
        save(&format!("model-info-animations-{suffix}.png"), &mut harness)?;

        harness.get_by_label(export).click();
        harness.run_quiet();
        let setup = harness
            .state_mut()
            .model_export_setup_mut()
            .ok_or("no setup")?;
        setup.set_animations(true);
        setup.set_file(folder.path().join("door.glb"));
        harness.run_quiet();
        save(
            &format!("model-export-animations-{suffix}.png"),
            &mut harness,
        )?;

        assert!(harness.state_mut().submit_model_export(|| {}), "started");
        wait_until(&mut harness, |model| {
            model.model_export().is_some_and(|run| !run.is_running())
        })?;
        save(
            &format!("model-export-animations-summary-{suffix}.png"),
            &mut harness,
        )?;
        harness.state_mut().close_model_export();
    }
    Ok(())
}

const LONG_LIBRARY: &str = "objects/spaceships/ships/aegis/gladius/interior/materials/aegis_gladius_interior_cockpit_panels_and_displays_variant_a";

pub(super) const fn rows_label(language: Language) -> &'static str {
    match language {
        Language::German => "Knoten",
        Language::BritishEnglish => "Nodes",
    }
}

fn textured_site() -> Site {
    let library = format!("Data\\{}.mtl", LONG_LIBRARY.replace('/', "\\"));
    let model = TestModel::cube().with_material_library(LONG_LIBRARY);
    let materials = TestMtl::multi(&[
        TestMtlMaterial::new("hull").texture("TexSlot1", "textures/hull/hull_panel_diff.tif"),
        TestMtlMaterial::new("glass").texture("TexSlot1", "textures/glass/glass_diff.tif"),
    ]);
    Site {
        entries: vec![
            TestEntry::zstd("Data\\Objects\\textured.cgf", model.cgf()),
            TestEntry::zstd("Data\\Objects\\textured.cgfm", model.cgfm()),
            TestEntry::zstd(library, materials.cryxml()),
            TestEntry::stored("Data\\readme.txt", "plain text"),
        ],
        installation: false,
        key: false,
    }
}

#[test]
fn the_texture_choices_and_the_missing_textures_render_in_both_languages() -> TestResult {
    for (language, suffix, info, export) in [
        (Language::BritishEnglish, "en-GB", "Info", "Export as GLB…"),
        (Language::German, "de-DE", "Info", "Als GLB exportieren …"),
    ] {
        let Some(renderer) = renderer() else {
            return Ok(());
        };
        let support = ViewerSupport::Unavailable(Unavailable::NoDevice);
        let (folder, mut harness) = opened_at(language, support, textured_site(), |builder| {
            builder.renderer(renderer)
        })?;
        harness.set_size(egui::vec2(900.0, 900.0));
        harness.run_quiet();
        select(&mut harness, "textured.cgf")?;

        harness.get_by_label(info).click();
        harness.run_quiet();
        harness.get_by_label_contains(rows_label(language)).hover();
        harness.event(egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Line,
            delta: egui::vec2(0.0, -4.0),
            phase: egui::TouchPhase::Move,
            source: egui::MouseWheelSource::Wheel,
            modifiers: egui::Modifiers::NONE,
        });
        harness.run_quiet();
        save(&format!("model-info-long-{suffix}.png"), &mut harness)?;
        harness.set_size(egui::vec2(1600.0, 900.0));
        harness.run_quiet();

        harness.get_by_label(export).click();
        harness.run_quiet();
        let setup = harness
            .state_mut()
            .model_export_setup_mut()
            .ok_or("no setup")?;
        setup.set_materials(ModelMaterials::Textures);
        setup.set_texture_mip(1);
        setup.set_file(folder.path().join("textured.glb"));
        harness.run_quiet();
        save(&format!("model-export-textures-{suffix}.png"), &mut harness)?;

        assert!(harness.state_mut().submit_model_export(|| {}), "started");
        wait_until(&mut harness, |model| {
            model.model_export().is_some_and(|run| !run.is_running())
        })?;
        save(
            &format!("model-export-missing-textures-{suffix}.png"),
            &mut harness,
        )?;
        harness.state_mut().close_model_export();
    }
    Ok(())
}
