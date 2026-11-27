// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use egui::{Align, Id, Layout, Rect, Role, Sense, Ui, Vec2, WidgetInfo, pos2, vec2};
use egui_extras::{Column, TableBuilder, TableRow};

use super::comparison_rows::{RowCache, RowGalleys};
use super::comparison_summary::kind_look;
use super::regions;
use super::table::{head_fill, sort_header};
use crate::theme::desktop::{CELL_PADDING_X, CHANGE_COLUMN, CHANGE_SIZE_COLUMN, ROW_HEIGHT};
use crate::theme::palette;
use crate::theme::tokens::{FS_XS, Weight};
use crate::widgets::chip;
use crate::widgets::rows::{RowState, focus_inside, row_weight};
use crate::widgets::table_row::RowSlots;
use crate::widgets::text::{line, paint_left, paint_right, plain};
use crate::widgets::tooltip;
use crate::{ChangeColumn, ComparisonText, ComparisonView, Texts};

const COLUMNS: [(ChangeColumn, ComparisonText); 4] = [
    (ChangeColumn::Change, ComparisonText::ColumnChange),
    (ChangeColumn::Path, ComparisonText::ColumnPath),
    (ChangeColumn::OldSize, ComparisonText::ColumnOldSize),
    (ChangeColumn::NewSize, ComparisonText::ColumnNewSize),
];

/// What the user did in the list.
pub(super) enum ListAction {
    Sort(ChangeColumn),
    Select(u32),
}

/// Shows the sortable, virtual list of the shown changes, scrolled so the row at `reveal` is
/// visible; only the visible rows are laid out.
pub(super) fn show(
    ui: &mut Ui,
    texts: &Texts,
    view: &ComparisonView,
    reveal: Option<usize>,
) -> Option<ListAction> {
    let mut cache = RowCache::load(ui, texts, view, &COLUMNS.map(|(_, text)| text));
    let hovered_id = ui.scope_id().with("changes-hovered-row");
    let hovered = ui.data(|data| data.get_temp::<usize>(hovered_id));
    let mut hovered_now = None;
    let mut action = None;
    let left = ui.max_rect().left();
    let body_top = ui.cursor().top() + ROW_HEIGHT;
    ui.spacing_mut().item_spacing = Vec2::ZERO;
    let mut table = TableBuilder::new(ui);
    if let Some(row) = reveal {
        table = table.scroll_to_row(row, None);
    }
    table
        .id_salt("comparison-changes")
        .striped(false)
        .resizable(true)
        .auto_shrink(false)
        .min_scrolled_height(0.0)
        .sense(Sense::click())
        .cell_layout(Layout::left_to_right(Align::Center))
        .column(Column::exact(CHANGE_COLUMN))
        .column(Column::remainder().at_least(160.0).clip(true))
        .column(Column::initial(CHANGE_SIZE_COLUMN).at_least(80.0))
        .column(Column::initial(CHANGE_SIZE_COLUMN).at_least(80.0))
        .header(ROW_HEIGHT, |mut header| {
            for (index, (column, _)) in COLUMNS.into_iter().enumerate() {
                header.col(|ui| {
                    head_fill(
                        ui,
                        if index == 0 {
                            left
                        } else {
                            ui.max_rect().left()
                        },
                    );
                    let sort = view.sort();
                    let sorted = (sort.column == column).then_some(sort.direction);
                    if sort_header(ui, sorted, cache.head(index)) {
                        action = Some(ListAction::Sort(column));
                    }
                });
            }
        })
        .body(|body| {
            regions::rows(body, ROW_HEIGHT, view.shown().len(), |row| {
                let index = row.index();
                let (clicked, is_hovered) =
                    show_row(row, (texts, view), &mut cache, hovered == Some(index));
                action = clicked.or_else(|| action.take());
                if is_hovered {
                    hovered_now = Some(index);
                }
            });
        });
    cache.store(ui);
    remember_hovered(ui, hovered_id, hovered_now);
    if view.shown().is_empty() && !view.is_filtering() {
        empty_note(ui, texts, view, pos2(left, body_top));
    }
    action
}

fn remember_hovered(ui: &Ui, id: Id, row: Option<usize>) {
    ui.data_mut(|data| match row {
        Some(index) => {
            data.insert_temp(id, index);
        }
        None => {
            data.remove_temp::<usize>(id);
        }
    });
}

fn show_row(
    mut row: TableRow<'_, '_>,
    (texts, view): (&Texts, &ComparisonView),
    cache: &mut RowCache,
    hovered: bool,
) -> (Option<ListAction>, bool) {
    row.set_hovered(false);
    let Some((id, change)) = view
        .shown()
        .get(row.index())
        .and_then(|&id| Some((id, view.change(id)?)))
    else {
        return (None, false);
    };
    let active = view.selected() == Some(id);
    let state = RowState::of(active, false, hovered);
    let (_, colour) = state.colours();
    let weight = row_weight(active);
    let mut galleys: Option<RowGalleys> = None;
    let index = row.index();
    let mut slots = None;
    row.col(|ui| {
        slots = Some(RowSlots::reserve(ui));
        let rect = ui.max_rect();
        if let Some(label) = cache.chip(change.kind()) {
            let at = pos2(rect.left() + CELL_PADDING_X, rect.center().y);
            chip::paint(ui, at, label, kind_look(change.kind()).1);
        }
    });
    row.col(|ui| {
        let cell = ui.max_rect().shrink2(vec2(CELL_PADDING_X, 0.0));
        let laid_out = cache.row(ui, texts, (id, &change), (cell.width(), weight));
        paint_left(ui, cell, laid_out.path.clone(), colour);
        galleys = Some(laid_out);
    });
    let sizes = galleys.map_or([None, None], |galleys| {
        [
            Some((galleys.old, change.old_entry().is_some())),
            Some((galleys.new, change.new_entry().is_some())),
        ]
    });
    for size in sizes {
        row.col(|ui| {
            if let Some((galley, present)) = size {
                let cell = ui.max_rect().shrink2(vec2(CELL_PADDING_X, 0.0));
                let tint = if present {
                    colour
                } else {
                    palette::GRAY_2_TEXT
                };
                paint_right(ui, cell, galley, tint);
            }
        });
    }
    let response = row.response();
    if let Some(slots) = slots {
        slots.paint(&response, state, index % 2 == 1);
    }
    let is_hovered = response.hovered();
    let clicked = finish_row(&response, texts, &change, active);
    (clicked.then_some(ListAction::Select(id)), is_hovered)
}

/// Names the row of `change` for assistive technology, rings it while focused and shows the full
/// path on hover; returns whether it was clicked.
fn finish_row(
    response: &egui::Response,
    texts: &Texts,
    change: &p4kx_comparison::EntryChange<'_>,
    active: bool,
) -> bool {
    response.widget_info(|| {
        let kind = texts.get(kind_look(change.kind()).0);
        let name = texts.format(
            ComparisonText::Row,
            &[
                ("change", kind.into()),
                ("path", change.path().to_owned().into()),
            ],
        );
        WidgetInfo::selected(Role::Row, true, active, name)
    });
    focus_inside(
        &response.ctx.layer_painter(response.layer_id),
        response.rect,
        response,
    );
    tooltip::hover(response.clone(), |ui| {
        tooltip::line(ui, change.path());
    })
    .clicked()
}

fn empty_note(ui: &Ui, texts: &Texts, view: &ComparisonView, top_left: egui::Pos2) {
    let text = if !view.comparison().levels().entries {
        ComparisonText::EntriesSkipped
    } else if view.summary().incomparables != 0 {
        ComparisonText::NoConfirmedDifferences
    } else if view.filter_text().is_empty() {
        ComparisonText::NoDifferences
    } else {
        ComparisonText::NoMatches
    };
    let rect = Rect::from_min_size(top_left, vec2(ui.max_rect().width(), ROW_HEIGHT))
        .shrink2(vec2(CELL_PADDING_X, 0.0));
    let galley = line(
        ui,
        plain(texts.get(text), FS_XS, Weight::Regular),
        rect.width(),
    );
    let response = ui.interact(rect, ui.scope_id().with("changes-empty"), Sense::hover());
    response.widget_info(|| WidgetInfo::labeled(Role::Label, true, texts.get(text)));
    paint_left(ui, rect, galley, palette::GRAY_2_TEXT);
}
