// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::Arc;

use egui::{Align, Frame, Id, Label, Layout, Margin, Ui, vec2};

use super::{Dialog, Requests, empty, list_footer, query_editor, query_table};
use crate::query::Trigger;
use crate::theme::desktop::{COMPARISON_PROGRESS_WIDTH, SPINNER_INLINE, TOOLBAR_PADDING};
use crate::widgets::button::{self, Kind, Size};
use crate::widgets::frames::{self, px};
use crate::widgets::indicator;
use crate::widgets::pagination::pagination;
use crate::widgets::text::hairline_below;
use crate::{Model, QueryExportFormat, QueryResult, QueryStatus, QueryTab, QueryText};

mod completion_list;
mod notes;
mod saved_dialog;
mod saved_list;

use notes::Notes;
pub(super) use saved_dialog::show as saved_dialog;
use saved_list::SavedAction;

/// Shows the Query tab: the editor with its actions and progress, the error of a Query that
/// cannot run, and the result with its export and pages.
pub(super) fn show(ui: &mut Ui, model: &mut Model, requests: &mut Requests) {
    if model.browser().is_none() {
        let texts = model.texts();
        let title = texts.get(QueryText::EmptyTitle);
        let hint = texts.get(QueryText::EmptyHint);
        empty::notice(ui, &title, &hint);
        return;
    }
    let notes = Notes::get(ui, model.texts(), model.query());
    let toolbar = frames::panel()
        .inner_margin(Margin::same(px(TOOLBAR_PADDING)))
        .show(ui, |ui| {
            editor(ui, model, &notes);
            saved(ui, model);
        });
    hairline_below(ui, toolbar.response.rect);
    frames::panel().show(ui, |ui| {
        ui.set_min_size(ui.available_size());
        results(ui, model, &notes, requests);
    });
}

fn editor(ui: &mut Ui, model: &mut Model, notes: &Notes) {
    ui.spacing_mut().item_spacing = vec2(TOOLBAR_PADDING, TOOLBAR_PADDING);
    let label = notes.show_label(ui);
    let mark = model.query().mark();

    let edited = query_editor::show(
        ui,
        model.query_mut().editing(),
        (&notes.hint, label.id),
        mark.as_ref(),
    );
    complete(ui, model, &edited);
    if let Some(alert) = &notes.alert {
        frames::danger_alert(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.add(Label::new(Arc::clone(alert)).selectable(true));
        });
    }
    let mut run = edited.submitted;
    let mut cancel = false;
    let mut save = false;
    ui.horizontal(|ui| {
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let enabled = !model.query().text().trim().is_empty();
            if model.query().is_running() {
                cancel =
                    button::button(ui, Kind::Ghost, Size::Normal, &notes.cancel, true).clicked();
            } else {
                run |= button::button(ui, Kind::Cta, Size::Normal, &notes.run, enabled).clicked();
            }
            save = button::button(ui, Kind::Ghost, Size::Normal, &notes.save, enabled).clicked();
            ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                progress(ui, model.query(), notes);
            });
        });
    });
    if save {
        model.query_mut().begin_save();
    }
    if cancel {
        model.query_mut().cancel();
    } else if run {
        let ctx = ui.ctx().clone();
        model.run_query(move || ctx.request_repaint());
    }
}

/// Reads the completion schema when the editor gets the focus or Ctrl+Space asks, keeps the list
/// in step with the text and the cursor, and shows it.
fn complete(ui: &Ui, model: &mut Model, edited: &query_editor::Edited) {
    let focus_id = Id::unique("p4kx-query-editor-had-focus");
    let had_focus = ui.data(|data| data.get_temp::<bool>(focus_id).unwrap_or(false));
    ui.data_mut(|data| data.insert_temp(focus_id, edited.cursor.is_some()));
    let editor = query_editor::editor_id();
    let Some(cursor) = edited.cursor else {
        completion_list::announce(ui.ctx(), editor, None);
        return;
    };
    if !had_focus || edited.asked || model.query_completion_unprepared() {
        let ctx = ui.ctx().clone();
        model.prepare_query_completion(move || ctx.request_repaint());
    }
    let trigger = if edited.asked {
        Trigger::Asked
    } else if edited.changed {
        Trigger::Typed
    } else {
        Trigger::Moved
    };
    model.update_query_completion(cursor, trigger);
    let (Some(shown), Some(caret)) = (model.query().completion_shown(), edited.caret) else {
        completion_list::announce(ui.ctx(), editor, None);
        return;
    };
    let generation = model.query().completion().generation();
    let typed = model.query().completion().typed_len();
    let clicked = completion_list::show(
        ui,
        &edited.response,
        shown,
        (caret, generation, typed),
        model.texts(),
    );
    if let Some(index) = clicked {
        let (text, completion) = model.query_mut().editing();
        completion.choose(index);
        if let Some(end) = completion.accept(text) {
            query_editor::place_cursor(ui.ctx(), text, end);
            ui.memory_mut(|memory| memory.request_focus(query_editor::editor_id()));
        }
    }
}

/// Shows the saved Queries under the editor and carries out what was done with them.
fn saved(ui: &mut Ui, model: &mut Model) {
    let key = {
        let mut hasher = DefaultHasher::new();
        model.texts().language().hash(&mut hasher);
        model.query().saved_revision().hash(&mut hasher);
        hasher.finish()
    };
    let action = saved_list::show(ui, model.query().saved(), (model.texts(), key));
    match action {
        Some(SavedAction::Run(index)) => {
            let ctx = ui.ctx().clone();
            model.run_saved(index, move || ctx.request_repaint());
        }
        Some(SavedAction::Rename(index)) => model.query_mut().begin_rename(index),
        Some(SavedAction::Delete(index)) => model.query_mut().begin_delete(index),
        None => {}
    }
}

fn progress(ui: &mut Ui, query: &QueryTab, notes: &Notes) {
    if let QueryStatus::Running(_) = query.status() {
        match notes.fraction {
            Some(fraction) => {
                let width = ui.available_width().min(COMPARISON_PROGRESS_WIDTH);
                indicator::progress(ui, fraction, false, width);
            }
            None => indicator::spinner(ui, SPINNER_INLINE),
        }
    }
    if let Some(status) = &notes.status {
        ui.add(Label::new(Arc::clone(status)).selectable(false));
    }
}

fn results(ui: &mut Ui, model: &mut Model, notes: &Notes, requests: &mut Requests) {
    let Some(result) = model.query().result() else {
        prompt(ui, notes);
        return;
    };
    let exporting = model.query().is_exporting();
    let cancel_export = export_bar(ui, result, notes, (exporting, requests));
    let page = footer(ui, result, notes);
    let sort = if result.is_empty() {
        prompt(ui, notes);
        None
    } else {
        let laid = query_table::Laid::get(ui, model.texts(), result);
        query_table::show(ui, result, &laid)
    };
    if cancel_export {
        model.query_mut().cancel_export();
    }
    let Some(result) = model.query_mut().result_mut() else {
        return;
    };
    if let Some(column) = sort {
        result.sort_by(column);
    }
    if let Some(page) = page {
        result.set_page(page);
    }
}

fn prompt(ui: &mut Ui, notes: &Notes) {
    if let Some(prompt) = &notes.prompt {
        frames::preview_body().show(ui, |ui| {
            ui.add(Label::new(Arc::clone(prompt)));
        });
    }
}

/// Shows the export actions and the export note above the table; returns whether the running
/// export was asked to stop.
fn export_bar(
    ui: &mut Ui,
    result: &QueryResult,
    notes: &Notes,
    (exporting, requests): (bool, &mut Requests),
) -> bool {
    let mut cancel = false;
    let bar = Frame::NONE
        .inner_margin(Margin::same(px(TOOLBAR_PADDING)))
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing = vec2(TOOLBAR_PADDING, TOOLBAR_PADDING);
            ui.horizontal(|ui| {
                if let Some(export) = &notes.export {
                    ui.add(Label::new(Arc::clone(export)).selectable(true));
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if exporting {
                        cancel = button::button(
                            ui,
                            Kind::Ghost,
                            Size::Normal,
                            &notes.export_cancel,
                            true,
                        )
                        .clicked();
                        return;
                    }
                    let enabled = !result.is_empty();
                    for (format, label) in [
                        (QueryExportFormat::Json, notes.export_json.as_ref()),
                        (QueryExportFormat::Csv, notes.export_csv.as_ref()),
                        (QueryExportFormat::CsvStrict, "CSV (RFC 4180)"),
                    ] {
                        if button::button(ui, Kind::Ghost, Size::Normal, label, enabled).clicked() {
                            requests.dialog = Some(Dialog::QueryExport(format));
                        }
                    }
                });
            });
        });
    hairline_below(ui, bar.response.rect);
    cancel
}

/// Shows the `.list-panel__footer` below the table: the number of Records and the pages;
/// returns the page asked for.
fn footer(ui: &mut Ui, result: &QueryResult, notes: &Notes) -> Option<usize> {
    let mut page = None;
    list_footer::show(ui, "query-footer", |ui| {
        if let Some(count) = &notes.count {
            ui.add(Label::new(Arc::clone(count)).selectable(false));
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if result.page_count() > 1
                && let Some(status) = &notes.page_status
            {
                page = pagination(
                    ui,
                    (result.page(), result.page_count()),
                    Arc::clone(status),
                    notes.page_names(),
                );
            }
        });
    });
    page
}
