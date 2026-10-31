// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The Sounds tab drawn by wgpu, rendered headless by `egui_kittest`.
//!
//! The test needs a graphics adapter, a software one is enough, and returns early without one.
//! `P4KX_RENDER_DIR` names a folder the rendered windows are written to as PNG files.

use std::fs::File;
use std::io::BufWriter;
use std::panic::catch_unwind;
use std::path::PathBuf;

use eframe::egui_wgpu::RendererOptions;
use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use egui_kittest::wgpu::{WgpuTestRenderer, create_render_state};
use p4kx_gui::theme::window_metrics::{SIDEBAR_RAIL_WIDTH, WORK_MIN_WIDTH};
use p4kx_gui::{Language, ListStatus, Model, SoundsListStatus};
use p4kx_testkit::wgpu_setup::software_wgpu_setup;
use tempfile::TempDir;

use crate::entity_sounds::{TestResult, opened_with, wait_until};
use crate::sounding::FakeOutput;

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

pub(crate) fn rendered(
    language: Language,
    renderer: WgpuTestRenderer,
    size: [f32; 2],
) -> Result<(TempDir, Harness<'static, Model>), Box<dyn std::error::Error>> {
    let (dir, mut harness) = opened_with(language, size, &FakeOutput::default(), Some(renderer))?;
    harness
        .state_mut()
        .sounds_mut()
        .set_entity_text("TEST_Ship");
    harness.state_mut().sounds_mut().show_entity_text();
    wait_until(&mut harness, |model| {
        matches!(model.sounds().list_status(), ListStatus::Ready(_))
    })?;
    harness.state_mut().sounds_mut().select(1);
    wait_until(&mut harness, |model| {
        matches!(model.sounds().sounds_status(), SoundsListStatus::Ready(_))
    })?;
    harness.state_mut().select_sound_variant(1, || {});
    wait_until(&mut harness, |model| {
        model
            .sounds()
            .audio()
            .is_some_and(|audio| audio.player().is_some())
    })?;
    harness.run_steps(3);
    Ok((dir, harness))
}

#[test]
fn the_sounds_tab_is_drawn_with_triggers_variants_and_player() -> TestResult {
    for (language, name) in [
        (Language::BritishEnglish, "sounds-en-GB.png"),
        (Language::German, "sounds-de-DE.png"),
    ] {
        let Some(renderer) = renderer() else {
            return Ok(());
        };
        let (_dir, mut harness) = rendered(language, renderer, [1280.0, 760.0])?;
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
        assert!(orange > 500, "the active row bars are orange: {orange}");
    }
    Ok(())
}

#[test]
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "pixel coordinates of rectangles inside the image"
)]
fn the_column_dividers_of_the_trigger_table_end_above_the_list_footer() -> TestResult {
    let Some(renderer) = renderer() else {
        return Ok(());
    };
    let (_dir, mut harness) = rendered(Language::BritishEnglish, renderer, [1280.0, 760.0])?;
    let divider = harness
        .get_by_role_and_label(Role::ColumnHeader, "Wwise events")
        .rect()
        .left();
    let footer = harness
        .get_by_label("TEST_Ship: 3 audio triggers, 2 with a soundbank")
        .rect();
    let image = harness.render()?;
    let width = image.width() as usize;
    let data = image.into_raw();
    let pixel = |x: f32, y: f32| {
        let at = (y as usize * width + x as usize) * 4;
        data.get(at..at + 4).map(<[u8]>::to_vec)
    };
    let row = footer.center().y;
    let background = pixel(divider + 10.0, row);

    assert!(background.is_some(), "the footer is in the image");
    for offset in [-3.0, -2.0, -1.0, 0.0, 1.0] {
        assert_eq!(
            pixel(divider + offset, row),
            background,
            "no divider through the footer at {offset} from the column edge"
        );
    }
    Ok(())
}

#[test]
fn a_small_window_keeps_the_toolbar_apart_and_the_whole_tab_reachable() -> TestResult {
    for (language, name, show) in [
        (
            Language::BritishEnglish,
            "sounds-small-en-GB.png",
            "Show sounds",
        ),
        (
            Language::German,
            "sounds-small-de-DE.png",
            "Sounds anzeigen",
        ),
    ] {
        let Some(renderer) = renderer() else {
            return Ok(());
        };
        let (_dir, mut harness) = rendered(language, renderer, [640.0, 400.0])?;
        let field = harness
            .query_all_by_role(Role::TextInput)
            .next()
            .ok_or("no Entity field")?
            .rect();
        assert!(
            field.right() <= SIDEBAR_RAIL_WIDTH + WORK_MIN_WIDTH,
            "the Entity field {field:?} ends inside the scrolling work surface"
        );
        let head = harness
            .get_by_role_and_label(Role::ColumnHeader, "Trigger")
            .rect();
        let detail = harness.get_by_label_contains("Play_Multi: 3").rect();
        assert!(
            !head.intersects(detail) && head.bottom() <= detail.top(),
            "the trigger table {head:?} lies above the detail panel {detail:?}"
        );
        let button = harness.get_by_role_and_label(Role::Button, show).rect();
        assert!(
            !field.intersects(button),
            "the Entity field {field:?} and the button {button:?} do not overlap"
        );
        let image = harness.render()?;
        save(name, image.width(), image.height(), &image.into_raw())?;
    }
    Ok(())
}

#[test]
fn the_export_setup_keeps_its_foot_inside_a_small_window() -> TestResult {
    for (language, name, export) in [
        (
            Language::BritishEnglish,
            "sounds-export-small-en-GB.png",
            "Export",
        ),
        (
            Language::German,
            "sounds-export-small-de-DE.png",
            "Exportieren",
        ),
    ] {
        let Some(renderer) = renderer() else {
            return Ok(());
        };
        let (_dir, mut harness) = rendered(language, renderer, [640.0, 400.0])?;
        assert!(harness.state_mut().begin_sounds_export(), "setup opens");
        harness.run_steps(4);
        let foot = harness
            .query_all_by_role_and_label(Role::Button, export)
            .last()
            .ok_or("no foot")?
            .rect();
        assert!(
            foot.bottom() <= 400.0,
            "the foot {foot:?} is inside the window"
        );
        let image = harness.render()?;
        save(name, image.width(), image.height(), &image.into_raw())?;
    }
    Ok(())
}
