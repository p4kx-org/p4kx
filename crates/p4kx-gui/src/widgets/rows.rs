// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use egui::{
    Color32, CornerRadius, Id, Painter, Pos2, Rect, Role, Sense, Stroke, Ui, WidgetInfo, pos2, vec2,
};

use super::focus::focus_ring;
use super::frames::accent_edge;
use super::text::{caps, line, paint_left};
use crate::theme::desktop::{
    ACCENT_BAR, ACTIVE_BAR, CHEVRON_STROKE, FOCUS_WIDTH, LIST_ROW_HOVER, SECTION_CHEVRON,
    SECTION_TRACKING, TITLE_SIZE, TREE_ROW_HOVER, ZEBRA,
};
use crate::theme::palette;
use crate::theme::tokens::{CORNER_CONTROL, SPACE_4, TOUCH_TARGET, Weight};

/// How a data row is shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RowState {
    Rest,
    Hovered,
    ListHovered,
    TreeHovered,
    Checked,
    Active,
}

impl RowState {
    /// Returns the state of a `.master-row` from its flags: like [`RowState::of`], but hovered in
    /// the 3 % white of a list.
    pub(crate) const fn of_list(active: bool, hovered: bool) -> Self {
        if active {
            Self::Active
        } else if hovered {
            Self::ListHovered
        } else {
            Self::Rest
        }
    }

    /// Returns the state of a `.tree-row` from its flags: like [`RowState::of_list`], but hovered in
    /// the 2.5 % white of the tree.
    pub(crate) const fn of_tree(active: bool, hovered: bool) -> Self {
        if active {
            Self::Active
        } else if hovered {
            Self::TreeHovered
        } else {
            Self::Rest
        }
    }

    /// Returns the state of a row from its flags, the active row winning.
    pub(crate) const fn of(active: bool, checked: bool, hovered: bool) -> Self {
        if active {
            Self::Active
        } else if hovered {
            Self::Hovered
        } else if checked {
            Self::Checked
        } else {
            Self::Rest
        }
    }

    /// Returns the fill of the row and the colour of its text.
    pub(crate) const fn colours(self) -> (Color32, Color32) {
        match self {
            Self::Rest => (Color32::TRANSPARENT, palette::GRAY_1),
            Self::Hovered => (palette::TABLE_ROW_HOVER, palette::GRAY_1),
            Self::ListHovered => (LIST_ROW_HOVER, palette::GRAY_1),
            Self::TreeHovered => (TREE_ROW_HOVER, palette::GRAY_1),
            Self::Checked => (palette::CONTROL_SELECTED_BG, palette::GRAY_1),
            Self::Active => (palette::CONTROL_SELECTED_BG, palette::WHITE),
        }
    }
}

/// Returns the weight of a row's text: Bold for the active row.
pub(crate) const fn row_weight(active: bool) -> Weight {
    if active {
        Weight::Bold
    } else {
        Weight::Regular
    }
}

/// Paints the background of a data row in `state`, with the orange bar of the active row.
pub(crate) fn paint_row(painter: &Painter, rect: Rect, state: RowState) {
    let (fill, _) = state.colours();
    painter.rect_filled(rect, CornerRadius::ZERO, fill);
    if state == RowState::Active {
        let bar = Rect::from_min_size(rect.min, vec2(ACTIVE_BAR, rect.height()));
        painter.rect_filled(bar, CornerRadius::ZERO, palette::PRIMARY);
    }
}

/// Paints the background of a table row like [`paint_row`], with the zebra stripe of `tbody tr`
/// when `striped` and at rest.
pub(crate) fn paint_table_row(painter: &Painter, rect: Rect, state: RowState, striped: bool) {
    if striped && state == RowState::Rest {
        painter.rect_filled(rect, CornerRadius::ZERO, ZEBRA);
    } else {
        paint_row(painter, rect, state);
    }
}

/// Paints the focus ring 2 points inside a row or head cell at `rect` while `response` has focus.
pub(crate) fn focus_inside(painter: &Painter, rect: Rect, response: &egui::Response) {
    if response.has_focus() {
        painter.rect_stroke(
            rect.shrink(FOCUS_WIDTH),
            CornerRadius::ZERO,
            Stroke::new(FOCUS_WIDTH, palette::PRIMARY),
            egui::StrokeKind::Inside,
        );
    }
}

/// Paints a hairline divider under a row.
pub(crate) fn paint_divider(painter: &Painter, rect: Rect) {
    painter.hline(
        rect.x_range(),
        rect.bottom() - 0.5,
        Stroke::new(1.0, palette::GRAY_3),
    );
}

/// Paints a chevron of `size` centred on `centre`, pointing right, or down when `open`.
pub(crate) fn chevron(painter: &Painter, centre: Pos2, size: f32, open: bool, colour: Color32) {
    let (quarter, half) = (size / 4.0, size / 2.0);
    let stroke = Stroke::new(CHEVRON_STROKE.min(quarter), colour);
    let (a, b, c) = if open {
        (
            pos2(centre.x - half, centre.y - quarter),
            pos2(centre.x, centre.y + quarter),
            pos2(centre.x + half, centre.y - quarter),
        )
    } else {
        (
            pos2(centre.x - quarter, centre.y - half),
            pos2(centre.x + quarter, centre.y),
            pos2(centre.x - quarter, centre.y + half),
        )
    };
    painter.line_segment([a, b], stroke);
    painter.line_segment([b, c], stroke);
}

/// Paints a chevron of `size` centred on `centre`, pointing up when `up`, else down.
pub(crate) fn sort_chevron(painter: &Painter, centre: Pos2, size: f32, up: bool, colour: Color32) {
    let (quarter, half) = (size / 4.0, size / 2.0);
    let tip = if up { -quarter } else { quarter };
    let stroke = Stroke::new(CHEVRON_STROKE.min(quarter), colour);
    let left = pos2(centre.x - half, centre.y - tip);
    let middle = pos2(centre.x, centre.y + tip);
    let right = pos2(centre.x + half, centre.y - tip);
    painter.line_segment([left, middle], stroke);
    painter.line_segment([middle, right], stroke);
}

/// Shows a `.panel-header` that opens and closes the section below it; returns whether it is open.
pub(crate) fn section_header(ui: &mut Ui, id: Id, title: &str, default_open: bool) -> bool {
    let mut open = ui.data(|data| data.get_temp(id)).unwrap_or(default_open);
    let size = vec2(ui.available_width(), TOUCH_TARGET);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    response.widget_info(|| WidgetInfo::selected(Role::Button, true, open, title));
    if response.clicked() {
        open = !open;
        ui.data_mut(|data| data.insert_temp(id, open));
    }
    let fill = if response.hovered() {
        palette::GRAY_3
    } else {
        palette::SURFACE_BAR
    };
    let painter = ui.painter();
    painter.rect(
        rect,
        CORNER_CONTROL,
        fill,
        Stroke::new(1.0, palette::GRAY_3),
        egui::StrokeKind::Inside,
    );
    let bar = Rect::from_min_size(rect.min, vec2(ACCENT_BAR, rect.height()));
    accent_edge(painter, (rect, CORNER_CONTROL), bar, palette::PRIMARY);
    let text = caps(
        title,
        TITLE_SIZE,
        Weight::Bold,
        SECTION_TRACKING / TITLE_SIZE,
    );
    let inner = rect.shrink2(vec2(SPACE_4, 0.0));
    let galley = line(ui, text, inner.width() - SECTION_CHEVRON - SPACE_4);
    paint_left(ui, inner, galley, palette::WHITE);
    let centre = pos2(inner.right() - SECTION_CHEVRON / 2.0, inner.center().y);
    chevron(
        ui.painter(),
        centre,
        SECTION_CHEVRON,
        open,
        palette::PRIMARY,
    );
    if response.has_focus() {
        focus_ring(ui, rect, palette::PRIMARY, FOCUS_WIDTH);
    }
    open
}

#[cfg(test)]
mod tests {
    use super::RowState;
    use crate::theme::desktop::{LIST_ROW_HOVER, TREE_ROW_HOVER};
    use crate::theme::palette;

    #[test]
    fn the_active_row_wins_over_hover_and_check() {
        assert_eq!(RowState::of(true, true, true), RowState::Active, "active");
        assert_eq!(RowState::of(false, true, true), RowState::Hovered, "hover");
        assert_eq!(
            RowState::of(false, true, false),
            RowState::Checked,
            "checked"
        );
    }

    #[test]
    fn rows_use_the_surface_ladder_and_never_a_solid_orange_block() {
        assert_eq!(
            RowState::Hovered.colours().0,
            palette::TABLE_ROW_HOVER,
            "hover"
        );
        assert_eq!(
            RowState::of_list(false, true).colours().0,
            LIST_ROW_HOVER,
            "list hover"
        );
        assert_eq!(RowState::of_list(true, true), RowState::Active, "active");
        assert_eq!(
            RowState::of_tree(false, true).colours().0,
            TREE_ROW_HOVER,
            "tree hover"
        );
        assert_eq!(
            RowState::Checked.colours().0,
            palette::CONTROL_SELECTED_BG,
            "checked tint"
        );
        assert_eq!(
            RowState::Active.colours(),
            (palette::CONTROL_SELECTED_BG, palette::WHITE),
            "active"
        );
    }
}
