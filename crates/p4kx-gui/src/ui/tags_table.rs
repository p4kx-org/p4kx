// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::{Arc, Mutex};

use egui::{
    Align, Color32, CornerRadius, Galley, Layout, Rect, Response, Role, ScrollArea, Sense, Ui,
    Vec2, WidgetInfo, pos2, vec2,
};
use egui_extras::{Column, TableBuilder, TableRow};

use super::regions;
use super::table::head_fill;
use super::tags_cache::{Cache, ForWidth, HEADS, Laid, RowCells, memo};
use super::tree::{rails, toggle};
use crate::theme::desktop::{
    CELL_PADDING_X, ROW_FONT, ROW_HEIGHT, TABLE_HEAD_SIZE, TABLE_HEAD_TRACKING,
};
use crate::theme::palette;
use crate::theme::tags_metrics::{TAGS_GUID_WIDTH, TAGS_LEGACY_WIDTH, TAGS_NAME_MIN_WIDTH};
use crate::theme::tokens::Weight;
use crate::widgets::rows::{RowState, focus_inside, paint_divider, paint_row, row_weight};
use crate::widgets::text::{caps, line, paint_left, paint_right, plain};
use crate::widgets::tooltip;
use crate::{TagRow, TagsText, TagsView, Texts};

/// What the user did in the tree: the position in the pre-order of the tag it is about.
#[derive(Clone, Copy)]
pub(super) enum Action {
    Select(usize),
    Toggle(usize),
}

#[derive(Default)]
struct Outcome {
    action: Option<Action>,
    hovered: Option<usize>,
}

/// What a cell of a row shows: the row, its place among the rows shown, its column and its state.
struct Cell<'a, 'v> {
    row: &'a TagRow<'v>,
    index: usize,
    column: usize,
    state: RowState,
}

/// Shows the tag tree as a table with the tag, GUID and legacy ID columns, as wide as its columns
/// need and scrolling sideways when the window is narrower; returns what the user did. The heads
/// and the cells come from `cache`, which lays them out once.
pub(super) fn show(
    ui: &mut Ui,
    texts: &Texts,
    view: &TagsView,
    cache: &Mutex<Cache>,
) -> Option<Action> {
    let hovered_id = ui.scope_id().with("tags-hovered-row");
    let hovered = ui.data(|data| data.get_temp::<usize>(hovered_id));
    let mut outcome = Outcome::default();
    let mut cache = Cache::lock(cache);
    let labels = cache.labels();
    let (fitted, cells) = cache.table(view.generation());
    ScrollArea::horizontal()
        .id_salt("tags")
        .auto_shrink(false)
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing = Vec2::ZERO;
            let left = ui.max_rect().left();
            TableBuilder::new(ui)
                .id_salt("tags-rows")
                .striped(false)
                .resizable(false)
                .auto_shrink(false)
                .min_scrolled_height(0.0)
                .sense(Sense::hover())
                .cell_layout(Layout::left_to_right(Align::Center))
                .column(Column::remainder().at_least(TAGS_NAME_MIN_WIDTH).clip(true))
                .column(Column::exact(TAGS_GUID_WIDTH).clip(true))
                .column(Column::exact(TAGS_LEGACY_WIDTH).clip(true))
                .header(ROW_HEIGHT, |mut header| {
                    for (column, ((head, laid), fit)) in
                        labels.heads.iter().zip(fitted.iter_mut()).enumerate()
                    {
                        header.col(|ui| head_cell(ui, (column, *head, laid, fit), left));
                    }
                })
                .body(|body| {
                    regions::rows(body, ROW_HEIGHT, view.shown_len(), |row| {
                        let index = row.index();
                        let Some(tag) = view.row(index) else {
                            return;
                        };
                        let laid = cells.entry(tag.tag().position()).or_default();
                        laid.touch();
                        let (action, is_hovered) =
                            show_row(row, texts, (&tag, index), laid, hovered == Some(index));
                        if action.is_some() {
                            outcome.action = action;
                        }
                        if is_hovered {
                            outcome.hovered = Some(index);
                        }
                    });
                });
        });
    drop(cache);
    ui.data_mut(|data| {
        if let Some(index) = outcome.hovered {
            data.insert_temp(hovered_id, index);
        } else {
            data.remove_temp::<usize>(hovered_id);
        }
    });
    outcome.action
}

/// Paints the head cell of `column`: the fill and the orange rule, the label in capitals; the
/// legacy ID is a number and its head is right-aligned.
fn head_cell(
    ui: &Ui,
    (column, head, laid, fit): (usize, TagsText, &Laid, &mut ForWidth<Arc<Galley>>),
    table_left: f32,
) {
    let rect = ui.max_rect();
    head_fill(ui, if column == 0 { table_left } else { rect.left() });
    let inner = rect.shrink2(vec2(CELL_PADDING_X, 0.0));
    let width = inner.width().max(0.0);
    let galley = if laid.galley.size().x <= width {
        Arc::clone(&laid.galley)
    } else {
        let head = caps(
            &laid.text,
            TABLE_HEAD_SIZE,
            Weight::Bold,
            TABLE_HEAD_TRACKING,
        );
        Arc::clone(memo(fit, width.to_bits(), || line(ui, head, width)))
    };
    if head == TagsText::ColumnLegacy {
        paint_right(ui, inner, galley, palette::GRAY_1);
    } else {
        paint_left(ui, inner, galley, palette::GRAY_1);
    }
    ui.interact(
        rect,
        ui.scope_id().with(("tags-head", column)),
        Sense::hover(),
    )
    .widget_info(|| WidgetInfo::labeled(Role::ColumnHeader, true, &*laid.text));
}

fn show_row(
    mut row: TableRow<'_, '_>,
    texts: &Texts,
    (tag_row, index): (&TagRow<'_>, usize),
    cells: &mut RowCells,
    hovered: bool,
) -> (Option<Action>, bool) {
    row.set_hovered(false);
    let position = tag_row.tag().position();
    let state = RowState::of_tree(tag_row.is_selected(), hovered);
    let mut action = None;
    let mut is_hovered = false;
    for column in 0..HEADS.len() {
        row.col(|ui| {
            let cell = Cell {
                row: tag_row,
                index,
                column,
                state,
            };
            let (response, toggled) = cell.show(ui, texts, cells);
            is_hovered |= response.hovered();
            if toggled || (response.double_clicked() && tag_row.is_expandable()) {
                action = Some(Action::Toggle(position));
            } else if response.clicked() && action.is_none() {
                action = Some(Action::Select(position));
            }
        });
    }
    (action, is_hovered)
}

impl Cell<'_, '_> {
    /// Shows the cell; returns its response and whether its chevron was clicked.
    fn show(&self, ui: &Ui, texts: &Texts, cells: &mut RowCells) -> (Response, bool) {
        let tag = self.row.tag();
        let rect = ui.max_rect();
        if self.column == 0 {
            paint_row(ui.painter(), rect, self.state);
        } else {
            ui.painter()
                .rect_filled(rect, CornerRadius::ZERO, self.state.colours().0);
        }
        paint_divider(ui.painter(), rect);
        let id = ui
            .scope_id()
            .with(("tags-cell", tag.position(), self.column));
        let response = ui.interact(rect, id, Sense::click());
        let (_, colour) = self.state.colours();
        let muted = if self.row.is_selected() {
            colour
        } else {
            palette::GRAY_2_TEXT
        };
        let toggled = match self.column {
            0 => self.name(ui, texts, cells, &response, colour),
            1 => {
                let width = text_width(ui);
                let laid = memo(&mut cells.guid, width.to_bits(), || {
                    lay(ui, tag.guid().to_string(), width)
                });
                number_or_id(ui, &response, laid, muted, false);
                false
            }
            _ => {
                let width = text_width(ui);
                let laid = memo(&mut cells.legacy, width.to_bits(), || {
                    lay(
                        ui,
                        tag.legacy_id()
                            .map_or_else(String::new, |id| id.to_string()),
                        width,
                    )
                });
                number_or_id(ui, &response, laid, muted, true);
                false
            }
        };
        focus_inside(ui.painter(), rect, &response);
        (response, toggled)
    }

    /// Shows the chevron, the rails and the name of the first cell; returns whether the chevron
    /// was clicked.
    fn name(
        &self,
        ui: &Ui,
        texts: &Texts,
        cells: &mut RowCells,
        response: &Response,
        colour: Color32,
    ) -> bool {
        let tag = self.row.tag();
        let rect = ui.max_rect();
        response.widget_info(|| {
            WidgetInfo::selected(
                Role::Button,
                true,
                self.row.is_selected(),
                tag.path_view().to_string(),
            )
        });
        let chevron = rails(ui, rect, tag.depth());
        let toggled = self.row.is_expandable() && {
            let id = ui
                .scope_id()
                .with(("tags-toggle", tag.position(), self.index));
            toggle(ui, texts, (id, chevron), self.row.is_expanded(), tag.name())
        };
        let name_rect = Rect::from_min_max(pos2(chevron.right(), rect.top()), rect.max);
        let width = name_rect.width().max(0.0);
        let selected = self.row.is_selected();
        let slot = if selected {
            &mut cells.name_bold
        } else {
            &mut cells.name
        };
        let galley = memo(slot, width.to_bits(), || {
            line(ui, plain(tag.name(), ROW_FONT, row_weight(selected)), width)
        });
        paint_left(ui, name_rect, Arc::clone(galley), colour);
        tooltip::hover(response.clone(), |ui| {
            tooltip::line(ui, tag.path());
        });
        toggled
    }
}

fn text_width(ui: &Ui) -> f32 {
    ui.max_rect()
        .shrink2(vec2(CELL_PADDING_X, 0.0))
        .width()
        .max(0.0)
}

fn lay(ui: &Ui, text: String, width: f32) -> Laid {
    Laid {
        galley: line(ui, plain(&text, ROW_FONT, Weight::Regular), width),
        text: text.into(),
    }
}

/// Paints the laid-out `text` in the cell of `ui`, right-aligned when it is a number, and names
/// the cell for assistive technology.
fn number_or_id(ui: &Ui, response: &Response, laid: &Laid, colour: Color32, right: bool) {
    let inner = ui.max_rect().shrink2(vec2(CELL_PADDING_X, 0.0));
    response.widget_info(|| WidgetInfo::labeled(Role::Cell, true, &*laid.text));
    let galley: Arc<Galley> = Arc::clone(&laid.galley);
    if right {
        paint_right(ui, inner, galley, colour);
    } else {
        paint_left(ui, inner, galley, colour);
    }
}
