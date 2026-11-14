// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The main tabs at the default window size, 1280 by 800, drawn by wgpu and rendered headless by
//! `egui_kittest`, with the zebra stripe of the tables and the fill of the comparison tiles
//! checked in the picture.
//!
//! The tests need a graphics adapter, a software one is enough, and return early without one.
//! `P4KX_RENDER_DIR` names a folder the rendered windows are written to as PNG files.

use crate::rendering::{pixel, pixels, render_state};
use crate::waiting::RunQuiet;
use crate::{comparing, entity_picking, entity_sounds, sounding, tagging};

use std::error::Error;
use std::fs::File;
use std::io::BufWriter;
use std::path::PathBuf;

use egui::Rect;
use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use egui_kittest::wgpu::WgpuTestRenderer;
use p4kx_channels::SearchPaths;
use p4kx_gui::{
    ChannelList, ComparisonSide, ComparisonStatus, Language, ListStatus, Model, SoundsListStatus,
    WorkTab, ui,
};
use p4kx_settings::ManualChannels;
use tempfile::TempDir;

type TestResult = Result<(), Box<dyn Error>>;

const SIZE: [f32; 2] = [1280.0, 800.0];
const STRIPE_ON_PANEL: [u8; 3] = [0x30, 0x35, 0x3A];
const TILE_FILL: [u8; 3] = [0x1A, 0x1D, 0x21];
const LANGUAGES: [(Language, &str); 2] = [
    (Language::BritishEnglish, "en-GB"),
    (Language::German, "de-DE"),
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

fn renderer() -> Option<WgpuTestRenderer> {
    render_state().map(WgpuTestRenderer::from_render_state)
}

struct Picture {
    width: u32,
    height: u32,
    data: Vec<u8>,
}

impl Picture {
    fn at(&self, column: u32, row: u32) -> [u8; 3] {
        pixel(&self.data, self.width, column, row)
    }

    /// Counts the pixels of the table stripe in the column just inside the left edge of `head`,
    /// below the head: the cell padding of the rows, where no text is drawn.
    fn stripe_rows(&self, head: Rect) -> usize {
        let [column, _, _, bottom] = pixels(head);
        (bottom..self.height)
            .filter(|&row| {
                self.at(column + 6, row)
                    .iter()
                    .zip(STRIPE_ON_PANEL)
                    .all(|(&got, want)| got.abs_diff(want) <= 2)
            })
            .count()
    }
}

fn picture<S>(harness: &mut Harness<'_, S>, name: &str) -> Result<Picture, Box<dyn Error>> {
    harness.run_steps(3);
    let image = harness.render()?;
    let (width, height) = (image.width(), image.height());
    let data = image.into_raw();
    save(name, width, height, &data)?;
    Ok(Picture {
        width,
        height,
        data,
    })
}

fn assert_striped(picture: &Picture, head: Rect, tab: &str) {
    let rows = picture.stripe_rows(head);
    assert!(
        rows >= 30,
        "{tab}: {rows} pixel rows of the stripe in the table"
    );
}

fn opened(
    language: Language,
    renderer: WgpuTestRenderer,
    tab: WorkTab,
) -> Result<(TempDir, Harness<'static, Model>), Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let path = tagging::archive(dir.path(), entity_picking::entries())?;
    let mut harness = Harness::builder()
        .with_context_setup(p4kx_gui::theme::apply)
        .with_size(SIZE)
        .renderer(renderer)
        .build_ui_state(
            |ui, model: &mut Model| {
                ui::show(ui, model);
            },
            Model::new(language),
        );
    harness.state_mut().open(path, || {});
    tagging::wait_until(&mut harness, |model| model.browser().is_some())?;
    harness.state_mut().set_work_tab(tab);
    harness.run_steps(2);
    Ok((dir, harness))
}

#[test]
fn the_browse_tab_stripes_its_rows_in_the_table_stripe() -> TestResult {
    for ((language, suffix), size) in LANGUAGES.into_iter().zip(["Size", "Größe"]) {
        let Some(renderer) = renderer() else {
            return Ok(());
        };
        let (_dir, mut harness) = opened(language, renderer, WorkTab::Archive)?;
        let picture = picture(&mut harness, &format!("browse-{suffix}.png"))?;
        let head = harness
            .get_all_by_label(size)
            .last()
            .ok_or("no size head")?
            .rect();
        assert_striped(&picture, head, &format!("browse {suffix}"));
    }
    Ok(())
}

#[test]
fn the_entities_tab_is_only_rendered() -> TestResult {
    for (language, suffix) in LANGUAGES {
        let Some(renderer) = renderer() else {
            return Ok(());
        };
        let (_dir, mut harness) = opened(language, renderer, WorkTab::Entities)?;
        tagging::wait_until(&mut harness, entity_picking::listed)?;
        entity_picking::select(&mut harness, "TEST_Ship")?;
        picture(&mut harness, &format!("entities-{suffix}.png"))?;
    }
    Ok(())
}

#[test]
fn the_sounds_tab_stripes_its_rows_in_the_table_stripe() -> TestResult {
    for (language, suffix) in LANGUAGES {
        let Some(renderer) = renderer() else {
            return Ok(());
        };
        let (_dir, mut harness) = entity_sounds::opened_with(
            language,
            SIZE,
            &sounding::FakeOutput::default(),
            Some(renderer),
        )?;
        harness
            .state_mut()
            .sounds_mut()
            .set_entity_text("TEST_Ship");
        harness.state_mut().sounds_mut().show_entity_text();
        entity_sounds::wait_until(&mut harness, |model| {
            matches!(model.sounds().list_status(), ListStatus::Ready(_))
        })?;
        harness.state_mut().sounds_mut().select(1);
        entity_sounds::wait_until(&mut harness, |model| {
            matches!(model.sounds().sounds_status(), SoundsListStatus::Ready(_))
        })?;
        harness.run_steps(3);
        let picture = picture(&mut harness, &format!("sounds-{suffix}.png"))?;
        let head = harness
            .get_by_role_and_label(Role::ColumnHeader, "Soundbank")
            .rect();
        assert_striped(&picture, head, &format!("sounds {suffix}"));
    }
    Ok(())
}

#[test]
fn the_comparison_tiles_take_the_bar_fill_inside_the_panel() -> TestResult {
    for (language, suffix, start, added) in [
        (Language::BritishEnglish, "en-GB", "Compare", "Added 1"),
        (Language::German, "de-DE", "Vergleichen", "Hinzugefügt 1"),
    ] {
        let Some(renderer) = renderer() else {
            return Ok(());
        };
        let pair = comparing::pair()?;
        let mut model = Model::new(language);
        model.set_channels(ChannelList::new(
            SearchPaths {
                default_library: Some(pair.dir.path().to_path_buf()),
                ..SearchPaths::default()
            },
            ManualChannels::default(),
        ));
        model.set_work_tab(WorkTab::Comparison);
        let mut harness = Harness::builder()
            .with_context_setup(p4kx_gui::theme::apply)
            .with_size(SIZE)
            .renderer(renderer)
            .build_ui_state(
                |ui, model: &mut Model| {
                    ui::show(ui, model);
                },
                model,
            );
        let tab = harness.state_mut().comparison_mut();
        tab.set_archive(ComparisonSide::Old, pair.old.clone());
        tab.set_archive(ComparisonSide::New, pair.new);
        harness.run_quiet();
        harness.get_by_label(start).click();
        tagging::wait_until(
            &mut harness,
            |model| matches!(model.comparison().status(), ComparisonStatus::Finished(view) if !view.is_filtering()),
        )?;
        let tile = harness
            .get_all_by_role_and_label(Role::Label, added)
            .next()
            .ok_or("no tile")?
            .rect();
        let picture = picture(&mut harness, &format!("comparison-{suffix}.png"))?;
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "the point is inside the window"
        )]
        let inside = picture.at((tile.right() - 8.0) as u32, tile.center().y as u32);
        assert_eq!(inside, TILE_FILL, "{suffix}: the fill of the tile");
    }
    Ok(())
}
