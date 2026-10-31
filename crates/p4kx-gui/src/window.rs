// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Placement of the main window and how it is remembered in the settings file.

use eframe::egui_wgpu::WgpuSetup;
use eframe::egui_wgpu::wgpu::Backends;
use egui::{Rect, ViewportBuilder, ViewportInfo};
use p4kx_settings::WindowGeometry;

use crate::backends;
use crate::theme::logo::window_icon;
use crate::theme::window_metrics::{WINDOW_MIN_HEIGHT, WINDOW_MIN_WIDTH};

const DEFAULT_SIZE: [f32; 2] = [1280.0, 800.0];
const MIN_SIZE: [f32; 2] = [WINDOW_MIN_WIDTH, WINDOW_MIN_HEIGHT];

/// Returns the main window for `title`, placed at `geometry` or at the default size.
#[must_use]
pub fn viewport(geometry: Option<WindowGeometry>, title: &str) -> ViewportBuilder {
    let builder = ViewportBuilder::default()
        .with_title(title)
        .with_icon(window_icon())
        .with_min_inner_size(MIN_SIZE)
        .with_drag_and_drop(true);
    let Some(geometry) = geometry else {
        return builder.with_inner_size(DEFAULT_SIZE);
    };
    builder
        .with_position([to_f32(geometry.x), to_f32(geometry.y)])
        .with_inner_size([
            to_f32(clamp_u32(geometry.width)),
            to_f32(clamp_u32(geometry.height)),
        ])
        .with_maximized(geometry.maximized)
}

/// Returns the options of the native window around `viewport`: egui's own pass keeps one sample
/// and no depth buffer, which is what the pipeline that blits the 3D view into it is made for.
///
/// The wgpu instance opens the backends `backends::choose` picks: `WGPU_BACKEND`, else Direct3D
/// 12 when it works, else eframe's default backends.
#[must_use]
pub fn native_options(viewport: ViewportBuilder) -> eframe::NativeOptions {
    let mut options = eframe::NativeOptions {
        viewport,
        multisampling: 0,
        depth_buffer: 0,
        ..eframe::NativeOptions::default()
    };
    if let WgpuSetup::CreateNew(setup) = &mut options.wgpu_options.wgpu_setup {
        let descriptor = &mut setup.instance_descriptor;
        descriptor.backends = backends::choose(
            Backends::from_env(),
            descriptor.backends,
            backends::dx12_usable,
        );
    }
    options
}

/// Returns the geometry to remember for the window described by `info`.
///
/// A minimised window keeps the previous geometry; a maximised one keeps the position and size it
/// had before, so it restores there.
#[must_use]
pub fn geometry(previous: Option<WindowGeometry>, info: &ViewportInfo) -> Option<WindowGeometry> {
    if info.minimized.unwrap_or(false) {
        return previous;
    }
    let maximized = info.maximized.unwrap_or(false);
    if let (true, Some(previous)) = (maximized, previous) {
        return Some(WindowGeometry {
            maximized,
            ..previous
        });
    }
    let (Some(outer), Some(inner)) = (info.outer_rect, info.inner_rect) else {
        return previous;
    };
    Some(from_rects(outer, inner, maximized))
}

fn from_rects(outer: Rect, inner: Rect, maximized: bool) -> WindowGeometry {
    WindowGeometry {
        x: to_i32(outer.min.x),
        y: to_i32(outer.min.y),
        width: to_i32(inner.width()).unsigned_abs(),
        height: to_i32(inner.height()).unsigned_abs(),
        maximized,
    }
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "window coordinates are far inside the i32 range and saturate otherwise"
)]
const fn to_i32(value: f32) -> i32 {
    value.round() as i32
}

#[expect(
    clippy::cast_precision_loss,
    reason = "window coordinates are far below the f32 precision limit"
)]
const fn to_f32(value: i32) -> f32 {
    value as f32
}

fn clamp_u32(value: u32) -> i32 {
    i32::try_from(value).unwrap_or(i32::MAX)
}

#[cfg(test)]
mod tests {
    use egui::{Rect, ViewportInfo, pos2, vec2};
    use p4kx_settings::WindowGeometry;

    use super::{geometry, native_options, viewport};

    #[test]
    fn egui_s_pass_has_one_sample_and_no_depth_buffer() {
        let options = native_options(viewport(None, "title"));

        assert_eq!(options.multisampling, 0, "no multisampling");
        assert_eq!(options.depth_buffer, 0, "no depth buffer");
    }

    #[test]
    fn the_window_carries_the_logo_as_its_icon() {
        let builder = viewport(None, "title");

        assert!(
            builder
                .icon
                .is_some_and(|icon| icon.width == 256 && icon.rgba.len() == 4 * 256 * 256),
            "icon"
        );
    }

    const SAVED: WindowGeometry = WindowGeometry {
        x: 100,
        y: 50,
        width: 1000,
        height: 700,
        maximized: false,
    };

    fn info(maximized: bool) -> ViewportInfo {
        ViewportInfo {
            outer_rect: Some(Rect::from_min_size(pos2(10.4, 20.6), vec2(816.0, 639.0))),
            inner_rect: Some(Rect::from_min_size(pos2(18.0, 51.0), vec2(800.0, 600.0))),
            maximized: Some(maximized),
            ..ViewportInfo::default()
        }
    }

    #[test]
    fn a_normal_window_keeps_its_outer_position_and_inner_size() {
        let geometry = geometry(Some(SAVED), &info(false));

        assert_eq!(
            geometry,
            Some(WindowGeometry {
                x: 10,
                y: 21,
                width: 800,
                height: 600,
                maximized: false,
            }),
            "outer position, inner size"
        );
    }

    #[test]
    fn a_maximised_window_keeps_the_size_it_restores_to() {
        let geometry = geometry(Some(SAVED), &info(true));

        assert_eq!(
            geometry,
            Some(WindowGeometry {
                maximized: true,
                ..SAVED
            }),
            "the previous geometry, maximised"
        );
    }

    #[test]
    fn a_minimised_window_keeps_the_previous_geometry() {
        let minimised = ViewportInfo {
            outer_rect: Some(Rect::from_min_size(
                pos2(-32000.0, -32000.0),
                vec2(160.0, 28.0),
            )),
            minimized: Some(true),
            ..info(false)
        };

        assert_eq!(geometry(Some(SAVED), &minimised), Some(SAVED), "unchanged");
    }

    #[test]
    fn a_window_maximised_without_previous_geometry_is_remembered_maximised() {
        let geometry = geometry(None, &info(true));

        assert_eq!(
            geometry.map(|geometry| geometry.maximized),
            Some(true),
            "maximised"
        );
    }

    #[test]
    fn the_viewport_restores_the_saved_geometry() {
        let builder = viewport(Some(SAVED), "P4KX");

        assert_eq!(builder.position, Some(pos2(100.0, 50.0)), "position");
        assert_eq!(builder.inner_size, Some(vec2(1000.0, 700.0)), "size");
        assert_eq!(builder.maximized, Some(false), "not maximised");
        assert_eq!(builder.title.as_deref(), Some("P4KX"), "title");
    }
}
