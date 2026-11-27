// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use egui::{Color32, Galley, Id, Response, TextStyle, TextWrapMode, Ui, WidgetText};
use fluent_bundle::FluentValue;

use super::preview::path_line;
use super::record_lines::{self, List};
use super::record_preview::{RecordStep, heading};
use super::source_origin;
use crate::theme::desktop::SPINNER_INLINE;
use crate::theme::palette;
use crate::theme::tokens::{FS_SM, Weight};
use crate::widgets::frames;
use crate::widgets::indicator::spinner;
use crate::widgets::text::plain;
use crate::widgets::tooltip;
use crate::{
    Browser, IncomingPreview, IncomingStatus, IncomingText, IncomingView, Language, RecordText,
    ScrollText, Text, Texts, format,
};

const SALT: &str = "incoming-lines";
const HEADS: [Text; 2] = [
    Text::Incoming(IncomingText::HeadField),
    Text::Incoming(IncomingText::HeadRecord),
];

/// What a laid-out note was made for; a note is laid out again when any of it changes.
#[derive(Debug, Clone, Copy, PartialEq)]
struct NoteKey {
    generation: u64,
    language: Language,
    pixels_per_point: f32,
    width: f32,
    state: u8,
}

/// Shows the "Referenced by" tab for the selected Record: its record path, the count line and
/// one line per Reference with a link to the Record that holds it; returns the step a link asks
/// for.
pub(super) fn show(ui: &mut Ui, texts: &Texts, browser: &Browser) -> Option<RecordStep> {
    let (path, name) = heading(browser);
    path_line(ui, path, name);
    let preview = browser.incoming_preview();
    let status = preview.map_or(IncomingStatus::Loading, IncomingPreview::status);
    let generation = preview.map_or(0, IncomingPreview::generation);
    if let IncomingStatus::Ready(view) = status {
        return ready(ui, texts, view, generation);
    }
    frames::preview_body().show(ui, |ui| match status {
        IncomingStatus::Loading => {
            ui.horizontal(|ui| {
                spinner(ui, SPINNER_INLINE);
                let text = || {
                    let path = FluentValue::from(path.to_owned());
                    (
                        texts.format(IncomingText::Loading, &[("path", path)]),
                        palette::GRAY_1,
                    )
                };
                note(ui, (texts, generation, 0), text);
            });
        }
        IncomingStatus::Failed(reason) => {
            let text = || {
                let path = ("path", FluentValue::from(path.to_owned()));
                let text = reason.map_or_else(
                    || texts.format(IncomingText::Stopped, std::slice::from_ref(&path)),
                    |reason| {
                        let reason = ("reason", FluentValue::from(reason.to_owned()));
                        texts.format(IncomingText::Failed, &[path.clone(), reason])
                    },
                );
                (text, palette::DANGER_TEXT)
            };
            note(ui, (texts, generation, 1), text);
        }
        IncomingStatus::ArchiveChanged => {
            let text = || (texts.get(IncomingText::ArchiveChanged), palette::WARNING);
            note(ui, (texts, generation, 2), text);
        }
        IncomingStatus::Ready(_) => {}
    });
    None
}

fn ready(ui: &mut Ui, texts: &Texts, view: &IncomingView, generation: u64) -> Option<RecordStep> {
    let empty = view.lines().is_empty();
    frames::preview_body().show(ui, |ui| {
        if empty {
            let none = || (texts.get(IncomingText::None), palette::GRAY_2_TEXT);
            let response = note(ui, (texts, generation, 3), none);
            count_origin(response, texts, view);
            return;
        }
        let counts = note(ui, (texts, generation, 4), || {
            (summary(texts, view), palette::GRAY_1)
        });
        count_origin(counts, texts, view);
        if view.is_truncated() {
            note(ui, (texts, generation, 5), || {
                let shown = ("shown", count(texts, view.lines().len()).into());
                let total = ("total", count(texts, view.references()).into());
                (
                    texts.format(IncomingText::Truncated, &[shown, total]),
                    palette::GRAY_2_TEXT,
                )
            });
        }
    });
    if view.is_incomplete() {
        frames::preview_body().show(ui, |ui| {
            frames::warning_alert(ui, |ui| {
                note(ui, (texts, generation, 6), || {
                    (texts.get(IncomingText::Incomplete), palette::GRAY_1)
                });
            });
        });
    }
    if empty {
        return None;
    }
    let list = List {
        lines: view.lines(),
        source: None,
        incoming: Some(view),
        heads: HEADS,
        follow: Text::Incoming(IncomingText::Follow),
        scroll: ScrollText::Incoming,
        salt: SALT,
    };
    record_lines::show_list(ui, texts, &list, generation)
}

/// Shows the note `make` writes, laid out once per Record, language, scale, width and `state`.
fn note(
    ui: &mut Ui,
    (texts, generation, state): (&Texts, u64, u8),
    make: impl FnOnce() -> (String, Color32),
) -> Response {
    let key = NoteKey {
        generation,
        language: texts.language(),
        pixels_per_point: ui.ctx().pixels_per_point(),
        width: ui.available_width(),
        state,
    };
    let id = Id::unique(("p4kx-incoming-note", state));
    let kept = ui.data(|data| data.get_temp::<(NoteKey, Arc<Galley>)>(id));
    let galley =
        if let Some((kept, galley)) = kept
            && kept == key
        {
            galley
        } else {
            let (text, colour) = make();
            let laid = WidgetText::from(plain(text, FS_SM, Weight::Regular).color(colour))
                .into_galley(ui, Some(TextWrapMode::Wrap), key.width, TextStyle::Body);
            ui.data_mut(|data| data.insert_temp(id, (key, Arc::clone(&laid))));
            laid
        };
    ui.label(WidgetText::Galley(galley))
}

fn count_origin(response: Response, texts: &Texts, view: &IncomingView) {
    tooltip::hover(response, |ui| {
        tooltip::line(ui, texts.get(source_origin::label(view.count_origin())));
    });
}

pub(super) fn line_origins(ui: &mut Ui, texts: &Texts, view: &IncomingView, index: usize) {
    let proof = view.line_origins().and_then(|lines| lines.get(index));
    source_origin::row(
        ui,
        texts,
        IncomingText::HeadField,
        proof.map(|row| &row.name),
    );
    source_origin::row(
        ui,
        texts,
        IncomingText::HeadRecord,
        proof.map(|row| &row.value),
    );
    source_origin::row(
        ui,
        texts,
        RecordText::Type,
        proof.and_then(|row| row.annotation.as_ref()),
    );
}

fn summary(texts: &Texts, view: &IncomingView) -> String {
    let counted = |text: IncomingText, count: usize| {
        let count = u64::try_from(count).unwrap_or(u64::MAX);
        let args = [
            ("count", FluentValue::from(count)),
            ("number", format::number(count, texts.language()).into()),
        ];
        texts.format(text, &args)
    };
    let references = counted(IncomingText::ReferenceCount, view.references());
    let records = counted(IncomingText::RecordCount, view.records());
    texts.format(
        IncomingText::Summary,
        &[
            ("references", references.into()),
            ("records", records.into()),
        ],
    )
}

fn count(texts: &Texts, count: usize) -> String {
    format::number(u64::try_from(count).unwrap_or(u64::MAX), texts.language())
}
