// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The P4KX logo of `docs/design/p4kx-logo.svg` and `docs/design/p4kx-mark.svg`, rasterised from
//! its geometry: data brackets around an X on a 130 unit square, in theme colours only.

use egui::{Color32, ColorImage, IconData};
use tiny_skia::{
    Color, FillRule, LineCap, LineJoin, Paint, Path, PathBuilder, Pixmap, Stroke, Transform,
};

use super::palette;

/// The side of the square the geometry is drawn on, in logo units.
pub const VIEW_BOX: f32 = 130.0;
/// The corner radius of the tile, in logo units.
pub const TILE_RADIUS: f32 = 24.0;
/// The stroke width of the brackets, in logo units.
pub const BRACKET_WIDTH: f32 = 9.0;
/// The stroke width of the two strokes of the X, in logo units.
pub const CROSS_WIDTH: f32 = 11.0;
/// The side of the window and taskbar icon, in pixels.
pub const ICON_SIZE: u32 = 256;
/// The sides of the images in the executable icon, in pixels.
pub const ICO_SIZES: [u32; 6] = [16, 24, 32, 48, 64, 256];

/// The points of the left bracket, in logo units.
pub const LEFT_BRACKET: [(f32, f32); 4] =
    [(42.0, 30.0), (28.0, 30.0), (28.0, 100.0), (42.0, 100.0)];
/// The points of the right bracket, in logo units.
pub const RIGHT_BRACKET: [(f32, f32); 4] =
    [(88.0, 30.0), (102.0, 30.0), (102.0, 100.0), (88.0, 100.0)];
/// The ends of the back stroke of the X, in logo units.
pub const BACK_STROKE: [(f32, f32); 2] = [(50.0, 42.0), (80.0, 88.0)];
/// The ends of the front stroke of the X, in logo units.
pub const FRONT_STROKE: [(f32, f32); 2] = [(80.0, 42.0), (50.0, 88.0)];
const BEZIER_QUARTER: f32 = 0.552_284_8;

/// Which drawing of the logo to rasterise.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Variant {
    /// The drawing on a rounded canvas tile, the icon.
    Tile,
    /// The drawing alone on a transparent ground.
    Mark,
}

/// Returns the window and taskbar icon: the tile at [`ICON_SIZE`].
#[must_use]
pub fn window_icon() -> IconData {
    IconData {
        rgba: rgba(Variant::Tile, ICON_SIZE),
        width: ICON_SIZE,
        height: ICON_SIZE,
    }
}

/// Returns the `variant` as an egui image of `size` pixels.
#[must_use]
pub fn image(variant: Variant, size: u32) -> ColorImage {
    let pixels = rgba(variant, size);
    let edge = usize::try_from(size).unwrap_or(0);
    if pixels.len() != edge.saturating_mul(edge).saturating_mul(4) {
        return ColorImage::default();
    }
    ColorImage::from_rgba_unmultiplied([edge, edge], &pixels)
}

/// Returns the `variant` as unmultiplied RGBA rows of `size` pixels, or nothing when `size` is
/// zero or too large to allocate.
#[must_use]
pub fn rgba(variant: Variant, size: u32) -> Vec<u8> {
    let Some(mut pixmap) = Pixmap::new(size, size) else {
        return Vec::new();
    };
    let scale = to_f32(size) / VIEW_BOX;
    let transform = Transform::from_scale(scale, scale);
    if variant == Variant::Tile
        && let Some(tile) = tile()
    {
        pixmap.fill_path(
            &tile,
            &paint(palette::BRAND_TILE),
            FillRule::Winding,
            transform,
            None,
        );
    }
    let brackets = Stroke {
        width: BRACKET_WIDTH,
        line_join: LineJoin::Miter,
        ..Stroke::default()
    };
    let cross = Stroke {
        width: CROSS_WIDTH,
        line_cap: LineCap::Round,
        ..Stroke::default()
    };
    let strokes = [
        (&LEFT_BRACKET[..], palette::WHITE, &brackets),
        (&RIGHT_BRACKET[..], palette::WHITE, &brackets),
        (&BACK_STROKE[..], palette::ACCENT_DARK, &cross),
        (&FRONT_STROKE[..], palette::PRIMARY, &cross),
    ];
    for (points, colour, stroke) in strokes {
        if let Some(path) = polyline(points) {
            pixmap.stroke_path(&path, &paint(colour), stroke, transform, None);
        }
    }
    pixmap
        .pixels()
        .iter()
        .flat_map(|pixel| {
            let straight = pixel.demultiply();
            [
                straight.red(),
                straight.green(),
                straight.blue(),
                straight.alpha(),
            ]
        })
        .collect()
}

fn paint(colour: Color32) -> Paint<'static> {
    let mut paint = Paint {
        anti_alias: true,
        ..Paint::default()
    };
    paint.set_color(Color::from_rgba8(colour.r(), colour.g(), colour.b(), 255));
    paint
}

fn polyline(points: &[(f32, f32)]) -> Option<Path> {
    let (first, rest) = points.split_first()?;
    let mut builder = PathBuilder::new();
    builder.move_to(first.0, first.1);
    for point in rest {
        builder.line_to(point.0, point.1);
    }
    builder.finish()
}

fn tile() -> Option<Path> {
    let (side, radius) = (VIEW_BOX, TILE_RADIUS);
    let control = radius * (1.0 - BEZIER_QUARTER);
    let mut builder = PathBuilder::new();
    builder.move_to(radius, 0.0);
    builder.line_to(side - radius, 0.0);
    builder.cubic_to(side - control, 0.0, side, control, side, radius);
    builder.line_to(side, side - radius);
    builder.cubic_to(
        side,
        side - control,
        side - control,
        side,
        side - radius,
        side,
    );
    builder.line_to(radius, side);
    builder.cubic_to(control, side, 0.0, side - control, 0.0, side - radius);
    builder.line_to(0.0, radius);
    builder.cubic_to(0.0, control, control, 0.0, radius, 0.0);
    builder.close();
    builder.finish()
}

#[expect(
    clippy::cast_precision_loss,
    reason = "icon sizes are far below the f32 precision limit"
)]
const fn to_f32(value: u32) -> f32 {
    value as f32
}

#[cfg(test)]
mod tests {
    use super::{ICON_SIZE, Variant, rgba, window_icon};
    use crate::theme::palette;

    fn pixel(data: &[u8], size: u32, x: u32, y: u32) -> [u8; 4] {
        let at = usize::try_from((y * size + x) * 4).unwrap_or(0);
        data.get(at..at + 4)
            .and_then(|bytes| <[u8; 4]>::try_from(bytes).ok())
            .unwrap_or_default()
    }

    fn colour(c: egui::Color32) -> [u8; 4] {
        [c.r(), c.g(), c.b(), 255]
    }

    #[test]
    fn the_tile_is_canvas_with_transparent_corners_and_the_drawing_in_theme_colours() {
        let data = rgba(Variant::Tile, 130);

        assert_eq!(
            pixel(&data, 130, 65, 10),
            colour(palette::BRAND_TILE),
            "brand tile"
        );
        assert_eq!(pixel(&data, 130, 0, 0)[3], 0, "rounded corner");
        assert_eq!(
            pixel(&data, 130, 28, 65),
            colour(palette::WHITE),
            "left bracket"
        );
        assert_eq!(
            pixel(&data, 130, 102, 65),
            colour(palette::WHITE),
            "right bracket"
        );
        assert_eq!(
            pixel(&data, 130, 65, 65),
            colour(palette::PRIMARY),
            "front stroke on top"
        );
        assert_eq!(
            pixel(&data, 130, 54, 48),
            colour(palette::ACCENT_DARK),
            "back stroke"
        );
    }

    #[test]
    fn the_mark_has_no_tile() {
        let data = rgba(Variant::Mark, 130);

        assert_eq!(pixel(&data, 130, 65, 10)[3], 0, "transparent ground");
        assert_eq!(pixel(&data, 130, 28, 65), colour(palette::WHITE), "bracket");
    }

    #[test]
    fn the_window_icon_is_a_square_of_the_icon_size() {
        let icon = window_icon();

        assert_eq!((icon.width, icon.height), (ICON_SIZE, ICON_SIZE), "square");
        assert_eq!(icon.rgba.len(), 4 * 256 * 256, "rows");
    }

    #[test]
    fn a_zero_size_has_no_pixels() {
        assert!(rgba(Variant::Mark, 0).is_empty(), "nothing");
    }
}
