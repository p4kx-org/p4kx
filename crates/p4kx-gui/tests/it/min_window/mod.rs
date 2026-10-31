// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Every work tab at the smallest and at the default window size, in both languages, rendered
//! headless by `egui_kittest`: the main regions of the window and the controls of a tab never
//! overlap, and the sidebar of a narrow window hides behind its rail.
//!
//! The tests need a graphics adapter, a software one is enough, and return early without one.
//! `P4KX_RENDER_DIR` names a folder the rendered windows are written to as PNG files.

mod browse;
mod checks;
mod dialogs;
mod packages;
mod tabs;

use std::fs::File;
use std::io::BufWriter;
use std::panic::catch_unwind;
use std::path::PathBuf;

use eframe::egui_wgpu::RendererOptions;
use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use egui_kittest::wgpu::{WgpuTestRenderer, create_render_state};
use p4kx_gui::{
    Language, LocalizationStatus, Model, QueryStatus, TagsStatus, UiFilesStatus, WorkTab, ui,
};
use p4kx_testkit::wgpu_setup::software_wgpu_setup;
use p4kx_testkit::{GlobalIniBuilder, TestEntry};

use crate::entity_picking::{TestResult, entries, listed, select};
use crate::tagging::{archive, forge, wait_until};
use crate::waiting::{RunQuiet, wait_for_model};
use checks::problems;

const MIN: [f32; 2] = [640.0, 400.0];
const DEFAULT: [f32; 2] = [1280.0, 800.0];
const MID: [f32; 2] = [1040.0, 900.0];

const LANGUAGES: [(Language, &str, &str, &str); 2] = [
    (
        Language::BritishEnglish,
        "en-GB",
        "Show sidebar",
        "Hide sidebar",
    ),
    (
        Language::German,
        "de-DE",
        "Seitenleiste einblenden",
        "Seitenleiste ausblenden",
    ),
];

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

fn entity_entries() -> Vec<TestEntry> {
    let mut entries = entries();
    entries.push(TestEntry::zstd(
        "Data\\Localization\\english\\global.ini",
        GlobalIniBuilder::new()
            .text("vehicle_NameTEST_Ship", "Test Ship")
            .text("item_NameTEST_Gun_S2", "Test Gun")
            .build(),
    ));
    entries.push(TestEntry::deflated(
        "Data\\Scripts\\ship.xml",
        b"<Ship>\n  <Part name=\"TEST_Ship\"/>\n</Ship>\n".to_vec(),
    ));
    entries
}

fn fixture(tab: WorkTab) -> Vec<TestEntry> {
    match tab {
        WorkTab::Tags => vec![TestEntry::zstd("Data\\Game2.dcb", forge())],
        WorkTab::Characters => crate::characters::entries(),
        WorkTab::Ui => crate::ui_tabbing::entries(),
        _ => entity_entries(),
    }
}

type Window = (tempfile::TempDir, Harness<'static, Model>);

fn opened(
    tab: WorkTab,
    language: Language,
    size: [f32; 2],
    renderer: WgpuTestRenderer,
) -> Result<Window, Box<dyn std::error::Error>> {
    if tab == WorkTab::Sounds {
        return crate::entity_sounds_render::rendered(language, renderer, size);
    }
    let dir = tempfile::tempdir()?;
    let path = archive(dir.path(), fixture(tab))?;
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
    wait_until(&mut harness, |model| model.browser().is_some())?;
    harness.state_mut().set_work_tab(tab);
    harness.run_steps(2);
    loaded(&mut harness, tab)?;
    harness.run_quiet();
    Ok((dir, harness))
}

fn loaded(harness: &mut Harness<'_, Model>, tab: WorkTab) -> TestResult {
    match tab {
        WorkTab::Entities => {
            wait_until(harness, listed)?;
            select(harness, "TEST_Ship")?;
        }
        WorkTab::Query => {
            harness
                .state_mut()
                .query_mut()
                .set_text("from EntityClassDefinition");
            harness.state_mut().run_query(|| {});
            wait_for_model(harness, "the Query to finish", |model| {
                matches!(model.query().status(), QueryStatus::Finished(_))
            })?;
        }
        WorkTab::Localization => {
            wait_for_model(harness, "the localisation texts", |model| {
                matches!(model.localization().status(), LocalizationStatus::Ready(_))
            })?;
        }
        WorkTab::Tags => {
            wait_for_model(harness, "the tags", |model| {
                matches!(model.tags().status(), TagsStatus::Ready(_))
            })?;
        }
        WorkTab::Ui => {
            wait_for_model(harness, "the UI files", |model| {
                matches!(model.ui().files_status(), UiFilesStatus::Ready(_))
            })?;
        }
        WorkTab::Characters => {
            wait_for_model(harness, "the Characters tab", crate::characters::listed)?;
        }
        _ => {}
    }
    Ok(())
}

fn shot(harness: &mut Harness<'_, Model>, name: &str) -> TestResult {
    let image = harness.render()?;
    save(
        &format!("{name}.png"),
        image.width(),
        image.height(),
        &image.into_raw(),
    )
}

fn every_size(tab: WorkTab, tab_name: &str) -> TestResult {
    let mut found = Vec::new();
    for size in [MIN, DEFAULT] {
        for (language, suffix, show, _) in LANGUAGES {
            let Some(renderer) = renderer() else {
                return Ok(());
            };
            let (_dir, mut harness) = opened(tab, language, size, renderer)?;
            let name = format!("{tab_name}-{}x{}-{suffix}", size[0], size[1]);
            shot(&mut harness, &name)?;
            found.extend(problems(&harness, &name));
            if size == MIN {
                harness.get_by_role_and_label(Role::Button, show).click();
                harness.run_steps(4);
                let name = format!("{tab_name}-{}x{}-sidebar-{suffix}", size[0], size[1]);
                shot(&mut harness, &name)?;
                found.extend(problems(&harness, &name));
            }
        }
    }
    assert!(found.is_empty(), "{}", found.join("\n"));
    Ok(())
}
