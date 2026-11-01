// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The Tags tab drawn by wgpu, rendered headless by `egui_kittest`.
//!
//! The test needs a graphics adapter, a software one is enough, and returns early without one.
//! `P4KX_RENDER_DIR` names a folder the rendered windows are written to as PNG files.

use crate::tagging;

use std::fs::File;
use std::io::BufWriter;
use std::panic::catch_unwind;
use std::path::PathBuf;

use crate::software_wgpu::software_wgpu_setup;
use eframe::egui_wgpu::RendererOptions;
use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use egui_kittest::wgpu::{WgpuTestRenderer, create_render_state};
use p4kx_gui::theme::tags_metrics::TAGS_NAME_MIN_WIDTH;
use p4kx_gui::{Language, Model, WorkTab, ui};
use p4kx_testkit::TestEntry;
use tagging::{TestResult, archive, forge, forge_many, wait_until};

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

fn window(
    renderer: WgpuTestRenderer,
    language: Language,
    dir: &std::path::Path,
    (size, forge): ([f32; 2], Vec<u8>),
) -> Result<Harness<'static, Model>, Box<dyn std::error::Error>> {
    let path = archive(dir, vec![TestEntry::zstd("Data\\Game2.dcb", forge)])?;
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
    harness.state_mut().set_work_tab(WorkTab::Tags);
    harness.run_steps(2);
    wait_until(&mut harness, tagging::read)?;
    Ok(harness)
}

#[test]
fn the_tag_tree_is_drawn_with_its_head_rows_and_chevrons() -> TestResult {
    for (language, name) in [
        (Language::BritishEnglish, "tags-en-GB.png"),
        (Language::German, "tags-de-DE.png"),
    ] {
        let Some(renderer) = renderer() else {
            return Ok(());
        };
        let dir = tempfile::tempdir()?;
        let mut harness = window(renderer, language, dir.path(), ([1100.0, 560.0], forge()))?;
        let expand = if language == Language::German {
            "Weapon aufklappen"
        } else {
            "Expand Weapon"
        };
        harness.get_by_role_and_label(Role::Button, expand).click();
        harness.run_steps(3);
        harness
            .get_by_role_and_label(Role::Button, "Weapon.Gun")
            .click();
        harness.run_steps(3);

        let image = harness.render()?;
        let (width, height) = (image.width(), image.height());
        let data = image.into_raw();
        save(name, width, height, &data)?;

        let orange = data
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|pixel| pixel.get(..3) == Some(&PRIMARY[..]))
            .count();
        assert!(
            orange > 1000,
            "the head rule and the selected row bar are orange: {orange} pixels"
        );
    }
    Ok(())
}

#[test]
fn the_tab_is_drawn_at_the_design_sizes_in_both_languages_with_and_without_a_search() -> TestResult
{
    for (language, tag) in [
        (Language::BritishEnglish, "en-GB"),
        (Language::German, "de-DE"),
    ] {
        for size in [[1280.0, 800.0], [640.0, 400.0]] {
            let Some(renderer) = renderer() else {
                return Ok(());
            };
            let dir = tempfile::tempdir()?;
            let mut harness = window(renderer, language, dir.path(), (size, forge()))?;
            let expand = if language == Language::German {
                "Weapon aufklappen"
            } else {
                "Expand Weapon"
            };
            harness.get_by_role_and_label(Role::Button, expand).click();
            harness.run_steps(3);
            harness
                .get_by_role_and_label(Role::Button, "Weapon.Gun")
                .click();
            harness.run_steps(3);
            for (searched, query) in [("", ""), ("-search", "gun")] {
                harness.state_mut().tags_mut().set_query(query);
                harness.run_steps(3);
                let image = harness.render()?;
                let name = format!("tags-{}x{}-{tag}{searched}.png", size[0], size[1]);
                let (width, height) = (image.width(), image.height());
                let data = image.into_raw();
                save(&name, width, height, &data)?;
                let orange = data
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .filter(|pixel| pixel.get(..3) == Some(&PRIMARY[..]))
                    .count();
                assert!(orange > 300, "{name}: the head rule is orange: {orange}");
            }
        }
    }
    Ok(())
}

#[test]
fn a_deep_tree_in_a_narrow_window_scrolls_sideways_instead_of_overlapping() -> TestResult {
    let Some(renderer) = renderer() else {
        return Ok(());
    };
    let dir = tempfile::tempdir()?;
    let mut harness = window(
        renderer,
        Language::BritishEnglish,
        dir.path(),
        ([640.0, 760.0], forge_many(100, 12)),
    )?;
    for position in 0..11 {
        harness.state_mut().tags_mut().toggle(position);
    }
    harness.run_steps(4);

    let image = harness.render()?;
    save(
        "tags-narrow-en-GB.png",
        image.width(),
        image.height(),
        &image.into_raw(),
    )?;
    let name = harness
        .get_by_role_and_label(
            Role::Button,
            "Tag0.Tag1.Tag2.Tag3.Tag4.Tag5.Tag6.Tag7.Tag8.Tag9.Tag10.Tag11",
        )
        .rect();
    assert!(
        name.width() >= TAGS_NAME_MIN_WIDTH - 0.5,
        "the tag column keeps its width: {name:?}"
    );
    Ok(())
}
