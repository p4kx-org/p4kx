// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::sync::Arc;

use egui::{
    Color32, CornerRadius, CursorIcon, Galley, Id, Rect, Role, ScrollArea, Sense, Stroke, Ui,
    UiBuilder, Vec2, WidgetInfo, pos2, vec2,
};

use super::record_incoming;
use super::record_key::Trailer;
use super::record_preview::{self, RecordStep};
use super::record_value::{spoken_value, value_text};
use super::table::head_fill;
use crate::theme::desktop::{
    CELL_PADDING_X, FOCUS_OFFSET, HAIRLINE, ROW_FONT, ROW_HEIGHT, TABLE_HEAD_SIZE,
    TABLE_HEAD_TRACKING, TREE_INDENT, TREE_RAIL_OFFSET, ZEBRA,
};
use crate::theme::palette;
use crate::theme::tokens::{FS_XS, SPACE_2, Weight};
use crate::widgets::focus::focus_ring_square;
use crate::widgets::rows::paint_divider;
use crate::widgets::scroll_region;
use crate::widgets::text::{caps, line, paint_left, plain};
use crate::widgets::tooltip;
use crate::{
    FieldLine, IncomingView, LineValue, RecordChangeText, RecordText, RecordView, ScrollText, Text,
    Texts,
};

const MAX_ROWS: usize = 512;
const HEADS: [Text; 2] = [
    Text::RecordChange(RecordChangeText::HeadField),
    Text::Record(RecordText::HeadValue),
];

/// The lines a list shows with what tells it from the other lists: its heads, the name of its
/// scroll view, the key of its caches and the text that names a link; it takes `path` and `field`.
pub(super) struct List<'a> {
    pub(super) lines: &'a [FieldLine],
    pub(super) source: Option<&'a RecordView>,
    pub(super) incoming: Option<&'a IncomingView>,
    pub(super) heads: [Text; 2],
    pub(super) follow: Text,
    pub(super) scroll: ScrollText,
    pub(super) salt: &'static str,
}

/// The laid-out cells of one line.
#[derive(Clone)]
struct Laid {
    width: f32,
    name: Arc<Galley>,
    value: (Arc<Galley>, Color32),
    trailer: Option<Trailer>,
}

/// The laid-out heads and the lines shown recently, kept until the Record, the language or the
/// scale change.
#[derive(Clone, Default)]
struct LineCache {
    key: Option<Id>,
    heads: Vec<Arc<Galley>>,
    head_names: String,
    rows: HashMap<usize, Laid>,
}

impl LineCache {
    fn load(ui: &Ui, texts: &Texts, generation: u64, list: &List<'_>) -> Self {
        let key = Id::unique((
            list.salt,
            texts.language(),
            generation,
            ui.ctx().pixels_per_point().to_bits(),
        ));
        let mut cache: Self = ui.data_mut(|data| {
            std::mem::take(data.get_temp_mut_or_default::<Self>(cache_id(list.salt)))
        });
        if cache.key != Some(key) {
            let names = list.heads.map(|text| texts.get(text));
            cache = Self {
                key: Some(key),
                heads: names
                    .iter()
                    .map(|name| {
                        let text = caps(name, TABLE_HEAD_SIZE, Weight::Bold, TABLE_HEAD_TRACKING);
                        line(ui, text, f32::INFINITY)
                    })
                    .collect(),
                head_names: names.join(" "),
                rows: HashMap::new(),
            };
        }
        cache
    }

    fn store(self, ui: &Ui, salt: &'static str) {
        ui.data_mut(|data| *data.get_temp_mut_or_default::<Self>(cache_id(salt)) = self);
    }

    fn row(
        &mut self,
        ui: &Ui,
        texts: &Texts,
        (index, field): (usize, &FieldLine),
        width: f32,
    ) -> Laid {
        if let Some(kept) = self.rows.get(&index)
            && (kept.width - width).abs() < 0.5
        {
            return kept.clone();
        }
        if self.rows.len() >= MAX_ROWS {
            self.rows.clear();
        }
        let indent = f32::from(field.depth()) * TREE_INDENT;
        let name = line(
            ui,
            plain(field.name(), ROW_FONT, Weight::Regular),
            (width - indent).max(0.0),
        );
        let (text, colour) = value_text(texts, field.value());
        let linked = matches!(
            field.value(),
            LineValue::Reference(_) | LineValue::Dangling { .. }
        );
        let value = if linked {
            line(ui, plain(text, FS_XS, Weight::Bold), width)
        } else {
            line(ui, plain(text, ROW_FONT, Weight::Regular), width)
        };
        let room = (width - value.size().x - SPACE_2).max(0.0);
        let trailer = match field.value() {
            LineValue::Reference(link) => {
                let shown = link.tag().unwrap_or_else(|| link.type_name());
                Some(Trailer::type_name(ui, shown, room))
            }
            LineValue::Key { text, .. } => Trailer::key(ui, texts, text, room),
            _ => None,
        };
        let laid = Laid {
            width,
            name,
            value: (value, colour),
            trailer,
        };
        self.rows.insert(index, laid.clone());
        laid
    }
}

fn cache_id(salt: &'static str) -> Id {
    Id::unique(("record-preview-lines", salt))
}

/// Shows the head and the virtual lines of `view`, two equal columns, field and value; returns
/// the Reference that was followed.
pub(super) fn show(
    ui: &mut Ui,
    texts: &Texts,
    view: &RecordView,
    generation: u64,
) -> Option<RecordStep> {
    let list = List {
        lines: view.lines(),
        source: Some(view),
        incoming: None,
        heads: HEADS,
        follow: Text::Record(RecordText::Follow),
        scroll: ScrollText::Lines,
        salt: "record-lines",
    };
    show_list(ui, texts, &list, generation)
}

/// Shows the head and the virtual lines of `list`; returns the Reference that was followed.
pub(super) fn show_list(
    ui: &mut Ui,
    texts: &Texts,
    list: &List<'_>,
    generation: u64,
) -> Option<RecordStep> {
    let mut cache = LineCache::load(ui, texts, generation, list);
    ui.spacing_mut().item_spacing = Vec2::ZERO;
    heads(ui, &cache);
    let mut step = None;
    let area = ScrollArea::vertical()
        .id_salt(("record-preview-lines", list.salt))
        .auto_shrink(false);
    let id = ui.scope_id().with(("record-preview-lines", list.salt));
    scroll_region::show(
        ui,
        (id, || texts.get(list.scroll)),
        ROW_HEIGHT,
        area,
        |area, ui| {
            area.show_rows(ui, ROW_HEIGHT, list.lines.len(), |ui, range| {
                for index in range {
                    if let Some(field) = list.lines.get(index)
                        && let Some(followed) = row(ui, texts, &mut cache, (index, field), list)
                    {
                        step = Some(followed);
                    }
                }
            })
        },
    );
    cache.store(ui, list.salt);
    step
}

fn column_width(rect: Rect) -> f32 {
    (rect.width() / 2.0).floor()
}

fn heads(ui: &mut Ui, cache: &LineCache) {
    let (rect, response) =
        ui.allocate_exact_size(vec2(ui.available_width(), ROW_HEIGHT), Sense::hover());
    response.widget_info(|| WidgetInfo::labeled(Role::RowHeader, true, &cache.head_names));
    head_fill(&ui.new_child(UiBuilder::new().max_rect(rect)), rect.left());
    let width = column_width(rect);
    for (index, head) in (0_u8..).zip(&cache.heads) {
        let left = f32::from(index).mul_add(width, rect.left());
        let at = pos2(left + CELL_PADDING_X, rect.center().y - head.size().y / 2.0);
        ui.painter().galley(at, Arc::clone(head), palette::GRAY_1);
    }
}

fn row(
    ui: &mut Ui,
    texts: &Texts,
    cache: &mut LineCache,
    (index, field): (usize, &FieldLine),
    list: &List<'_>,
) -> Option<RecordStep> {
    let (rect, response) =
        ui.allocate_exact_size(vec2(ui.available_width(), ROW_HEIGHT), Sense::hover());
    let width = column_width(rect);
    let laid = cache.row(
        ui,
        texts,
        (index, field),
        2.0f32.mul_add(-CELL_PADDING_X, width),
    );
    if response.hovered() {
        ui.painter()
            .rect_filled(rect, CornerRadius::ZERO, palette::CONTROL_BG);
    } else if index % 2 == 1 {
        ui.painter().rect_filled(rect, CornerRadius::ZERO, ZEBRA);
    }
    paint_divider(ui.painter(), rect);
    let name_cell = Rect::from_min_max(rect.min, pos2(rect.left() + width, rect.bottom()))
        .shrink2(vec2(CELL_PADDING_X, 0.0));
    name(ui, name_cell, field.depth(), &laid.name);
    let value_cell = Rect::from_min_max(pos2(rect.left() + width, rect.top()), rect.max)
        .shrink2(vec2(CELL_PADDING_X, 0.0));
    let described = || {
        texts.format(
            RecordText::Row,
            &[
                ("field", field.name().to_owned().into()),
                ("value", spoken_value(texts, field.value()).into()),
            ],
        )
    };
    response.widget_info(|| WidgetInfo::labeled(Role::Row, true, described()));
    tooltip::hover(response, |ui| {
        tooltip::line(ui, described());
        if let Some(view) = list.source {
            record_preview::line_origins(ui, texts, view, (index, field));
        } else if let Some(view) = list.incoming {
            record_incoming::line_origins(ui, texts, view, index);
        }
    });
    value(ui, texts, (index, field, list.follow), (value_cell, &laid))
}

/// Shows the value of `field` in `cell`: a link to the Record it refers to, named by `follow`, or
/// the value as text; returns the step a click asks for.
fn value(
    ui: &Ui,
    texts: &Texts,
    (index, field, follow): (usize, &FieldLine, Text),
    (cell, laid): (Rect, &Laid),
) -> Option<RecordStep> {
    match field.value() {
        LineValue::Reference(link) => {
            let label = || {
                texts.format(
                    follow,
                    &[
                        ("path", link.record_path().to_owned().into()),
                        ("field", field.name().to_owned().into()),
                    ],
                )
            };
            let clicked = link_cell(ui, (index, cell), laid, label);
            clicked.then(|| RecordStep::Follow(link.clone()))
        }
        LineValue::Dangling { guid, .. } => {
            let label = || value_text(texts, field.value()).0;
            let clicked = link_cell(ui, (index, cell), laid, label);
            clicked.then_some(RecordStep::Dangling(*guid))
        }
        _ => {
            let (galley, colour) = &laid.value;
            paint_left(ui, cell, Arc::clone(galley), *colour);
            if let Some(trailer) = &laid.trailer {
                let after = cell.left() + galley.size().x + SPACE_2;
                trailer.paint(ui, cell.with_min_x(after.min(cell.right())));
            }
            None
        }
    }
}
/// Paints the depth rails and the name of a line of `depth` in `cell`.
fn name(ui: &Ui, cell: Rect, depth: u16, galley: &Arc<Galley>) {
    for level in 0..depth {
        let x = f32::from(level).mul_add(TREE_INDENT, cell.left() + TREE_RAIL_OFFSET);
        ui.painter()
            .vline(x, cell.y_range(), Stroke::new(1.0, palette::GRAY_3));
    }
    let indent = f32::from(depth) * TREE_INDENT;
    let text = cell.with_min_x((cell.left() + indent).min(cell.right()));
    paint_left(ui, text, Arc::clone(galley), palette::GRAY_1);
}

/// Shows the value of a link line in `cell` as a link named by `label`, the target's type after it;
/// returns whether it was clicked.
fn link_cell(
    ui: &Ui,
    (index, cell): (usize, Rect),
    laid: &Laid,
    label: impl Fn() -> String,
) -> bool {
    let (galley, colour) = &laid.value;
    let size = vec2(galley.size().x.min(cell.width()), galley.size().y);
    let link = Rect::from_min_size(pos2(cell.left(), cell.center().y - size.y / 2.0), size);
    let response = ui
        .interact(
            link,
            ui.scope_id().with(("record-link", index)),
            Sense::click(),
        )
        .on_hover_cursor(CursorIcon::PointingHand);
    response.widget_info(|| WidgetInfo::labeled(Role::Link, true, label()));
    let hovered = response.hovered();
    let shown = if hovered && *colour == palette::PRIMARY {
        palette::ACCENT_LIGHT
    } else {
        *colour
    };
    ui.painter().galley(link.min, Arc::clone(galley), shown);
    if hovered {
        ui.painter()
            .hline(link.x_range(), link.bottom(), Stroke::new(HAIRLINE, shown));
    }
    if response.has_focus() {
        focus_ring_square(ui, link, palette::PRIMARY, FOCUS_OFFSET);
    }
    if let Some(trailer) = &laid.trailer {
        trailer.paint(
            ui,
            cell.with_min_x((link.right() + SPACE_2).min(cell.right())),
        );
    }
    response.clicked()
}
