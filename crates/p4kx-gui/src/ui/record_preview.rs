// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use egui::{Id, Key, KeyboardShortcut, Modifiers, PointerButton, Rect, Response, Sense, Ui};
use fluent_bundle::FluentValue;
use p4kx_extraction::Guid;

use super::preview::path_line;
use super::{record_lines, source_origin, xml_view};
use crate::texts::language_name;
use crate::theme::desktop::SPINNER_INLINE;
use crate::theme::palette;
use crate::widgets::frames;
use crate::widgets::indicator::spinner;
use crate::widgets::summary::{Fact, FactsStrip};
use crate::widgets::tooltip;
use crate::{
    Browser, DataForgeText, FieldLine, KeyText, Language, LineValue, Model, PreviewTab,
    RecordChangeText, RecordLanguageText, RecordLink, RecordPreview, RecordPreviewProvenance,
    RecordPreviewStatus, RecordText, RecordView, TagsText, Text, Texts, WorkTab,
};

/// Selects the Record before the current one on the way taken: Alt+Left.
pub(super) const BACK: KeyboardShortcut = KeyboardShortcut::new(Modifiers::ALT, Key::ArrowLeft);
/// Selects the Record after the current one on the way taken: Alt+Right.
pub(super) const FORWARD: KeyboardShortcut = KeyboardShortcut::new(Modifiers::ALT, Key::ArrowRight);

/// A step through the Records that the preview asks for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum RecordStep {
    /// Back along the way taken.
    Back,
    /// Forward along the way taken.
    Forward,
    /// Along a Reference to its target.
    Follow(RecordLink),
    /// Along a Reference to a Record that does not exist.
    Dangling(Guid),
}

/// Applies `step` to the Records of the open Archive; a step to another Record shows it in the
/// Record tab unless the XML tab shows the Record already.
pub(super) fn apply(model: &mut Model, step: &RecordStep) {
    let Some(browser) = model.browser_mut() else {
        return;
    };
    let moved = match step {
        RecordStep::Back => browser.go_back(),
        RecordStep::Forward => browser.go_forward(),
        RecordStep::Follow(link) => {
            browser.follow(link);
            true
        }
        RecordStep::Dangling(guid) => {
            browser.follow_dangling(*guid);
            false
        }
    };
    if moved
        && !matches!(
            model.preview_tab(),
            PreviewTab::Record | PreviewTab::Xml | PreviewTab::Incoming
        )
    {
        model.set_preview_tab(PreviewTab::Record);
    }
}

/// Applies Alt+Left and Alt+Right and the back and forward mouse buttons while the Archive tab
/// shows the Records; the keys wait while a text field has the keyboard.
pub(super) fn handle_shortcuts(ui: &Ui, model: &mut Model) {
    if model.work_tab() != WorkTab::Archive || !model.browser().is_some_and(Browser::shows_records)
    {
        return;
    }
    let typing = ui.ctx().text_edit_focused();
    let (back, forward) = ui.input_mut(|input| {
        let back = (!typing && input.consume_shortcut(&BACK))
            || input.pointer.button_pressed(PointerButton::Extra1);
        let forward = (!typing && input.consume_shortcut(&FORWARD))
            || input.pointer.button_pressed(PointerButton::Extra2);
        (back, forward)
    });
    if back {
        apply(model, &RecordStep::Back);
    } else if forward {
        apply(model, &RecordStep::Forward);
    }
}

/// Shows the Record tab for the selected Record: its record path and type, the facts, a note
/// on a followed dangling Reference and the field lines; returns the step a link asks for.
pub(super) fn show(ui: &mut Ui, texts: &Texts, browser: &Browser) -> Option<RecordStep> {
    let (path, name) = heading(browser);
    let path_row = path_line(ui, path, name);
    let ready = status_body(ui, texts, browser, path)?;
    let generation = browser
        .record_preview()
        .map_or(0, RecordPreview::generation);
    if let Some(guid) = browser.dangling_reference() {
        frames::danger_alert(ui, |ui| {
            ui.colored_label(
                palette::GRAY_1,
                texts.format(RecordText::Dangling, &[("guid", guid.to_string().into())]),
            );
        });
    }
    if let Some((language, reason)) = ready.texts_failure() {
        let language = language_name(texts, language);
        let args = [
            ("language", FluentValue::from(language)),
            ("reason", FluentValue::from(reason.to_owned())),
        ];
        frames::warning_alert(ui, |ui| {
            ui.colored_label(
                palette::GRAY_1,
                texts.format(RecordLanguageText::LoadFailed, &args),
            );
        });
    }
    let header = facts(ui, texts, generation, ready).show(ui);
    header_origins(ui, texts, ready, (path_row, header));
    if ready.lines().is_empty() {
        frames::preview_body().show(ui, |ui| {
            ui.colored_label(palette::GRAY_2_TEXT, texts.get(RecordText::NoFields));
        });
        return None;
    }
    record_lines::show(ui, texts, ready, generation)
}

fn header_origins(ui: &Ui, texts: &Texts, view: &RecordView, (path_row, header): (Response, Rect)) {
    let origin = view
        .provenance()
        .and_then(Result::ok)
        .map(RecordPreviewProvenance::header);
    tooltip::hover(path_row, |ui| {
        source_origin::row(
            ui,
            texts,
            DataForgeText::ColumnRecordPath,
            origin.map(|proof| &proof.record_path),
        );
        source_origin::row(ui, texts, RecordText::Name, origin.map(|proof| &proof.name));
    });
    let response = ui.interact(
        header,
        ui.scope_id().with("record-heading-origins"),
        Sense::hover(),
    );
    tooltip::hover(response, |ui| {
        source_origin::row(
            ui,
            texts,
            RecordText::Type,
            origin.map(|proof| &proof.type_name),
        );
        source_origin::row(ui, texts, RecordText::Guid, origin.map(|proof| &proof.guid));
    });
}

pub(super) fn line_origins(
    ui: &mut Ui,
    texts: &Texts,
    view: &RecordView,
    (index, field): (usize, &FieldLine),
) {
    let proof = view
        .provenance()
        .and_then(Result::ok)
        .and_then(|proof| proof.lines().get(index));
    source_origin::row(
        ui,
        texts,
        RecordChangeText::HeadField,
        proof.map(|proof| &proof.name),
    );
    source_origin::row(
        ui,
        texts,
        RecordText::HeadValue,
        proof.map(|proof| &proof.value),
    );
    let annotation: Option<Text> = match field.value() {
        LineValue::Key { text, .. } if !matches!(text, KeyText::None) => {
            Some(RecordLanguageText::Menu.into())
        }
        LineValue::Reference(link) if link.tag().is_some() => Some(TagsText::ColumnTag.into()),
        LineValue::Reference(_) => Some(RecordText::Type.into()),
        _ => None,
    };
    if let Some(label) = annotation {
        source_origin::row(
            ui,
            texts,
            label,
            proof.and_then(|proof| proof.annotation.as_ref()),
        );
    }
}

/// Shows the XML tab for the selected Record: its Record XML, or where reading it stands.
pub(super) fn show_xml(ui: &mut Ui, texts: &Texts, browser: &Browser) {
    let (path, name) = heading(browser);
    path_line(ui, path, name);
    if let Some(view) = status_body(ui, texts, browser, path) {
        frames::preview_body().show(ui, |ui| match view.xml_error() {
            Some(reason) => {
                let reason = ("reason", FluentValue::from(reason.to_owned()));
                let path = ("path", FluentValue::from(path.to_owned()));
                let text = texts.format(RecordText::XmlFailed, &[path, reason]);
                ui.colored_label(palette::DANGER_TEXT, text);
            }
            None => xml_view::show(ui, texts, view.xml(), None),
        });
    }
}

/// Returns the Record XML of the selected Record once it was read.
pub(super) fn xml_bytes(browser: &Browser) -> Option<&[u8]> {
    browser
        .record_preview()
        .and_then(|preview| preview.view())
        .filter(|view| view.xml_error().is_none())
        .map(|view| view.xml().bytes())
}

pub(super) fn heading(browser: &Browser) -> (&str, &str) {
    browser
        .selected_record()
        .and_then(|id| browser.records()?.record(id))
        .map_or(("", ""), |record| (record.record_path(), record.name()))
}

/// Shows where reading the selected Record stands unless it was read; returns it once read.
fn status_body<'b>(
    ui: &mut Ui,
    texts: &Texts,
    browser: &'b Browser,
    path: &str,
) -> Option<&'b RecordView> {
    let status = browser
        .record_preview()
        .map_or(RecordPreviewStatus::Loading, |preview| preview.status());
    if let RecordPreviewStatus::Ready(view) = status {
        return Some(view);
    }
    let path_arg = || ("path", FluentValue::from(path.to_owned()));
    frames::preview_body()
        .show(ui, |ui| match status {
            RecordPreviewStatus::Ready(view) => Some(view),
            RecordPreviewStatus::Loading => {
                ui.horizontal(|ui| {
                    spinner(ui, SPINNER_INLINE);
                    ui.label(texts.format(RecordText::Loading, &[path_arg()]));
                });
                None
            }
            RecordPreviewStatus::Failed(reason) => {
                let text = reason.map_or_else(
                    || texts.format(RecordText::Stopped, &[path_arg()]),
                    |reason| {
                        let reason = ("reason", FluentValue::from(reason.to_owned()));
                        texts.format(RecordText::Failed, &[path_arg(), reason])
                    },
                );
                ui.colored_label(palette::DANGER_TEXT, text);
                None
            }
            RecordPreviewStatus::ArchiveChanged => {
                ui.colored_label(palette::WARNING, texts.get(RecordText::ArchiveChanged));
                None
            }
        })
        .inner
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct FactsKey {
    generation: u64,
    language: Language,
    pixels_per_point: f32,
}

fn facts(ui: &Ui, texts: &Texts, generation: u64, view: &RecordView) -> Arc<FactsStrip> {
    let id = Id::unique("p4kx-record-facts");
    let key = FactsKey {
        generation,
        language: texts.language(),
        pixels_per_point: ui.ctx().pixels_per_point(),
    };
    if let Some((kept, strip)) = ui.data(|data| data.get_temp::<(FactsKey, Arc<FactsStrip>)>(id))
        && kept == key
    {
        return strip;
    }
    let guid = view.guid().to_string();
    let [type_key, guid_key, name_key] =
        [RecordText::Type, RecordText::Guid, RecordText::Name].map(|text| texts.get(text));
    let laid = [
        Fact {
            key: &type_key,
            value: view.type_name(),
        },
        Fact {
            key: &guid_key,
            value: &guid,
        },
        Fact {
            key: &name_key,
            value: view.name(),
        },
    ];
    let strip = Arc::new(FactsStrip::lay_out(ui, &laid));
    ui.data_mut(|data| data.insert_temp(id, (key, Arc::clone(&strip))));
    strip
}
