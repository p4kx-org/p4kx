// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use egui::{
    CornerRadius, Galley, Painter, Rect, Response, Role, Sense, Stroke, StrokeKind, TextStyle,
    TextWrapMode, Ui, WidgetInfo, WidgetText, pos2, vec2,
};

use super::focus::focus_ring_square;
use super::frames::accent_edge;
use super::text::{caps, line, paint_left, plain};
use crate::theme::desktop::{
    ACTIVE_BAR, FOCUS_WIDTH, SEGMENT_FOCUS_OFFSET, SEGMENT_HEIGHT, SEGMENT_PADDING_X,
    TAB_BAR_PADDING_LEFT, TAB_COUNT_GAP, TAB_PADDING_X, TAB_SIZE, TAB_TRACKING,
};
use crate::theme::palette;
use crate::theme::tokens::{
    CORNER_CONTROL, CORNER_NONE, FS_XS, RADIUS_CONTROL_POINTS, SPACE_2, TOUCH_TARGET, Weight,
};

const INNER_RADIUS: u8 = RADIUS_CONTROL_POINTS - 1;
const SEGMENT_COUNT_GAP: f32 = SPACE_2;

/// One tab of a `.tab-nav` laid out once: its label and an optional `.tab-count`.
#[derive(Clone)]
pub(crate) struct TabLabel {
    name: String,
    label: Arc<Galley>,
    count: Option<Arc<Galley>>,
}

impl TabLabel {
    /// Lays out the tab `name` with `count` after it, if any.
    pub(crate) fn new(ui: &Ui, name: &str, count: Option<&str>) -> Self {
        let caps_galley = |text: &str| {
            WidgetText::from(caps(text, TAB_SIZE, Weight::Bold, TAB_TRACKING)).into_galley(
                ui,
                Some(TextWrapMode::Extend),
                f32::INFINITY,
                TextStyle::Button,
            )
        };
        Self {
            name: name.to_owned(),
            label: caps_galley(name),
            count: count.map(caps_galley),
        }
    }

    fn width(&self) -> f32 {
        let count = self
            .count
            .as_ref()
            .map_or(0.0, |count| TAB_COUNT_GAP + count.size().x);
        TAB_PADDING_X.mul_add(2.0, self.label.size().x + count)
    }
}

/// Shows a `.tab-nav` with `labels`, `current` the selected index; returns the clicked index.
pub(crate) fn tab_bar(ui: &mut Ui, labels: &[String], current: usize) -> Option<usize> {
    tab_bar_reserving(ui, labels, current, 0.0)
}

/// Shows a `.tab-nav` like [`tab_bar`] that leaves `reserve` at its right end to other controls.
pub(crate) fn tab_bar_reserving(
    ui: &mut Ui,
    labels: &[String],
    current: usize,
    reserve: f32,
) -> Option<usize> {
    let laid: Vec<TabLabel> = labels
        .iter()
        .map(|label| TabLabel::new(ui, label, None))
        .collect();
    tabs_reserving(ui, &laid, current, reserve)
}

/// Shows a `.tab-nav` with the tabs laid out already, `current` the selected index; returns the
/// clicked index.
pub(crate) fn tabs(ui: &mut Ui, labels: &[TabLabel], current: usize) -> Option<usize> {
    tabs_reserving(ui, labels, current, 0.0)
}

/// Shows a `.tab-nav` like [`tabs`] that leaves `reserve` at its right end to other controls; tabs
/// that do not fit scroll sideways with the wheel, and the selected tab is scrolled into view.
pub(crate) fn tabs_reserving(
    ui: &mut Ui,
    labels: &[TabLabel],
    current: usize,
    reserve: f32,
) -> Option<usize> {
    let width = ui.available_width();
    let (bar, _) = ui.allocate_exact_size(vec2(width, TOUCH_TARGET), Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(bar, CORNER_NONE, palette::SURFACE_BAR);
    painter.hline(
        bar.x_range(),
        bar.bottom() - 0.5,
        Stroke::new(1.0, palette::GRAY_3),
    );
    let strip = Rect::from_min_max(
        bar.min,
        pos2((bar.right() - reserve).max(bar.left()), bar.bottom()),
    );
    let painter = ui.painter().with_clip_rect(strip);
    let mut left = strip.left() + TAB_BAR_PADDING_LEFT - scrolled(ui, strip, labels, current);
    let mut clicked = None;
    for (index, tab) in labels.iter().enumerate() {
        let rect = Rect::from_min_size(pos2(left, bar.top()), vec2(tab.width(), bar.height()));
        left = rect.right();
        if !rect.intersects(strip) {
            continue;
        }
        let id = ui.scope_id().with(("tab", index));
        let response = ui.interact(rect.intersect(strip), id, Sense::click());
        let active = index == current;
        response.widget_info(|| WidgetInfo::selected(Role::Tab, true, active, &tab.name));
        paint_tab(ui, &painter, tab, (rect, strip), (active, &response));
        if response.clicked() {
            clicked = Some(index);
        }
    }
    clicked
}

/// Paints `tab` in `rect`, clipped to `strip`; `active` tells the selected tab, `response` its state.
fn paint_tab(
    ui: &Ui,
    painter: &Painter,
    tab: &TabLabel,
    (rect, strip): (Rect, Rect),
    (active, response): (bool, &Response),
) {
    let colour = if active {
        palette::WHITE
    } else if response.hovered() {
        palette::GRAY_1
    } else {
        palette::GRAY_2_TEXT
    };
    let label_left = rect.left() + TAB_PADDING_X;
    let centre = |galley: &Galley| rect.center().y - galley.size().y / 2.0;
    painter.galley(
        pos2(label_left, centre(&tab.label)),
        Arc::clone(&tab.label),
        colour,
    );
    if let Some(count) = &tab.count {
        let count_colour = if active {
            palette::ACCENT_LIGHT
        } else {
            palette::GRAY_1
        };
        let at = pos2(
            label_left + tab.label.size().x + TAB_COUNT_GAP,
            centre(count),
        );
        painter.galley(at, Arc::clone(count), count_colour);
    }
    if active {
        let underline = Rect::from_min_max(pos2(rect.left(), rect.bottom() - ACTIVE_BAR), rect.max);
        painter.rect_filled(underline, CORNER_NONE, palette::PRIMARY);
    }
    if response.has_focus() {
        focus_ring_square(ui, rect.intersect(strip), palette::PRIMARY, -FOCUS_WIDTH);
    }
}
/// Returns how far the tabs of `strip` are scrolled to the left: kept from the last frame, moved
/// by the wheel over the strip and, once `current` or the strip changed, enough to show that tab.
fn scrolled(ui: &Ui, strip: Rect, labels: &[TabLabel], current: usize) -> f32 {
    let before: f32 = labels.iter().take(current).map(TabLabel::width).sum();
    let after = before + labels.get(current).map_or(0.0, TabLabel::width);
    let total = TAB_BAR_PADDING_LEFT + labels.iter().map(TabLabel::width).sum::<f32>();
    let most = (total - strip.width()).max(0.0);
    let id = ui.scope_id().with("tab-scroll");
    let (mut offset, seen, seen_width) = ui
        .data(|data| data.get_temp::<(f32, usize, f32)>(id))
        .unwrap_or((0.0, usize::MAX, 0.0));
    if most > 0.0 {
        if seen != current || (seen_width - strip.width()).abs() > 0.5 {
            offset = offset
                .max(after + TAB_BAR_PADDING_LEFT - strip.width())
                .min(before);
        }
        if ui.rect_contains_pointer(strip) {
            let delta = ui.input(|input| input.smooth_scroll_delta);
            offset -= delta.x + delta.y;
        }
    }
    let offset = offset.clamp(0.0, most);
    ui.data_mut(|data| data.insert_temp(id, (offset, current, strip.width())));
    offset
}
/// One item of a `.segmented` group laid out once: its name in both weights and an optional
/// `.segmented__count`.
#[derive(Clone)]
pub(crate) struct SegmentLabel {
    accessible: String,
    regular: Arc<Galley>,
    bold: Arc<Galley>,
    count: Option<Arc<Galley>>,
}

impl SegmentLabel {
    /// Lays out the item `name` with `count` after it, if any.
    pub(crate) fn new(ui: &Ui, name: &str, count: Option<&str>) -> Self {
        let named = |weight| line(ui, plain(name, FS_XS, weight), f32::INFINITY);
        Self {
            accessible: count.map_or_else(|| name.to_owned(), |count| format!("{name} {count}")),
            regular: named(Weight::Regular),
            bold: named(Weight::Bold),
            count: count.map(|count| line(ui, plain(count, FS_XS, Weight::Bold), f32::INFINITY)),
        }
    }
}

/// Shows the items of a `.segmented--sm` group with their data kept as written; `active` is
/// the pressed item. Returns the clicked index.
pub(crate) fn segments<S: AsRef<str>>(
    ui: &mut Ui,
    items: &[S],
    active: Option<usize>,
) -> Option<usize> {
    group(ui, items.len(), active, |ui, index, pressed, place| {
        let item = items.get(index).map_or("", AsRef::as_ref);
        let weight = if pressed {
            Weight::Bold
        } else {
            Weight::Regular
        };
        let galley = line(ui, plain(item, FS_XS, weight), f32::INFINITY);
        segment(ui, (item, galley, None), pressed, place)
    })
}

/// Shows the items of a `.segmented--sm` group laid out already, each with its
/// `.segmented__count`; `active` is the pressed item. Returns the clicked index.
pub(crate) fn counted_segments(
    ui: &mut Ui,
    items: &[SegmentLabel],
    active: Option<usize>,
) -> Option<usize> {
    group(ui, items.len(), active, |ui, index, pressed, place| {
        let Some(item) = items.get(index) else {
            return ui.allocate_response(vec2(0.0, 0.0), Sense::hover());
        };
        let galley = Arc::clone(if pressed { &item.bold } else { &item.regular });
        segment(
            ui,
            (&item.accessible, galley, item.count.as_ref()),
            pressed,
            place,
        )
    })
}

fn group(
    ui: &mut Ui,
    len: usize,
    active: Option<usize>,
    mut item: impl FnMut(&mut Ui, usize, bool, Place) -> Response,
) -> Option<usize> {
    let mut clicked = None;
    let spacing = std::mem::replace(&mut ui.spacing_mut().item_spacing.x, 0.0);
    let mut group = Rect::NOTHING;
    for index in 0..len {
        let place = Place {
            first: index == 0,
            last: index + 1 == len,
        };
        let response = item(ui, index, active == Some(index), place);
        group = group.union(response.rect);
        if response.clicked() {
            clicked = Some(index);
        }
    }
    ui.spacing_mut().item_spacing.x = spacing;
    ui.painter().rect_stroke(
        group,
        CORNER_CONTROL,
        Stroke::new(1.0, palette::CONTROL_BORDER),
        StrokeKind::Inside,
    );
    clicked
}

/// Where a segment sits in its group, which decides the corners it shares with the group frame.
#[derive(Debug, Clone, Copy)]
struct Place {
    first: bool,
    last: bool,
}

impl Place {
    /// Returns the corners of the whole segment, rounded at the group's outer corners.
    const fn corner(self) -> CornerRadius {
        CornerRadius {
            nw: if self.first { INNER_RADIUS } else { 0 },
            sw: if self.first { INNER_RADIUS } else { 0 },
            ne: if self.last { INNER_RADIUS } else { 0 },
            se: if self.last { INNER_RADIUS } else { 0 },
        }
    }
}

fn segment(
    ui: &mut Ui,
    (item, galley, count): (&str, Arc<Galley>, Option<&Arc<Galley>>),
    pressed: bool,
    place: Place,
) -> Response {
    let counted = count.map_or(0.0, |count| SEGMENT_COUNT_GAP + count.size().x);
    let size = vec2(
        SEGMENT_PADDING_X.mul_add(2.0, galley.size().x + counted),
        SEGMENT_HEIGHT,
    );
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    response.widget_info(|| WidgetInfo::selected(Role::Button, true, pressed, item));
    let hovered = response.hovered() && !pressed;
    let (fill, text) = if pressed {
        (palette::CONTROL_SELECTED_BG, palette::CONTROL_SELECTED_FG)
    } else if hovered {
        (palette::CONTROL_BG_HOVER, palette::WHITE)
    } else {
        (palette::CONTROL_BG, palette::CONTROL_FG)
    };
    let painter = ui.painter();
    painter.rect_filled(rect, place.corner(), palette::CONTROL_BG);
    painter.rect_filled(rect, place.corner(), fill);
    if !place.first {
        painter.vline(
            rect.left() + 0.5,
            rect.y_range(),
            Stroke::new(1.0, palette::CONTROL_BORDER),
        );
    }
    if pressed {
        let border = palette::CONTROL_SELECTED_BORDER;
        painter.rect_stroke(
            rect,
            place.corner(),
            Stroke::new(1.0, border),
            StrokeKind::Inside,
        );
        let underline = Rect::from_min_max(pos2(rect.left(), rect.bottom() - ACTIVE_BAR), rect.max);
        accent_edge(painter, (rect, place.corner()), underline, border);
    }
    let inner = rect.shrink2(vec2(SEGMENT_PADDING_X, 0.0));
    let name_width = galley.size().x;
    paint_left(ui, inner, galley, text);
    if let Some(count) = count {
        let colour = if pressed {
            palette::CONTROL_SELECTED_FG
        } else {
            palette::GRAY_2_TEXT
        };
        let room = Rect::from_min_max(
            pos2(inner.left() + name_width + SEGMENT_COUNT_GAP, inner.top()),
            inner.max,
        );
        paint_left(ui, room, Arc::clone(count), colour);
    }
    if response.has_focus() {
        let colour = if pressed {
            palette::CONTROL_SELECTED_FG
        } else {
            palette::PRIMARY
        };
        focus_ring_square(ui, rect, colour, SEGMENT_FOCUS_OFFSET);
    }
    response
}
