// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The non-colour tokens of the P4KX design: type scale, weights, tracking, spacing, radii
//! and glows, named as in the token list, with 1rem as 16 points.

use egui::epaint::Shadow;
use egui::{Color32, CornerRadius, FontFamily, FontId};

/// One rem in points.
pub const REM: f32 = 16.0;

/// `fs-h1`, the page title.
pub const FS_H1: f32 = 2.0 * REM;
/// `fs-h2`, the section heading.
pub const FS_H2: f32 = 1.5 * REM;
/// `fs-h3`, the sub-section heading.
pub const FS_H3: f32 = 1.17 * REM;
/// `fs-body`, paragraphs and inputs.
pub const FS_BODY: f32 = REM;
/// `fs-sm`, table heads, nav links and small labels.
pub const FS_SM: f32 = 0.9 * REM;
/// `fs-xs`, button labels, the footer and chips.
pub const FS_XS: f32 = 0.8 * REM;
/// `fs-2xs`, chip labels and overlines.
pub const FS_2XS: f32 = 0.7 * REM;

/// `fw-thin`, Lato Light.
pub const FW_THIN: u16 = 300;
/// `fw-regular`, Lato Regular.
pub const FW_REGULAR: u16 = 400;
/// `fw-bold`, Lato Bold.
pub const FW_BOLD: u16 = 700;
/// `fw-black`, Lato Black.
pub const FW_BLACK: u16 = 900;

/// `tracking-display`, the headline tracking in em.
pub const TRACKING_DISPLAY: f32 = 0.05;
/// `tracking-overline`, the overline tracking in em.
pub const TRACKING_OVERLINE: f32 = 0.15;
/// `lh-body`, the body line height.
pub const LH_BODY: f32 = 1.5;

/// `space-1`.
pub const SPACE_1: f32 = 0.25 * REM;
/// `space-2`.
pub const SPACE_2: f32 = 0.5 * REM;
/// `space-3`.
pub const SPACE_3: f32 = 0.75 * REM;
/// `space-4`.
pub const SPACE_4: f32 = REM;
/// `space-6`.
pub const SPACE_6: f32 = 1.5 * REM;
/// `space-8`.
pub const SPACE_8: f32 = 2.0 * REM;
/// `touch-target`, the minimum height of a control.
pub const TOUCH_TARGET: f32 = 44.0;
/// `content-max`, the cap of a content column.
pub const CONTENT_MAX: f32 = 1200.0;

/// `radius-none`: table cells, accent bars and gauges stay square.
pub const RADIUS_NONE: f32 = 0.0;
/// `radius-sm`: inner elements such as chips and facts bars.
pub const RADIUS_SM: f32 = 4.0;
/// `radius-pill`: pill badges and the drag handle only.
pub const RADIUS_PILL: f32 = 999.0;

/// `radius-sm` as egui stores a corner, in whole points.
pub const RADIUS_SM_POINTS: u8 = 4;
/// The `radius-sm` corner as egui stores it.
pub const CORNER_SM: CornerRadius = CornerRadius::same(RADIUS_SM_POINTS);
/// The control radius, in points.
pub const RADIUS_CONTROL_POINTS: u8 = 6;
/// The tile radius, in points.
pub const RADIUS_TILE_POINTS: u8 = 8;
/// The panel and dialog radius, in points.
pub const RADIUS_BOX_POINTS: u8 = 10;
/// The `radius-control` corner as egui stores it.
pub const CORNER_CONTROL: CornerRadius = CornerRadius::same(RADIUS_CONTROL_POINTS);
/// The `radius-tile` corner as egui stores it.
pub const CORNER_TILE: CornerRadius = CornerRadius::same(RADIUS_TILE_POINTS);
/// The `radius-box` corner as egui stores it.
pub const CORNER_BOX: CornerRadius = CornerRadius::same(RADIUS_BOX_POINTS);
/// The `radius-none` corner as egui stores it.
pub const CORNER_NONE: CornerRadius = CornerRadius::ZERO;

/// `glow-primary`: the orange bloom of input focus and the filled CTA.
pub const GLOW_PRIMARY: Shadow = glow(
    0,
    5,
    Color32::from_rgba_unmultiplied_const(231, 126, 35, 77),
);
/// `glow-primary-lg`: the large orange bloom of modals and the hovered CTA.
pub const GLOW_PRIMARY_LG: Shadow = glow(
    0,
    20,
    Color32::from_rgba_unmultiplied_const(231, 126, 35, 51),
);
/// `glow-danger-lg`: the red bloom of the danger modal.
pub const GLOW_DANGER_LG: Shadow =
    glow(0, 20, Color32::from_rgba_unmultiplied_const(163, 0, 10, 51));
/// The orange glow of a tooltip, `0 0 10px rgb(231 126 35 / 30%)`.
pub const GLOW_TOOLTIP: Shadow = glow(
    0,
    10,
    Color32::from_rgba_unmultiplied_const(231, 126, 35, 77),
);
/// `shadow-drawer`: the separation of a drawer.
pub const SHADOW_DRAWER: Shadow = Shadow {
    offset: [5, 0],
    blur: 15,
    spread: 0,
    color: Color32::from_rgba_unmultiplied_const(0, 0, 0, 128),
};
/// `shadow-dropdown`: the separation of dropdowns and popovers.
pub const SHADOW_DROPDOWN: Shadow =
    glow(8, 16, Color32::from_rgba_unmultiplied_const(0, 0, 0, 204));

const fn glow(y: i8, blur: u8, color: Color32) -> Shadow {
    Shadow {
        offset: [0, y],
        blur,
        spread: 0,
        color,
    }
}

/// A weight of Lato the window ships.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Weight {
    /// `fw-thin`, for prose at 16 points or more.
    Light,
    /// `fw-regular`, for text below 16 points and inputs.
    Regular,
    /// `fw-bold`, for labels, buttons and headings.
    Bold,
}

impl Weight {
    /// Returns the CSS weight of the token.
    #[must_use]
    pub const fn value(self) -> u16 {
        match self {
            Self::Light => FW_THIN,
            Self::Regular => FW_REGULAR,
            Self::Bold => FW_BOLD,
        }
    }

    /// Returns the egui font family that sets this weight.
    #[must_use]
    pub fn family(self) -> FontFamily {
        match self {
            Self::Light => FontFamily::Name(super::LIGHT_FAMILY.into()),
            Self::Regular => FontFamily::Proportional,
            Self::Bold => FontFamily::Name(super::BOLD_FAMILY.into()),
        }
    }
}

/// Returns Lato at `size` points in `weight`.
#[must_use]
pub fn font(size: f32, weight: Weight) -> FontId {
    FontId::new(size, weight.family())
}
