// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Helpers of the tests that render the 3D viewer headless: a window on a real graphics device,
//! pixels of its picture and PNG files of it.
//!
//! The tests need a graphics adapter, a software one is enough, and return early without one.
//! `P4KX_RENDER_DIR` names a folder the rendered windows are written to as PNG files.

use crate::viewing;

use std::error::Error;
use std::fs::File;
use std::io::BufWriter;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use eframe::egui_wgpu::{RenderState, RendererOptions};
use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use egui_kittest::wgpu::{WgpuTestRenderer, create_render_state};
use p4kx_gui::{GpuReport, Language, Model, ViewerSupport, install_viewer};
use p4kx_testkit::wgpu_setup::software_wgpu_setup;
use viewing::{Site, TestResult, entries, opened_at};

pub(crate) const BACKGROUND: [u8; 3] = [0x0F, 0x0F, 0x0F];

pub(crate) fn render_state() -> Option<RenderState> {
    render_state_with(software_wgpu_setup())
}

#[expect(
    clippy::print_stderr,
    reason = "tells the test run why a test checked nothing"
)]
fn render_state_with(setup: eframe::egui_wgpu::WgpuSetup) -> Option<RenderState> {
    let state = catch_unwind(AssertUnwindSafe(|| {
        create_render_state(setup, RendererOptions::PREDICTABLE)
    }))
    .ok();
    if state.is_none() {
        eprintln!("no graphics adapter: the test checks nothing");
    }
    state
}

/// A render state whose device reads no storage buffer in a vertex stage, so it refuses the
/// pipelines that skin a model and draw its skeleton.
pub(crate) fn render_state_without_skinning() -> Option<RenderState> {
    let eframe::egui_wgpu::WgpuSetup::CreateNew(mut setup) = software_wgpu_setup() else {
        return None;
    };
    setup.device_descriptor = Arc::new(|adapter| eframe::egui_wgpu::wgpu::DeviceDescriptor {
        required_limits: eframe::egui_wgpu::wgpu::Limits {
            max_storage_buffers_per_shader_stage: 0,
            ..eframe::egui_wgpu::wgpu::Limits::default()
        }
        .using_resolution(adapter.limits()),
        ..eframe::egui_wgpu::wgpu::DeviceDescriptor::default()
    });
    render_state_with(eframe::egui_wgpu::WgpuSetup::CreateNew(setup))
}

pub(crate) struct Window {
    pub(crate) _folder: tempfile::TempDir,
    pub(crate) harness: Harness<'static, Model>,
    pub(crate) report: Arc<GpuReport>,
    pub(crate) state: RenderState,
}

pub(crate) fn window(language: Language) -> Result<Option<Window>, Box<dyn Error>> {
    let site = Site {
        entries: entries(),
        installation: false,
        key: false,
    };
    window_at(language, site)
}

pub(crate) fn window_at(language: Language, site: Site) -> Result<Option<Window>, Box<dyn Error>> {
    window_on(render_state(), language, site)
}

pub(crate) fn window_on(
    state: Option<RenderState>,
    language: Language,
    site: Site,
) -> Result<Option<Window>, Box<dyn Error>> {
    let Some(state) = state else {
        return Ok(None);
    };
    let report = install_viewer(&state)?;
    let renderer = WgpuTestRenderer::from_render_state(state.clone());
    let (folder, harness) = opened_at(
        language,
        ViewerSupport::Ready(Arc::clone(&report)),
        site,
        |builder| builder.renderer(renderer),
    )?;
    Ok(Some(Window {
        _folder: folder,
        harness,
        report,
        state,
    }))
}

pub(crate) fn pixel(image: &[u8], width: u32, column: u32, row: u32) -> [u8; 3] {
    let at = (row as usize * width as usize + column as usize) * 4;
    match image.get(at..at + 3) {
        Some(&[red, green, blue]) => [red, green, blue],
        _ => [0; 3],
    }
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the view lies inside the window, whose pixels are positive and few"
)]
pub(crate) fn pixels(rect: egui::Rect) -> [u32; 4] {
    [rect.left(), rect.top(), rect.right(), rect.bottom()].map(|edge| edge as u32)
}

pub(crate) fn save(name: &str, width: u32, height: u32, image: &[u8]) -> TestResult {
    let Some(folder) = std::env::var_os("P4KX_RENDER_DIR").map(PathBuf::from) else {
        return Ok(());
    };
    std::fs::create_dir_all(&folder)?;
    write_png(&folder.join(name), width, height, image)
}

fn write_png(path: &Path, width: u32, height: u32, image: &[u8]) -> TestResult {
    let mut encoder = png::Encoder::new(BufWriter::new(File::create(path)?), width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header()?.write_image_data(image)?;
    Ok(())
}

pub(crate) fn rendered(
    window: &mut Window,
    name: &str,
) -> Result<(u32, u32, Vec<u8>), Box<dyn Error>> {
    window.harness.run_steps(2);
    rendered_now(window, name)
}

/// Renders the frame the window drew last, without running another one.
pub(crate) fn rendered_now(
    window: &mut Window,
    name: &str,
) -> Result<(u32, u32, Vec<u8>), Box<dyn Error>> {
    let image = window.harness.render()?;
    let (width, height) = (image.width(), image.height());
    let data = image.into_raw();
    save(name, width, height, &data)?;
    Ok((width, height, data))
}

pub(crate) fn view_pixels(harness: &Harness<'_, Model>) -> [u32; 4] {
    pixels(harness.get_by_role(Role::Canvas).rect())
}
