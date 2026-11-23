// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use egui::{Align, Layout, Ui};
use p4kx_export::{FLAT_CSV_FILE, FLAT_NDJSON_FILE, NDJSON_FILE};

use super::extraction::{
    POLICIES, folder_field, foot, group_label, new_folder_name, number, refusal_notice,
};
use super::filter_text::problem_text;
use super::search::filter_field;
use super::{Dialog, Requests, regions};
use crate::theme::desktop::MODAL_GROUP_GAP;
use crate::theme::palette;
use crate::theme::tokens::{FS_XS, Weight};
use crate::widgets::button::{self, Kind, Size};
use crate::widgets::frames::{self, Tone};
use crate::widgets::placement;
use crate::widgets::text::plain;
use crate::widgets::toggle;
use crate::{
    DataForgeText, ExtractionScope, ExtractionText, Model, RecordExportSetup,
    RecordExportText as T, RecordForm, RecordScope, TargetKind, Text, Texts,
};

enum Action {
    Submit,
    Close,
    ChooseFolder,
}

/// Shows the Record export setup while it is open and applies what the user chose.
pub(super) fn setup(ui: &Ui, model: &mut Model, requests: &mut Requests) {
    if model.record_export_setup().is_none() {
        return;
    }
    let new_folder = new_folder_name(model, ExtractionScope::All);
    let Some((setup, texts)) = model.record_export_setup_with_texts() else {
        return;
    };
    let title = texts.get(T::SetupTitle);
    let close_dialog = texts.get(Text::CloseDialog);
    let id = frames::modal_id("record-export-setup");
    let shown = placement::hanging_modal(ui.ctx(), id, Tone::Normal).show(ui.ctx(), |ui| {
        ui.set_width(frames::MODAL_WIDE_BODY_WIDTH);
        let closed = frames::head(ui, (&title, Tone::Normal), true, Some(&close_dialog));
        let action = choices(ui, texts, setup, &new_folder);
        if closed { Some(Action::Close) } else { action }
    });
    frames::modal_decoration(
        ui.ctx(),
        shown.response.layer_id,
        shown.response.rect,
        Tone::Normal,
    );
    regions::record(ui.ctx(), |regions| {
        regions.dialogs.push(shown.response.rect);
    });
    let close = shown.should_close();
    match shown.inner {
        Some(Action::Submit) => {
            let ctx = ui.ctx().clone();
            requests.settings_changed |= model.submit_record_export(move || ctx.request_repaint());
        }
        Some(Action::Close) => model.close_record_export_setup(),
        Some(Action::ChooseFolder) => requests.dialog = Some(Dialog::RecordExportFolder),
        None if close => model.close_record_export_setup(),
        None => {}
    }
}

fn choices(
    ui: &mut Ui,
    texts: &Texts,
    setup: &mut RecordExportSetup,
    new_folder: &str,
) -> Option<Action> {
    let room = placement::body_room(ui);
    let scrolled = frames::scrolling_body_in(ui, ("record-export-body", room), |ui| {
        body(ui, texts, setup, new_folder)
    });
    regions::record(ui.ctx(), |regions| {
        regions.scrolls = scrolled.content_size.y > scrolled.inner_rect.height() + 0.5;
    });
    let action = scrolled.inner;
    let pressed = foot(
        ui,
        texts,
        (T::Start, setup.is_ready()),
        ExtractionText::Close,
    );
    action.or(match pressed {
        Some(true) => Some(Action::Submit),
        Some(false) => Some(Action::Close),
        None => None,
    })
}

/// Shows everything above the foot of the setup: the Records, the form and the target.
fn body(
    ui: &mut Ui,
    texts: &Texts,
    setup: &mut RecordExportSetup,
    new_folder: &str,
) -> Option<Action> {
    let mut action = None;
    records(ui, texts, setup);
    ui.add_space(MODAL_GROUP_GAP);
    group_label(ui, &texts.get(T::FormHeading));
    for form in crate::RECORD_FORMS {
        let label = match form {
            RecordForm::Ndjson => texts.format(T::FormNdjson, &[("name", NDJSON_FILE.into())]),
            RecordForm::FlatCsv => texts.format(T::FormFlatCsv, &[("name", FLAT_CSV_FILE.into())]),
            RecordForm::FlatCsvRaw => texts.get(T::FormFlatCsvRaw),
            RecordForm::FlatCsvStrict => "CSV (RFC 4180)".into(),
            RecordForm::FlatNdjson => {
                texts.format(T::FormFlatNdjson, &[("name", FLAT_NDJSON_FILE.into())])
            }
            RecordForm::Json | RecordForm::Xml => texts.get(T::FormJson),
        };
        if toggle::radio(ui, setup.form() == form, &label, true).clicked() {
            setup.set_form(form);
        }
    }
    let mut tags = setup.tags();
    let enabled = crate::form_annotates_tags(setup.form());
    if toggle::checkbox_if(ui, &mut tags, &texts.get(T::Tags), enabled).clicked() {
        setup.set_tags(tags);
    }
    ui.add_space(MODAL_GROUP_GAP);
    group_label(ui, &texts.get(ExtractionText::TargetHeading));
    for (kind, text) in [
        (TargetKind::NewFolder, ExtractionText::TargetNew),
        (TargetKind::ExistingFolder, ExtractionText::TargetExisting),
    ] {
        if toggle::radio(ui, setup.kind() == kind, &texts.get(text), true).clicked() {
            setup.set_kind(kind);
        }
    }
    ui.horizontal(|ui| {
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let choose = texts.get(ExtractionText::ChooseFolder);
            if button::button(ui, Kind::Ghost, Size::Normal, &choose, true).clicked() {
                action = Some(Action::ChooseFolder);
            }
            ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                folder_field(ui, texts, setup.folder());
            });
        });
    });
    refusal_notice(ui, texts, setup.refusal());
    target_details(ui, texts, setup, new_folder);
    action
}

/// Shows the choice of the Records: the selection of the Record tree or a record-path filter.
fn records(ui: &mut Ui, texts: &Texts, setup: &mut RecordExportSetup) {
    group_label(ui, &texts.get(T::ScopeHeading));
    let tree_label = setup.tree().filter(|tree| !tree.is_empty()).map_or_else(
        || texts.get(T::ScopeTreeNone),
        |tree| {
            let name = if tree.name().is_empty() {
                texts.get(DataForgeText::Node)
            } else {
                tree.name().to_owned()
            };
            texts.format(
                T::ScopeTree,
                &[("name", name.into()), ("number", number(texts, tree.len()))],
            )
        },
    );
    let has_tree = setup.tree().is_some_and(|tree| !tree.is_empty());
    let selected = setup.scope() == RecordScope::Tree;
    if toggle::radio(ui, selected, &tree_label, has_tree).clicked() {
        setup.set_scope(RecordScope::Tree);
    }
    let filter_label = texts.get(T::ScopeFilter);
    let by_filter = setup.scope() == RecordScope::Filter;
    let radio = toggle::radio(ui, by_filter, &filter_label, true);
    if radio.clicked() {
        setup.set_scope(RecordScope::Filter);
    }
    let problem = setup
        .filter_error()
        .map(|problem| problem_text(texts, &problem));
    let hint = texts.get(T::FilterHint);
    if filter_field(
        ui,
        setup.filter_mut(),
        (&hint, radio.id),
        problem.as_deref(),
    ) {
        setup.set_scope(RecordScope::Filter);
    }
}

/// Shows the name of a new folder or the Conflict policy for an existing one.
fn target_details(ui: &mut Ui, texts: &Texts, setup: &mut RecordExportSetup, new_folder: &str) {
    if setup.kind() == TargetKind::NewFolder {
        let hint = texts.format(
            ExtractionText::TargetNewName,
            &[("name", new_folder.to_owned().into())],
        );
        ui.label(plain(hint, FS_XS, Weight::Regular).color(palette::GRAY_2_TEXT));
        return;
    }
    ui.add_space(MODAL_GROUP_GAP);
    group_label(ui, &texts.get(ExtractionText::PolicyHeading));
    for (policy, text) in POLICIES {
        if toggle::radio(ui, setup.policy() == policy, &texts.get(text), true).clicked() {
            setup.set_policy(policy);
        }
    }
}
