// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The windows of the Basetool catalog drawn by wgpu at the desktop size and at the smallest
//! window, in both languages, rendered headless by `egui_kittest`.
//!
//! The tests need a graphics adapter, a software one is enough, and return early without one
//! unless `P4KX_REQUIRE_RENDER_ADAPTER` is set, which CI does. `P4KX_RENDER_DIR` names a folder
//! the rendered windows are written to as PNG files.

use crate::cataloging;
use crate::waiting::wait_for;

use std::fs::File;
use std::io::BufWriter;
use std::panic::catch_unwind;
use std::path::PathBuf;

use crate::software_wgpu::software_wgpu_setup;
use cataloging::{TestResult, makers, opened};
use eframe::egui_wgpu::{RenderState, RendererOptions};
use egui::Rect;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use egui_kittest::wgpu::{WgpuTestRenderer, create_render_state};
use p4kx_export::CatalogPublicationMode;
use p4kx_gui::{
    CatalogExportOptions, CatalogExportStage, CatalogExportStatus, Language, Model, ui,
};

const SIZES: [[f32; 2]; 2] = [[1280.0, 800.0], [640.0, 400.0]];
const LANGUAGES: [(Language, &str); 2] =
    [(Language::BritishEnglish, "en"), (Language::German, "de")];

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
fn adapter() -> Option<RenderState> {
    let state =
        catch_unwind(|| create_render_state(software_wgpu_setup(), RendererOptions::PREDICTABLE))
            .ok();
    if state.is_none() {
        assert!(
            !p4kx_testkit::render_adapter_required(),
            "a graphics adapter is required"
        );
        eprintln!("no graphics adapter: the test checks nothing");
    }
    state
}

fn window(model: Model, size: [f32; 2]) -> Option<Harness<'static, Model>> {
    let state = adapter()?;
    let mut harness = Harness::builder()
        .with_context_setup(p4kx_gui::theme::apply)
        .with_size(size)
        .renderer(WgpuTestRenderer::from_render_state(state))
        .build_ui_state(
            |ui, model: &mut Model| {
                ui::show(ui, model);
            },
            model,
        );
    harness.run_steps(3);
    Some(harness)
}

fn shoot(
    harness: &mut Harness<'_, Model>,
    (scene, language, size): (&str, &str, [f32; 2]),
    foot: &str,
) -> TestResult {
    let screen = Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(size[0], size[1]));
    let button = harness.get_by_label(foot).rect();
    assert!(
        screen.contains_rect(button),
        "`{foot}` lies inside the {size:?} window, but is at {button:?}"
    );
    let image = harness.render()?;
    save(
        &format!("catalog-{scene}-{language}-{}x{}.png", size[0], size[1]),
        image.width(),
        image.height(),
        image.as_raw(),
    )
}

#[test]
fn the_summary_of_a_compressed_catalog_is_drawn_at_both_sizes_in_both_languages() -> TestResult {
    for (language, tag) in LANGUAGES {
        for size in SIZES {
            let (_dir, mut model, out) = opened(&makers())?;
            model.set_language(language);
            model.start_catalog(out.join("basetool-catalog.json.zst"), || {});
            let Some(mut harness) = window(model, size) else {
                return Ok(());
            };
            wait_for(&mut harness, "the catalog to end", |harness| {
                harness
                    .state()
                    .catalog()
                    .is_some_and(|run| !run.is_running())
            })?;
            harness.run_steps(3);

            let close = if tag == "en" { "Close" } else { "Schlie�en" };
            shoot(&mut harness, ("summary", tag, size), close)?;
            let sizes = if tag == "en" {
                " compressed (of"
            } else {
                " komprimiert (vom"
            };
            harness.get_by_label_contains(sizes).scroll_to_me();
            harness.run_steps(3);
            shoot(&mut harness, ("summary-sizes", tag, size), close)?;
        }
    }
    Ok(())
}

#[test]
fn the_progress_window_of_a_building_catalog_is_drawn_at_both_sizes_in_both_languages() -> TestResult
{
    for (language, tag) in LANGUAGES {
        for size in SIZES {
            let (_dir, mut model, out) = opened(&makers())?;
            model.set_language(language);
            let hold = model.hold_catalog();
            model.start_catalog(out.join("basetool-catalog.json.zst"), || {});
            let Some(mut harness) = window(model, size) else {
                return Ok(());
            };
            wait_for(&mut harness, "the catalog to build", |harness| {
                harness.state().catalog().is_some_and(|run| {
                    matches!(
                        run.status(),
                        CatalogExportStatus::Running(progress)
                            if progress.stage == CatalogExportStage::Building
                    )
                })
            })?;
            harness.run_steps(3);

            let cancel = if tag == "en" { "Cancel" } else { "Abbrechen" };
            shoot(&mut harness, ("running", tag, size), cancel)?;
            drop(hold);
        }
    }
    Ok(())
}

#[test]
fn the_warnings_of_the_game_data_are_drawn_at_both_sizes_in_both_languages() -> TestResult {
    for (language, tag) in LANGUAGES {
        for size in SIZES {
            let (_dir, mut model, out) = opened(&cataloging::ambiguous())?;
            model.set_language(language);
            model.start_catalog_with_options(
                out.join("basetool-catalog.diagnostic.json.zst"),
                CatalogExportOptions {
                    publication_mode: CatalogPublicationMode::DiagnosticPartial,
                    ..CatalogExportOptions::default()
                },
                || {},
            );
            let Some(mut harness) = window(model, size) else {
                return Ok(());
            };
            wait_for(&mut harness, "the catalog to end", |harness| {
                harness
                    .state()
                    .catalog()
                    .is_some_and(|run| !run.is_running())
            })?;
            harness.run_steps(3);

            let (sentence, close) = if tag == "en" {
                ("2 dismantle processes instead of one", "Close")
            } else {
                ("2 Zerlegungsprozesse statt einem", "Schlie�en")
            };
            harness.get_by_label_contains(sentence);
            shoot(&mut harness, ("warnings", tag, size), close)?;
        }
    }
    Ok(())
}

#[test]
fn normal_incomplete_publication_is_drawn_as_a_refusal_in_both_languages() -> TestResult {
    for (language, tag) in LANGUAGES {
        for size in SIZES {
            let (_dir, mut model, out) = opened(&cataloging::unreadable())?;
            model.set_language(language);
            model.start_catalog(out.join("catalog.json"), || {});
            let Some(mut harness) = window(model, size) else {
                return Ok(());
            };
            wait_for(&mut harness, "the catalog refusal", |harness| {
                harness
                    .state()
                    .catalog()
                    .is_some_and(|run| !run.is_running())
            })?;
            harness.run_steps(3);

            let (sentence, close) = if tag == "en" {
                ("No file was created or replaced.", "Close")
            } else {
                ("Es wurde keine Datei erstellt oder ersetzt.", "Schlie�en")
            };
            harness.get_by_label_contains(sentence);
            shoot(&mut harness, ("incomplete-refusal", tag, size), close)?;
        }
    }
    Ok(())
}

#[test]
fn the_warning_alert_of_a_catalog_near_the_limit_is_drawn_at_both_sizes() -> TestResult {
    for size in SIZES {
        let (_dir, mut model, out) = opened(&cataloging::near_upload_limit())?;
        model.start_catalog(out.join("basetool-catalog.json"), || {});
        let Some(mut harness) = window(model, size) else {
            return Ok(());
        };
        wait_for(&mut harness, "the catalog to end", |harness| {
            harness
                .state()
                .catalog()
                .is_some_and(|run| !run.is_running())
        })?;
        harness.run_steps(3);

        shoot(&mut harness, ("near-limit", "en", size), "Close")?;
    }
    Ok(())
}

#[test]
fn the_resource_kind_is_localized_and_visible_at_both_window_sizes() -> TestResult {
    for (language, tag) in LANGUAGES {
        for size in SIZES {
            let (_dir, mut model, out) = opened(&cataloging::resources())?;
            model.set_language(language);
            model.start_catalog(out.join("resources.json.zst"), || {});
            let Some(mut harness) = window(model, size) else {
                return Ok(());
            };
            wait_for(&mut harness, "the resource catalog to finish", |harness| {
                harness
                    .state()
                    .catalog()
                    .is_some_and(|run| !run.is_running())
            })?;
            let (line, close) = if tag == "en" {
                ("Resource types: 1 written, 0 skipped, complete", "Close")
            } else {
                (
                    "Ressourcentypen: 1 geschrieben, 0 �bersprungen, vollst�ndig",
                    "Schlie�en",
                )
            };
            harness.get_by_label(line).scroll_to_me();
            harness.run_steps(3);
            shoot(&mut harness, ("resources", tag, size), close)?;
        }
    }
    Ok(())
}
