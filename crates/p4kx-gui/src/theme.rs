// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The P4KX look: its tokens, Lato in three weights, the `radius-sm` corner and the desktop
//! metrics of `docs/design/desktop-spec.md`.

pub mod assembly_metrics;
mod brand_metrics;
pub mod categorical;
pub mod character_metrics;
pub mod desktop;
pub mod logo;
pub mod model_metrics;
pub mod palette;
mod query_metrics;
mod row_tints;
pub mod sound_metrics;
pub mod tags_metrics;
pub mod tokens;
mod tooltip_metrics;
pub mod ui_metrics;
pub mod window_metrics;

#[cfg(test)]
mod assembly_tests;
#[cfg(test)]
mod audio_tests;
#[cfg(test)]
mod character_tests;
#[cfg(test)]
mod comparison_tests;
#[cfg(test)]
mod image_tests;
#[cfg(test)]
mod localization_tests;
#[cfg(test)]
mod model_tests;
#[cfg(test)]
mod query_tests;
#[cfg(test)]
mod sound_columns_tests;
#[cfg(test)]
mod tags_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod token_tests;
#[cfg(test)]
mod ui_tests;
#[cfg(test)]
mod xml_tests;

use std::sync::Arc;

use egui::style::{ScrollStyle, Selection, Spacing, TextCursorStyle, WidgetVisuals, Widgets};
use egui::{
    Color32, Context, CornerRadius, FontData, FontDefinitions, FontFamily, Margin, Stroke, Style,
    TextStyle, Theme, Visuals, vec2,
};

use self::desktop as d;
use self::tokens::{SPACE_1, SPACE_2, Weight, font};

/// The font family name for Lato Bold, used by labels, buttons and headings.
pub const BOLD_FAMILY: &str = "lato-bold";
/// The font family name for Lato Light, used by prose at 16 points or more.
pub const LIGHT_FAMILY: &str = "lato-light";

/// The licence of the fonts the window ships, as shown under Help.
pub const FONT_LICENCE: &str = include_str!("../assets/fonts/OFL.txt");

const LATO_LIGHT: &str = "Lato-Light";
const LATO_REGULAR: &str = "Lato-Regular";
const LATO_BOLD: &str = "Lato-Bold";
const TOOLTIP_DELAY: f32 = 0.5;

/// Sets the fonts and style on `ctx` and keeps it dark whatever the system theme.
pub fn apply(ctx: &Context) {
    ctx.set_fonts(fonts());
    ctx.set_theme(Theme::Dark);
    ctx.set_style_of(Theme::Dark, style());
}

/// Returns Lato Regular as the first font of the proportional and monospace families, Lato Bold
/// as [`BOLD_FAMILY`] and Lato Light as [`LIGHT_FAMILY`], with egui's fonts behind them for
/// glyphs Lato lacks.
#[must_use]
pub fn fonts() -> FontDefinitions {
    let mut fonts = FontDefinitions::default();
    for (name, bytes) in [
        (
            LATO_LIGHT,
            include_bytes!("../assets/fonts/Lato-Light.ttf").as_slice(),
        ),
        (
            LATO_REGULAR,
            include_bytes!("../assets/fonts/Lato-Regular.ttf"),
        ),
        (LATO_BOLD, include_bytes!("../assets/fonts/Lato-Bold.ttf")),
    ] {
        fonts
            .font_data
            .insert(name.to_owned(), Arc::new(FontData::from_static(bytes)));
    }
    let fallbacks = fonts
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();
    for family in [FontFamily::Proportional, FontFamily::Monospace] {
        fonts
            .families
            .entry(family)
            .or_default()
            .insert(0, LATO_REGULAR.to_owned());
    }
    for (family, first) in [(BOLD_FAMILY, LATO_BOLD), (LIGHT_FAMILY, LATO_LIGHT)] {
        let names = std::iter::once(first.to_owned())
            .chain(fallbacks.iter().cloned())
            .collect();
        fonts
            .families
            .insert(FontFamily::Name(family.into()), names);
    }
    fonts
}

/// Returns the egui style of the P4KX design.
#[must_use]
pub fn style() -> Style {
    let mut style = Style {
        visuals: visuals(),
        spacing: spacing(),
        ..Style::default()
    };
    style.text_styles = [
        (TextStyle::Small, font(tokens::FS_XS, Weight::Regular)),
        (TextStyle::Body, font(tokens::FS_SM, Weight::Regular)),
        (TextStyle::Button, font(tokens::FS_XS, Weight::Bold)),
        (TextStyle::Monospace, font(tokens::FS_XS, Weight::Regular)),
        (TextStyle::Heading, font(d::TITLE_SIZE, Weight::Bold)),
    ]
    .into();
    style.interaction.tooltip_delay = TOOLTIP_DELAY;
    style
}

fn spacing() -> Spacing {
    Spacing {
        item_spacing: vec2(SPACE_2, SPACE_1),
        button_padding: vec2(d::MENU_LINK_PADDING_X, SPACE_1),
        menu_margin: Margin::from(SPACE_1),
        interact_size: vec2(d::ROW_HEIGHT, d::ROW_HEIGHT),
        icon_width: d::TOGGLE_SIZE,
        icon_width_inner: d::RADIO_DOT,
        icon_spacing: d::TOGGLE_GAP,
        menu_width: d::DROPDOWN_MIN_WIDTH,
        tooltip_width: d::TOOLTIP_MAX_WIDTH,
        scroll: ScrollStyle {
            floating: false,
            bar_width: d::SCROLLBAR,
            handle_min_length: d::SCROLL_THUMB_MIN,
            bar_inner_margin: 0.0,
            bar_outer_margin: 0.0,
            foreground_color: false,
            ..ScrollStyle::solid()
        },
        ..Spacing::default()
    }
}

fn visuals() -> Visuals {
    Visuals {
        dark_mode: true,
        override_text_color: None,
        weak_text_color: Some(palette::GRAY_2_TEXT),
        widgets: widgets(),
        selection: Selection {
            bg_fill: palette::PRIMARY,
            stroke: Stroke::new(1.0, palette::PRIMARY),
        },
        hyperlink_color: palette::PRIMARY,
        faint_bg_color: d::ZEBRA,
        extreme_bg_color: palette::GRAY_4,
        text_edit_bg_color: Some(palette::CONTROL_BG),
        code_bg_color: palette::SURFACE_INPUT,
        warn_fg_color: palette::WARNING,
        error_fg_color: palette::DANGER_TEXT,
        window_corner_radius: tokens::CORNER_BOX,
        window_shadow: tokens::GLOW_PRIMARY_LG,
        window_fill: palette::GRAY_4,
        window_stroke: Stroke::new(1.0, palette::PRIMARY),
        menu_corner_radius: tokens::CORNER_TILE,
        panel_fill: palette::GRAY_4,
        popup_shadow: tokens::SHADOW_DROPDOWN,
        text_cursor: TextCursorStyle {
            stroke: Stroke::new(d::FOCUS_WIDTH, palette::PRIMARY),
            ..TextCursorStyle::default()
        },
        ..Visuals::dark()
    }
}

/// The egui widget states; `bg_fill` carries the scrollbar thumb, `weak_bg_fill` the control fill.
///
/// The corner stays square because egui rounds the scrollbar with it; the drawn controls and the
/// text fields set the control radius themselves.
fn widgets() -> Widgets {
    let widget = |thumb: Color32, fill: Color32, border: Color32, text: Color32| WidgetVisuals {
        bg_fill: thumb,
        weak_bg_fill: fill,
        bg_stroke: Stroke::new(1.0, border),
        corner_radius: CornerRadius::ZERO,
        fg_stroke: Stroke::new(1.0, text),
        expansion: 0.0,
    };
    Widgets {
        noninteractive: widget(
            palette::GRAY_4,
            palette::GRAY_4,
            palette::GRAY_3,
            palette::GRAY_1,
        ),
        inactive: widget(
            palette::GRAY_2,
            palette::CONTROL_BG,
            palette::CONTROL_BORDER,
            palette::CONTROL_FG,
        ),
        hovered: widget(
            palette::ACCENT_LIGHT,
            palette::GRAY_3,
            palette::PRIMARY,
            palette::PRIMARY,
        ),
        active: widget(
            palette::PRIMARY,
            palette::CONTROL_BG_HOVER,
            palette::PRIMARY,
            palette::PRIMARY,
        ),
        open: widget(
            palette::GRAY_2,
            palette::CONTROL_SELECTED_BG,
            palette::CONTROL_SELECTED_BORDER,
            palette::CONTROL_SELECTED_FG,
        ),
    }
}
