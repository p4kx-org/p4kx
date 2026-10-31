// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The UI tab drawn by wgpu, rendered headless by `egui_kittest`.
//!
//! The test needs a graphics adapter, a software one is enough, and returns early without one.
//! `P4KX_RENDER_DIR` names a folder the rendered windows are written to as PNG files.

use std::fs::File;
use std::io::BufWriter;
use std::panic::catch_unwind;
use std::path::PathBuf;

use crate::software_wgpu::software_wgpu_setup;
use eframe::egui_wgpu::RendererOptions;
use egui_kittest::Harness;
use egui_kittest::wgpu::{WgpuTestRenderer, create_render_state};
use p4kx_gui::{Language, Model, UiDetail, UiMode, UiPower};

use crate::tagging::wait_until;
use crate::ui_tabbing::{TestResult, drawn, listed_with, pick_file, pick_ship};

const RED: [u8; 3] = [255, 0, 0];

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
        .filter(|pixel| pixel.get(..3) == Some(&RED[..]))
        .count())
}

#[test]
fn the_files_and_the_ships_are_drawn_with_their_pictures_in_both_languages() -> TestResult {
    drawn_at([1280.0, 800.0], "")
}

#[test]
fn the_tab_fits_the_smallest_window() -> TestResult {
    drawn_at([640.0, 400.0], "small-")
}

fn drawn_at(size: [f32; 2], tag: &str) -> TestResult {
    for (language, suffix) in [
        (Language::BritishEnglish, "en-GB"),
        (Language::German, "de-DE"),
    ] {
        let Some(renderer) = renderer() else {
            return Ok(());
        };
        let mut window = listed_with(language, size, Some(renderer))?;
        render(&mut window.harness, &format!("ui-empty-{tag}{suffix}.png"))?;
        pick_file(&mut window, "panel.gfx")?;
        let red = render(
            &mut window.harness,
            &format!("ui-files-contents-{tag}{suffix}.png"),
        )?;
        assert!(
            red > 0,
            "the work area is visible, the red half of the movie is on screen: {red}"
        );
        window
            .harness
            .state_mut()
            .ui_mut()
            .set_detail(UiDetail::Report);
        render(
            &mut window.harness,
            &format!("ui-files-report-{tag}{suffix}.png"),
        )?;

        window.harness.state_mut().ui_mut().set_mode(UiMode::Ships);
        pick_ship(&mut window)?;
        window
            .harness
            .state_mut()
            .ui_mut()
            .set_detail(UiDetail::Contents);
        render(
            &mut window.harness,
            &format!("ui-ships-contents-{tag}{suffix}.png"),
        )?;
        window
            .harness
            .state_mut()
            .ui_mut()
            .set_detail(UiDetail::Report);
        render(
            &mut window.harness,
            &format!("ui-ships-report-{tag}{suffix}.png"),
        )?;
        let tab = window.harness.state_mut().ui_mut();
        tab.set_power(UiPower::PoweredOn);
        tab.set_variables_text("isradar=1 bad");
        render(
            &mut window.harness,
            &format!("ui-ships-power-{tag}{suffix}.png"),
        )?;
        wait_until(&mut window.harness, drawn)?;
    }
    Ok(())
}
