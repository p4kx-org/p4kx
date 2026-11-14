// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use egui::{
    Color32, Context, CornerRadius, Frame, Id, LayerId, Margin, Modal, Painter, Pos2, Rect,
    ScrollArea, Shape, Stroke, Ui, Vec2, pos2, scroll_area::ScrollAreaOutput, vec2,
};

use super::text::caps;
use crate::theme::desktop::{
    ACCENT_BAR, ALERT_DANGER_FILL, ALERT_WARNING_FILL, BRACKET, BRACKET_STROKE, BUTTON_HEIGHT,
    BUTTON_XS_HEIGHT, HEADER_PADDING_X, MODAL_TOP_EDGE, MODAL_WIDE_WIDTH, MODAL_WIDTH,
    NOTICE_PADDING_Y, PREVIEW_PADDING_X, SCRIM, TITLE_SIZE, TITLE_TRACKING,
};
use crate::theme::palette;
use crate::theme::tokens::{
    CORNER_SM, GLOW_DANGER_LG, GLOW_PRIMARY_LG, RADIUS_SM, SPACE_3, SPACE_4, Weight,
};

/// The padding above and below the content of a modal.
pub(crate) const MODAL_PADDING_Y: f32 = SPACE_3;
/// The height of the foot of a modal: its gap, the hairline's gap and the action.
pub(crate) const FOOT_HEIGHT: f32 = SPACE_4 + SPACE_3 + BUTTON_HEIGHT;
/// The width of the body of a modal.
pub(crate) const MODAL_BODY_WIDTH: f32 = 2.0f32.mul_add(-SPACE_4, MODAL_WIDTH);
/// The width of the body of a wide modal.
pub(crate) const MODAL_WIDE_BODY_WIDTH: f32 = 2.0f32.mul_add(-SPACE_4, MODAL_WIDE_WIDTH);

/// The tone of a modal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Tone {
    Normal,
    Danger,
}

impl Tone {
    const fn accent(self) -> Color32 {
        match self {
            Self::Normal => palette::PRIMARY,
            Self::Danger => palette::DANGER,
        }
    }
}

const fn margin(x: f32, y: f32) -> Margin {
    Margin::symmetric(px(x), px(y))
}

/// Returns a margin of `value` points as egui stores it.
#[expect(
    clippy::cast_possible_truncation,
    reason = "the theme's margins are small whole points"
)]
pub(crate) const fn px(value: f32) -> i8 {
    value as i8
}

/// The frame of the header: the bar surface, its padding and room for the 2 point orange rule.
pub(crate) fn header() -> Frame {
    Frame::NONE.fill(palette::SURFACE_BAR).inner_margin(Margin {
        left: px(HEADER_PADDING_X),
        right: px(HEADER_PADDING_X),
        top: 1,
        bottom: 3,
    })
}

/// Paints the 2 point orange rule under the header at `rect`.
pub(crate) fn header_rule(ui: &Ui, rect: Rect) {
    let rule = Rect::from_min_max(pos2(rect.left(), rect.bottom() - 2.0), rect.max);
    ui.painter()
        .rect_filled(rule, CornerRadius::ZERO, palette::PRIMARY);
}

/// The frame of the status bar.
pub(crate) fn footer() -> Frame {
    Frame::NONE
        .fill(palette::SURFACE_BAR)
        .inner_margin(margin(SPACE_4, 0.0))
}

/// The frame of the sidebar.
pub(crate) fn sidebar() -> Frame {
    Frame::NONE.fill(palette::SURFACE_PANEL)
}

/// The frame of the canvas around the work surface.
pub(crate) fn canvas() -> Frame {
    Frame::NONE.fill(palette::SURFACE_CANVAS)
}

/// The frame of the work surface under the tab bar: the gap the panels sit in as cards.
pub(crate) fn inset() -> Frame {
    Frame::NONE.inner_margin(Margin::same(px(SPACE_3)))
}

/// The frame of a `.list-panel` or the preview: the panel surface with a hairline and the
/// `radius-sm` corner.
pub(crate) fn panel() -> Frame {
    Frame::NONE
        .fill(palette::SURFACE_PANEL)
        .stroke(Stroke::new(1.0, palette::GRAY_3))
        .corner_radius(CORNER_SM)
}

/// Paints `colour` over the part of the rounded `shape` that lies in `edge`, so an accent edge
/// follows the corners of the frame it sits on.
pub(crate) fn accent_edge(
    painter: &Painter,
    (shape, corner): (Rect, CornerRadius),
    edge: Rect,
    colour: Color32,
) {
    painter
        .with_clip_rect(edge)
        .rect_filled(shape, corner, colour);
}

/// The frame of the preview body.
pub(crate) fn preview_body() -> Frame {
    Frame::NONE.inner_margin(margin(PREVIEW_PADDING_X, 8.0))
}

/// Returns the width a text inside an `.alert` that is `outer` wide may take.
pub(crate) const fn alert_text_width(outer: f32) -> f32 {
    2.0f32.mul_add(-SPACE_4, outer) - ACCENT_BAR - 2.0
}

/// Shows an `.alert.alert-warning` with `body` inside.
pub(crate) fn warning_alert<R>(ui: &mut Ui, body: impl FnOnce(&mut Ui) -> R) -> R {
    alert(ui, (ALERT_WARNING_FILL, palette::WARNING), body)
}

/// Shows an `.alert.alert-danger` with `body` inside.
pub(crate) fn danger_alert<R>(ui: &mut Ui, body: impl FnOnce(&mut Ui) -> R) -> R {
    alert(ui, (ALERT_DANGER_FILL, palette::DANGER), body)
}

fn alert<R>(ui: &mut Ui, (fill, accent): (Color32, Color32), body: impl FnOnce(&mut Ui) -> R) -> R {
    let response = Frame::NONE
        .fill(fill)
        .stroke(Stroke::new(1.0, accent))
        .corner_radius(CORNER_SM)
        .inner_margin(Margin {
            left: px(SPACE_4 + ACCENT_BAR),
            right: px(SPACE_4),
            top: px(NOTICE_PADDING_Y),
            bottom: px(NOTICE_PADDING_Y),
        })
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            body(ui)
        });
    let rect = response.response.rect;
    let bar = Rect::from_min_size(rect.min, vec2(ACCENT_BAR, rect.height()));
    accent_edge(ui.painter(), (rect, CORNER_SM), bar, accent);
    response.inner
}

pub(super) fn modal_frame(tone: Tone) -> Frame {
    let glow = if tone == Tone::Danger {
        GLOW_DANGER_LG
    } else {
        GLOW_PRIMARY_LG
    };
    Frame::NONE
        .fill(palette::BG_DARK_GRAY)
        .stroke(Stroke::new(1.0, palette::GRAY_3))
        .corner_radius(CORNER_SM)
        .shadow(glow)
        .inner_margin(margin(SPACE_4, MODAL_PADDING_Y))
}

/// Returns the title of a modal head in `tone`.
pub(crate) fn modal_title(title: &str, tone: Tone) -> egui::RichText {
    let colour = if tone == Tone::Danger {
        palette::DANGER_TEXT
    } else {
        palette::WHITE
    };
    caps(title, TITLE_SIZE, Weight::Bold, TITLE_TRACKING).color(colour)
}

/// Shows the head of a modal: the upper-case `title` over a hairline, with the close cross named
/// `close` when given. A `labelled` head carries `title` as its accessible name; a window's head
/// leaves that to the window. Returns whether the cross was clicked.
pub(crate) fn head(
    ui: &mut Ui,
    (title, tone): (&str, Tone),
    labelled: bool,
    close: Option<&str>,
) -> bool {
    let galley = super::text::line(ui, modal_title(title, tone), f32::INFINITY);
    let (rect, response) = ui.allocate_exact_size(
        vec2(ui.available_width(), BUTTON_XS_HEIGHT),
        egui::Sense::hover(),
    );
    if labelled {
        response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Label, true, title));
    }
    super::text::paint_left(ui, rect, galley, palette::WHITE);
    let closed = close.is_some_and(|label| {
        let cross = Rect::from_min_size(
            pos2(rect.right() - BUTTON_XS_HEIGHT, rect.top()),
            vec2(BUTTON_XS_HEIGHT, BUTTON_XS_HEIGHT),
        );
        super::button::close_icon(ui, cross, label).clicked()
    });
    let y = rect.bottom() + SPACE_3;
    ui.painter().hline(
        rect.expand2(vec2(SPACE_4, 0.0)).x_range(),
        y,
        Stroke::new(1.0, palette::GRAY_3),
    );
    ui.add_space(SPACE_3 + SPACE_4);
    closed
}

/// Starts the foot of a modal: its gap and the hairline across the whole modal above it.
pub(crate) fn foot_rule(ui: &mut Ui) {
    ui.add_space(SPACE_4);
    let top = ui.cursor().top();
    ui.painter().hline(
        ui.max_rect().expand2(vec2(SPACE_4, 0.0)).x_range(),
        top,
        Stroke::new(1.0, palette::GRAY_3),
    );
    ui.add_space(SPACE_3);
}

/// Shows the one action of a modal or window foot at its right end, in a row as high as the
/// button; returns whether it was clicked.
pub(crate) fn foot_action(ui: &mut Ui, label: &str, kind: super::button::Kind) -> bool {
    ui.horizontal(|ui| {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            super::button::button(ui, kind, super::button::Size::Normal, label, true).clicked()
        })
        .inner
    })
    .inner
}

/// Returns the height the body of a modal can take below what `ui` holds so far, so that the foot
/// of the modal stays inside the window: what is left of the window after the head, the foot
/// and the space around the modal.
pub(crate) fn modal_room(ui: &Ui) -> f32 {
    let above = ui.cursor().top() - ui.min_rect().top();
    4.0f32
        .mul_add(
            -SPACE_4,
            ui.ctx().content_rect().height() - above - FOOT_HEIGHT,
        )
        .max(BUTTON_HEIGHT)
}

/// Shows the body of a modal in a vertical scroll area that never grows taller than
/// [`modal_room`], so that the head and the foot stay in view in a small window.
pub(crate) fn scrolling_body<R>(
    ui: &mut Ui,
    salt: &'static str,
    body: impl FnOnce(&mut Ui) -> R,
) -> R {
    let room = modal_room(ui);
    scrolling_body_in(ui, (salt, room), body).inner
}

/// Shows the body of a modal in a vertical scroll area that never grows taller than `room`.
pub(crate) fn scrolling_body_in<R>(
    ui: &mut Ui,
    (salt, room): (&'static str, f32),
    body: impl FnOnce(&mut Ui) -> R,
) -> ScrollAreaOutput<R> {
    ScrollArea::vertical()
        .id_salt(salt)
        .max_height(room)
        .auto_shrink([false, true])
        .show(ui, body)
}

/// Returns a `.modal` on the flat scrim.
pub(crate) fn modal(id: Id, tone: Tone) -> Modal {
    Modal::new(id)
        .frame(modal_frame(tone))
        .backdrop_color(SCRIM)
}

/// Paints the top edge and the corner brackets of a modal at `rect` on its `layer`, and has the
/// frame run again when the modal changed its size, because egui centres a modal from the size
/// of the last frame.
pub(crate) fn modal_decoration(ctx: &Context, layer: LayerId, rect: Rect, tone: Tone) {
    let size_id = layer.id.with("modal-size");
    let resized = ctx.data_mut(|data| {
        let last = data.get_temp::<Vec2>(size_id);
        data.insert_temp(size_id, rect.size());
        last.is_some_and(|last| (last - rect.size()).abs().max_elem() > 0.5)
    });
    if resized {
        ctx.request_discard("modal resized");
    }
    let painter = ctx.layer_painter(layer);
    let accent = tone.accent();
    let edge = Rect::from_min_size(rect.min, vec2(rect.width(), MODAL_TOP_EDGE));
    accent_edge(&painter, (rect, CORNER_SM), edge, accent);
    brackets(&painter, rect.expand(BRACKET_STROKE / 2.0), accent);
}

fn brackets(painter: &Painter, rect: Rect, colour: Color32) {
    let stroke = Stroke::new(BRACKET_STROKE, colour);
    let (min, max) = (rect.min, rect.max);
    painter.line_segment([min, pos2(min.x + BRACKET, min.y)], stroke);
    painter.line_segment([min, pos2(min.x, min.y + BRACKET)], stroke);
    painter.line_segment([max, pos2(max.x - BRACKET, max.y)], stroke);
    painter.line_segment([max, pos2(max.x, max.y - BRACKET)], stroke);
}

const ARC_STEPS: u8 = 4;

/// Returns the closed outline of `rect` with `radius-sm` corners, clockwise from the top-left.
fn rounded_outline(rect: Rect) -> Vec<Pos2> {
    let radius = RADIUS_SM;
    let corners = [
        (pos2(rect.left() + radius, rect.top() + radius), 2.0),
        (pos2(rect.right() - radius, rect.top() + radius), 3.0),
        (pos2(rect.right() - radius, rect.bottom() - radius), 0.0),
        (pos2(rect.left() + radius, rect.bottom() - radius), 1.0),
    ];
    let quarter = std::f32::consts::FRAC_PI_2;
    let mut outline = Vec::with_capacity(4 * (usize::from(ARC_STEPS) + 1) + 1);
    for (centre, start) in corners {
        for step in 0..=ARC_STEPS {
            let angle = (start + f32::from(step) / f32::from(ARC_STEPS)) * quarter;
            outline.push(centre + radius * vec2(angle.cos(), angle.sin()));
        }
    }
    outline.extend(outline.first().copied());
    outline
}

/// Paints a dashed hairline around `rect` in `colour`, following the `radius-sm` corner.
pub(crate) fn dashed(ui: &Ui, rect: Rect, colour: Color32) {
    let stroke = Stroke::new(1.0, colour);
    ui.painter()
        .extend(Shape::dashed_line(&rounded_outline(rect), stroke, 4.0, 4.0));
}

/// The id of a modal from a fixed `name`.
pub(crate) fn modal_id(name: &'static str) -> Id {
    Id::unique(name)
}
