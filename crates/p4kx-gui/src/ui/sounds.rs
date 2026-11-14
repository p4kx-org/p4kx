// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use egui::{Color32, Frame, Label, Margin, Panel, RichText, ScrollArea, Ui};
use fluent_bundle::FluentValue;

use super::audio_preview::{self, AudioAction, Named};
use super::record_export_outcome;
use super::sounds_note::player_note;
use super::{RESIZE_GAP, empty, list_footer, sounds_toolbar, sounds_triggers, sounds_variants};
use crate::theme::desktop::{
    CELL_PADDING_X, PREVIEW_HEIGHT, PREVIEW_MIN_HEIGHT, ROW_HEIGHT, SPINNER, SPINNER_INLINE,
    TOOLBAR_PADDING,
};
use crate::theme::palette;
use crate::theme::tokens::{FS_SM, Weight};
use crate::theme::window_metrics::SOUNDS_TOOLBAR_MIN_HEIGHT;
use crate::widgets::frames::{self, px};
use crate::widgets::indicator;
use crate::widgets::text::{hairline_below, plain};
use crate::{
    ListSource, ListStatus, Model, SoundList, SoundsListStatus, SoundsStatus, SoundsText, Texts,
    TriggerList, format,
};

/// Shows the Sounds tab: the Entity and search fields, the audio triggers they list and, below,
/// the sounds of the selected trigger with the player, or the state of indexing the audio.
pub(super) fn show(ui: &mut Ui, model: &mut Model) {
    if model.browser().is_none() {
        let texts = model.texts();
        let title = texts.get(SoundsText::EmptyTitle);
        let hint = texts.get(SoundsText::EmptyHint);
        empty::notice(ui, &title, &hint);
        return;
    }
    if !model.sounds().is_started() {
        let ctx = ui.ctx().clone();
        model.load_sounds(move || ctx.request_repaint());
    }
    let texts = model.texts();
    match model.sounds().status() {
        SoundsStatus::Idle | SoundsStatus::Building => {
            frames::panel().show(ui, |ui| loading(ui, texts, SoundsText::Loading));
        }
        SoundsStatus::Failed(reason) => {
            let text = reason.map_or_else(
                || texts.get(SoundsText::Stopped),
                |reason| texts.format(SoundsText::Failed, &[("reason", reason.to_owned().into())]),
            );
            frames::panel().show(ui, |ui| {
                frames::danger_alert(ui, |ui| {
                    ui.add(Label::new(text).selectable(true));
                });
            });
        }
        SoundsStatus::ArchiveChanged => {
            let text = texts.get(SoundsText::ArchiveChanged);
            frames::panel().show(ui, |ui| {
                frames::warning_alert(ui, |ui| {
                    ui.add(Label::new(text).selectable(true));
                });
            });
        }
        SoundsStatus::Ready(info) => {
            let issues = info.issues;
            ready(ui, model, issues);
        }
    }
}

fn ready(ui: &mut Ui, model: &mut Model, issues: usize) {
    let toolbar = frames::panel()
        .inner_margin(Margin::same(px(TOOLBAR_PADDING)))
        .show(ui, |ui| {
            ScrollArea::vertical()
                .id_salt("sounds-toolbar-scroll")
                .max_height((ui.available_height() / 3.0).max(SOUNDS_TOOLBAR_MIN_HEIGHT))
                .auto_shrink([false, true])
                .show(ui, |ui| sounds_toolbar::show(ui, model));
        });
    hairline_below(ui, toolbar.response.rect);
    if issues > 0 {
        self::issues(ui, model.texts(), issues);
    }
    let room = ROW_HEIGHT.mul_add(-4.0, ui.available_height()).max(0.0);
    let action = Panel::bottom("sounds-detail")
        .frame(frames::panel().outer_margin(Margin {
            top: RESIZE_GAP,
            ..Margin::ZERO
        }))
        .resizable(true)
        .default_size(PREVIEW_HEIGHT.min(room))
        .size_range(PREVIEW_MIN_HEIGHT.min(room)..=room)
        .show(ui, |ui| detail(ui, model))
        .inner;
    let picked = frames::panel()
        .show(ui, |ui| {
            ui.set_min_size(ui.available_size());
            triggers(ui, model)
        })
        .inner;
    if let Some(position) = picked {
        model.sounds_mut().select(position);
    }
    match action {
        Some(DetailAction::Audio(action)) => audio_preview::apply(model, action),
        Some(DetailAction::Select(position)) => {
            let ctx = ui.ctx().clone();
            model.select_sound_variant(position, move || ctx.request_repaint());
        }
        None => {}
    }
}

fn issues(ui: &mut Ui, texts: &Texts, count: usize) {
    let count = u64::try_from(count).unwrap_or(u64::MAX);
    let text = texts.format(
        SoundsText::Issues,
        &[
            ("count", FluentValue::from(count)),
            ("number", format::number(count, texts.language()).into()),
        ],
    );
    frames::warning_alert(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.add(Label::new(text).selectable(true));
    });
}

fn triggers(ui: &mut Ui, model: &Model) -> Option<usize> {
    let texts = model.texts();
    let tab = model.sounds();
    match tab.list_status() {
        ListStatus::Empty => message(ui, texts.get(SoundsText::ListHint), palette::GRAY_2_TEXT),
        ListStatus::Loading => loading(ui, texts, SoundsText::ListLoading),
        ListStatus::NotAnEntity => {
            frames::warning_alert(ui, |ui| {
                ui.label(texts.get(SoundsText::NotAnEntity));
            });
        }
        ListStatus::Failed(reason) => {
            let text = texts.format(
                SoundsText::ListFailed,
                &[("reason", reason.to_owned().into())],
            );
            frames::danger_alert(ui, |ui| {
                ui.add(Label::new(text).selectable(true));
            });
        }
        ListStatus::ArchiveChanged => {
            frames::warning_alert(ui, |ui| {
                ui.label(texts.get(SoundsText::ListArchiveChanged));
            });
        }
        ListStatus::Ready(list) => return ready_list(ui, model, list),
    }
    None
}

fn ready_list(ui: &mut Ui, model: &Model, list: &TriggerList) -> Option<usize> {
    let texts = model.texts();
    let tab = model.sounds();
    let footer = count(texts, list);
    list_footer::show(ui, "sounds-footer", |ui| {
        if tab.is_listing() {
            indicator::spinner(ui, SPINNER_INLINE);
        }
        ui.add(
            Label::new(plain(footer, FS_SM, Weight::Regular).color(palette::GRAY_2_TEXT))
                .selectable(false),
        );
    });
    if list.rows.is_empty() {
        let text = match list.source {
            ListSource::Entity(_) => SoundsText::NoTriggers,
            ListSource::Search(_) => SoundsText::NoMatch,
        };
        message(ui, texts.get(text), palette::GRAY_2_TEXT);
        return None;
    }
    sounds_triggers::show(ui, texts, (list, tab.list_serial()), tab.selected())
}

fn count(texts: &Texts, list: &TriggerList) -> String {
    let number =
        |value: usize| format::number(u64::try_from(value).unwrap_or(u64::MAX), texts.language());
    let shown = u64::try_from(list.rows.len()).unwrap_or(u64::MAX);
    match &list.source {
        ListSource::Entity(entity) => texts.format(
            SoundsText::CountEntity,
            &[
                ("name", entity.class_name.clone().into()),
                ("count", FluentValue::from(shown)),
                ("number", number(list.rows.len()).into()),
                ("resolved", number(list.matched_count()).into()),
            ],
        ),
        ListSource::Search(_) if list.total > list.rows.len() => texts.format(
            SoundsText::CountSearchMore,
            &[
                ("number", number(list.rows.len()).into()),
                ("total", number(list.total).into()),
            ],
        ),
        ListSource::Search(_) => texts.format(
            SoundsText::CountSearch,
            &[
                ("count", FluentValue::from(shown)),
                ("number", number(list.rows.len()).into()),
            ],
        ),
    }
}

enum DetailAction {
    Select(usize),
    Audio(AudioAction),
}

fn detail(ui: &mut Ui, model: &Model) -> Option<DetailAction> {
    let texts = model.texts();
    ScrollArea::vertical()
        .id_salt("sounds-detail-scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| detail_body(ui, model, texts))
        .inner
}

fn detail_body(ui: &mut Ui, model: &Model, texts: &Texts) -> Option<DetailAction> {
    match model.sounds().sounds_status() {
        SoundsListStatus::None => {
            message(ui, texts.get(SoundsText::SelectHint), palette::GRAY_2_TEXT);
        }
        SoundsListStatus::Loading => loading(ui, texts, SoundsText::VariantsLoading),
        SoundsListStatus::Undefined => {
            message(ui, texts.get(SoundsText::Undefined), palette::WARNING);
        }
        SoundsListStatus::Ready(list) => return sounds(ui, model, list),
    }
    None
}

fn sounds(ui: &mut Ui, model: &Model, list: &SoundList) -> Option<DetailAction> {
    let texts = model.texts();
    let tab = model.sounds();
    let total = u64::try_from(list.variants.len()).unwrap_or(u64::MAX);
    let heading = texts.format(
        SoundsText::VariantsCount,
        &[
            ("name", list.trigger.clone().into()),
            ("count", FluentValue::from(total)),
            ("number", format::number(total, texts.language()).into()),
        ],
    );
    padded(
        ui,
        plain(heading, FS_SM, Weight::Bold).color(palette::GRAY_1),
    );
    if !list.breaks.is_empty() {
        let breaks = u64::try_from(list.breaks.len()).unwrap_or(u64::MAX);
        let heading = texts.format(
            SoundsText::BreaksHeading,
            &[
                ("count", FluentValue::from(breaks)),
                ("number", format::number(breaks, texts.language()).into()),
            ],
        );
        let lines: Vec<String> = list
            .breaks
            .iter()
            .map(|link| {
                let mut args = vec![
                    ("name", link.event.clone().into()),
                    ("reason", link.reason.clone().into()),
                ];
                let text = link
                    .soundbank
                    .as_ref()
                    .map_or(SoundsText::BreakLine, |bank| {
                        args.push(("bank", bank.clone().into()));
                        SoundsText::BreakLineIn
                    });
                texts.format(text, &args)
            })
            .collect();
        record_export_outcome::list(ui, (&heading, "sound-breaks"), &lines, None, false);
    }
    if list.variants.is_empty() {
        message(ui, texts.get(SoundsText::NoVariants), palette::GRAY_2_TEXT);
        return None;
    }
    let audio = selected(ui, model, list);
    let room = ui.clip_rect().bottom() - ui.cursor().top();
    let picked = sounds_variants::show(
        ui,
        texts,
        (list, tab.sounds_serial()),
        (tab.selected_variant(), room),
    );
    picked
        .map(DetailAction::Select)
        .or_else(|| audio.map(DetailAction::Audio))
}

fn selected(ui: &mut Ui, model: &Model, list: &SoundList) -> Option<AudioAction> {
    let texts = model.texts();
    let tab = model.sounds();
    let Some(variant) = tab.selected_variant().and_then(|at| list.variants.get(at)) else {
        message(
            ui,
            texts.get(SoundsText::SelectVariant),
            palette::GRAY_2_TEXT,
        );
        return None;
    };
    let id = variant.wem_id.map_or_default(|id| id.to_string());
    let source = sounds_variants::source_name(texts, variant.kind);
    let detail = texts.format(
        SoundsText::Detail,
        &[
            ("path", variant.soundbank_path.clone().into()),
            ("id", id.clone().into()),
            ("source", source.into()),
        ],
    );
    padded(
        ui,
        plain(detail, FS_SM, Weight::Regular).color(palette::GRAY_2_TEXT),
    );
    let audio = tab.audio();
    if let Some(note) = player_note(variant.playable(), audio.is_some()) {
        message(ui, texts.get(note), palette::WARNING);
        return None;
    }
    let audio = audio?;
    let name = texts.format(
        crate::AudioText::WemName,
        &[
            ("id", id.into()),
            ("bank", variant.soundbank.clone().into()),
        ],
    );
    let named = Named {
        path: &name,
        size: variant.size,
        embedded: variant.is_embedded(),
    };
    audio_preview::show(ui, texts, named, audio, model.audio_volume())
}

/// Shows `text` with the inner padding of a table cell.
fn padded(ui: &mut Ui, text: RichText) {
    Frame::NONE
        .inner_margin(Margin::symmetric(px(CELL_PADDING_X), 0))
        .show(ui, |ui| {
            ui.label(text);
        });
}

fn loading(ui: &mut Ui, texts: &Texts, text: SoundsText) {
    ui.add_space(SPINNER);
    ui.horizontal(|ui| {
        indicator::spinner(ui, SPINNER_INLINE);
        ui.add(
            Label::new(plain(texts.get(text), FS_SM, Weight::Regular).color(palette::GRAY_1))
                .selectable(false),
        );
    });
}

fn message(ui: &mut Ui, text: String, colour: Color32) {
    frames::preview_body().show(ui, |ui| {
        ui.add(Label::new(
            plain(text, FS_SM, Weight::Regular).color(colour),
        ));
    });
}
