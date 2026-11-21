// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use egui::epaint::Shadow;
use egui::{
    Align2, Color32, Rect, Response, Role, Sense, Stroke, StrokeKind, TextStyle, TextWrapMode, Ui,
    WidgetInfo, WidgetText, vec2,
};

use super::focus::focus_ring;
use super::text::caps;
use crate::theme::desktop::{
    ALERT_DANGER_FILL, BUTTON_HEIGHT, BUTTON_PADDING_X, BUTTON_TRACKING, BUTTON_XS_HEIGHT,
    BUTTON_XS_PADDING_X, BUTTON_XS_SIZE, DISABLED_OPACITY, FOCUS_OFFSET, FOCUS_WIDTH, GHOST_HOVER,
    OUTLINE_HOVER, QUIET_DANGER_HOVER, TRANSITION,
};
use crate::theme::palette;
use crate::theme::tokens::{CORNER_CONTROL, FS_XS, GLOW_PRIMARY, GLOW_PRIMARY_LG, Weight};

/// A rung of the theme's action hierarchy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    /// `.btn--cta`, the one filled primary action of a context.
    Cta,
    /// `.btn-outline`, the emphasised secondary action.
    Outline,
    /// `.btn-ghost`, a routine action.
    Ghost,
    /// `.btn-quiet-danger`, a destructive action that only turns red on hover.
    QuietDanger,
    /// `.btn-danger`, the filled red action that confirms a destructive step in a danger modal.
    Danger,
}

/// The size of a button.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Size {
    /// `.btn`, 44 points high.
    Normal,
    /// `.btn-xs`, 32 points high.
    Compact,
}

/// The pointer state a button is painted in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Phase {
    Rest,
    Hover,
    Pressed,
}

/// The colours of a button in one phase.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Look {
    pub(crate) fill: Color32,
    pub(crate) tint: Color32,
    pub(crate) border: Color32,
    pub(crate) text: Color32,
    pub(crate) glow: Option<Shadow>,
}

/// Returns the colours of `kind` in `phase`, after the desktop spec.
pub(crate) const fn look(kind: Kind, phase: Phase) -> Look {
    let none = Color32::TRANSPARENT;
    match (kind, phase) {
        (Kind::Cta, phase) => cta(phase),
        (Kind::Outline, Phase::Rest) => Look {
            fill: none,
            tint: none,
            border: palette::PRIMARY,
            text: palette::PRIMARY,
            glow: None,
        },
        (Kind::Outline, Phase::Hover) => Look {
            fill: none,
            tint: OUTLINE_HOVER,
            border: palette::ACCENT_LIGHT,
            text: palette::ACCENT_LIGHT,
            glow: None,
        },
        (Kind::Ghost, Phase::Rest) => Look {
            fill: palette::CONTROL_BG,
            tint: none,
            border: palette::CONTROL_BORDER,
            text: palette::CONTROL_FG,
            glow: None,
        },
        (Kind::Ghost, Phase::Hover) => Look {
            fill: none,
            tint: GHOST_HOVER,
            border: palette::PRIMARY,
            text: palette::ACCENT_LIGHT,
            glow: None,
        },
        (Kind::QuietDanger, phase) => quiet_danger(phase),
        (Kind::Danger, phase) => danger(phase),
        (Kind::Outline | Kind::Ghost, Phase::Pressed) => Look {
            fill: palette::CONTROL_BG,
            tint: palette::CONTROL_SELECTED_BG,
            border: palette::PRIMARY,
            text: palette::CONTROL_SELECTED_FG,
            glow: None,
        },
    }
}

/// Returns the colours of a chosen button: the selected background, border and text of the
/// controls.
const fn chosen_look() -> Look {
    Look {
        fill: palette::CONTROL_SELECTED_BG,
        tint: Color32::TRANSPARENT,
        border: palette::CONTROL_SELECTED_BORDER,
        text: palette::CONTROL_SELECTED_FG,
        glow: None,
    }
}

/// Returns the colours of `.btn-cta` in `phase`.
const fn cta(phase: Phase) -> Look {
    let (fill, glow) = match phase {
        Phase::Rest => (palette::PRIMARY, Some(GLOW_PRIMARY)),
        Phase::Hover => (palette::ACCENT_LIGHT, Some(GLOW_PRIMARY_LG)),
        Phase::Pressed => (palette::ACCENT_DARK, None),
    };
    Look {
        fill,
        tint: Color32::TRANSPARENT,
        border: Color32::TRANSPARENT,
        text: palette::BLACK,
        glow,
    }
}

/// Returns the colours of `.btn-danger` in `phase`: `color-danger` with white text, the hover fill
/// while hovered or pressed.
const fn danger(phase: Phase) -> Look {
    let fill = match phase {
        Phase::Rest => palette::DANGER,
        Phase::Hover | Phase::Pressed => palette::DANGER_HOVER,
    };
    Look {
        fill,
        tint: Color32::TRANSPARENT,
        border: fill,
        text: palette::WHITE,
        glow: None,
    }
}

/// Returns the colours of `.btn-quiet-danger` in `phase`.
const fn quiet_danger(phase: Phase) -> Look {
    let none = Color32::TRANSPARENT;
    match phase {
        Phase::Rest => Look {
            fill: palette::CONTROL_BG,
            tint: none,
            border: palette::CONTROL_BORDER,
            text: palette::GRAY_2_TEXT,
            glow: None,
        },
        Phase::Hover => Look {
            fill: palette::CONTROL_BG,
            tint: QUIET_DANGER_HOVER,
            border: palette::DANGER,
            text: palette::DANGER_TEXT,
            glow: None,
        },
        Phase::Pressed => Look {
            fill: palette::CONTROL_BG,
            tint: ALERT_DANGER_FILL,
            border: palette::DANGER,
            text: palette::DANGER_TEXT,
            glow: None,
        },
    }
}

impl Look {
    fn blend(self, to: Self, t: f32) -> Self {
        let mix = |a: Color32, b: Color32| a.lerp_to_gamma(b, t);
        Self {
            fill: mix(self.fill, to.fill),
            tint: mix(self.tint, to.tint),
            border: mix(self.border, to.border),
            text: mix(self.text, to.text),
            glow: if t < 0.5 { self.glow } else { to.glow },
        }
    }

    fn dimmed(self) -> Self {
        let dim = |colour: Color32| colour.gamma_multiply(DISABLED_OPACITY);
        Self {
            fill: dim(self.fill),
            tint: dim(self.tint),
            border: dim(self.border),
            text: dim(self.text),
            glow: None,
        }
    }
}

const fn metrics(size: Size) -> (f32, f32, f32) {
    match size {
        Size::Normal => (BUTTON_HEIGHT, BUTTON_PADDING_X, FS_XS),
        Size::Compact => (BUTTON_XS_HEIGHT, BUTTON_XS_PADDING_X, BUTTON_XS_SIZE),
    }
}

fn label_galley(ui: &Ui, size: Size, label: &str) -> std::sync::Arc<egui::Galley> {
    let (_, _, font_size) = metrics(size);
    WidgetText::from(caps(label, font_size, Weight::Bold, BUTTON_TRACKING)).into_galley(
        ui,
        Some(TextWrapMode::Extend),
        f32::INFINITY,
        TextStyle::Button,
    )
}

/// Returns the width of a button of `size` with `label`.
pub(crate) fn width(ui: &Ui, size: Size, label: &str) -> f32 {
    let (_, padding, _) = metrics(size);
    padding.mul_add(2.0, label_galley(ui, size, label).size().x)
}

/// Shows a button of `kind` and `size` with `label` set in upper case; its accessible name stays
/// `label`. A disabled button senses no click.
pub(crate) fn button(ui: &mut Ui, kind: Kind, size: Size, label: &str, enabled: bool) -> Response {
    let galley = label_galley(ui, size, label);
    button_laid(ui, (kind, size), (label, galley), enabled)
}

/// Returns the width of a button of `size` whose label is laid out as `galley`.
pub(crate) fn width_of(size: Size, galley: &egui::Galley) -> f32 {
    let (_, padding, _) = metrics(size);
    padding.mul_add(2.0, galley.size().x)
}

/// Lays out `label` as a button of `size` sets it, for [`button_laid`].
pub(crate) fn laid_label(ui: &Ui, size: Size, label: &str) -> std::sync::Arc<egui::Galley> {
    label_galley(ui, size, label)
}

/// Shows a button as [`button`] does with the label laid out before by [`laid_label`].
pub(crate) fn button_laid(
    ui: &mut Ui,
    (kind, size): (Kind, Size),
    (label, galley): (&str, std::sync::Arc<egui::Galley>),
    enabled: bool,
) -> Response {
    button_state(ui, (kind, size), (label, galley), (enabled, None))
}

/// Shows a button as [`button_laid`] does; with `selected` it is a choice that says whether it is
/// chosen: in the selected colours of the controls and as selected for assistive technology.
pub(super) fn button_state(
    ui: &mut Ui,
    (kind, size): (Kind, Size),
    (label, galley): (&str, std::sync::Arc<egui::Galley>),
    (enabled, selected): (bool, Option<bool>),
) -> Response {
    let (height, padding, _) = metrics(size);
    let desired = vec2(padding.mul_add(2.0, galley.size().x), height);
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(desired, sense);
    response.widget_info(|| {
        selected.map_or_else(
            || WidgetInfo::labeled(Role::Button, enabled, label),
            |active| WidgetInfo::selected(Role::Button, enabled, active, label),
        )
    });
    if !ui.is_rect_visible(rect) {
        return response;
    }
    let hover =
        ui.ctx()
            .animate_bool_with_time(response.id, enabled && response.hovered(), TRANSITION);
    let mut colours = if enabled && response.is_pointer_button_down_on() {
        look(kind, Phase::Pressed)
    } else if selected == Some(true) {
        chosen_look()
    } else {
        look(kind, Phase::Rest).blend(look(kind, Phase::Hover), hover)
    };
    if !enabled {
        colours = colours.dimmed();
    }
    paint(ui, rect, colours);
    let text_at = Align2::CENTER_CENTER
        .align_size_within_rect(galley.size(), rect)
        .min;
    ui.painter().galley(text_at, galley, colours.text);
    if response.has_focus() {
        let ring = if matches!(kind, Kind::Cta | Kind::Danger) {
            palette::WHITE
        } else {
            palette::PRIMARY
        };
        focus_ring(ui, rect, ring, FOCUS_OFFSET);
    }
    response
}

/// Shows the ghost `.btn-xs.btn-icon` close cross at `rect`, named `label` for assistive
/// technology.
pub(crate) fn close_icon(ui: &Ui, rect: Rect, label: &str) -> Response {
    let response = ui.interact(rect, ui.scope_id().with(("close", label)), Sense::click());
    ghost_icon(ui, &response, label, |colour| {
        let cross = rect.shrink(rect.width() / 2.0 - FS_XS / 2.0);
        let stroke = Stroke::new(FOCUS_WIDTH, colour);
        ui.painter()
            .line_segment([cross.left_top(), cross.right_bottom()], stroke);
        ui.painter()
            .line_segment([cross.right_top(), cross.left_bottom()], stroke);
    });
    response
}

/// Paints the ghost `.btn-xs.btn-icon` of `response`, named `label` for assistive technology,
/// and calls `icon` with the colour of its glyph.
pub(crate) fn ghost_icon(ui: &Ui, response: &Response, label: &str, icon: impl FnOnce(Color32)) {
    response.widget_info(|| WidgetInfo::labeled(Role::Button, true, label));
    let phase = if response.is_pointer_button_down_on() {
        Phase::Pressed
    } else if response.hovered() {
        Phase::Hover
    } else {
        Phase::Rest
    };
    let colours = look(Kind::Ghost, phase);
    paint(ui, response.rect, colours);
    icon(colours.text);
    if response.has_focus() {
        focus_ring(ui, response.rect, palette::PRIMARY, FOCUS_OFFSET);
    }
}
/// Paints the fill, tint, border and glow of `look` over `rect` with the control radius.
pub(crate) fn paint(ui: &Ui, rect: Rect, look: Look) {
    let painter = ui.painter();
    if let Some(glow) = look.glow {
        painter.add(glow.as_shape(rect, CORNER_CONTROL));
    }
    painter.rect_filled(rect, CORNER_CONTROL, look.fill);
    painter.rect_filled(rect, CORNER_CONTROL, look.tint);
    painter.rect_stroke(
        rect,
        CORNER_CONTROL,
        Stroke::new(1.0, look.border),
        StrokeKind::Inside,
    );
}

#[cfg(test)]
mod tests {
    use super::{Kind, Phase, look};
    use crate::theme::desktop::GHOST_HOVER;
    use crate::theme::palette;
    use crate::theme::tokens::{GLOW_PRIMARY, GLOW_PRIMARY_LG};

    #[test]
    fn the_cta_is_filled_orange_with_black_text_and_a_bloom() {
        let rest = look(Kind::Cta, Phase::Rest);
        let hover = look(Kind::Cta, Phase::Hover);

        assert_eq!(rest.fill, palette::PRIMARY, "fill");
        assert_eq!(rest.text, palette::BLACK, "text");
        assert_eq!(rest.glow, Some(GLOW_PRIMARY), "bloom");
        assert_eq!(hover.fill, palette::ACCENT_LIGHT, "hover fill");
        assert_eq!(hover.glow, Some(GLOW_PRIMARY_LG), "hover bloom");
        assert_eq!(
            look(Kind::Cta, Phase::Pressed).fill,
            palette::ACCENT_DARK,
            "pressed"
        );
    }

    #[test]
    fn a_ghost_has_a_control_fill_and_a_grey_border_that_turn_orange_on_hover() {
        let rest = look(Kind::Ghost, Phase::Rest);
        let hover = look(Kind::Ghost, Phase::Hover);

        assert_eq!(rest.fill, palette::CONTROL_BG, "fill");
        assert_eq!(rest.border, palette::CONTROL_BORDER, "border");
        assert_eq!(rest.text, palette::CONTROL_FG, "text");
        assert_eq!(hover.fill.a(), 0, "hover replaces the fill");
        assert_eq!(hover.tint, GHOST_HOVER, "hover tint");
        assert_eq!(hover.border, palette::PRIMARY, "hover border");
        assert_eq!(hover.text, palette::ACCENT_LIGHT, "hover text");
    }

    #[test]
    fn an_outline_is_unfilled_and_orange() {
        let rest = look(Kind::Outline, Phase::Rest);

        assert_eq!(rest.fill.a(), 0, "no fill");
        assert_eq!(rest.border, palette::PRIMARY, "border");
        assert_eq!(rest.text, palette::PRIMARY, "text");
    }
}
