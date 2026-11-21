// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use egui::Ui;
use fluent_bundle::FluentValue;

use super::audio_preview::{self, AudioAction, Named};
use super::soundbank_table;
use crate::theme::desktop::SPINNER_INLINE;
use crate::theme::palette;
use crate::widgets::frames;
use crate::widgets::indicator::spinner;
use crate::{
    AudioText, BankSound, PREVIEW_SIZE_LIMIT, SoundbankPreview, SoundbankStatus, Text, Texts,
    format,
};

/// What the user did in the Audio tab of a soundbank.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum BankAction {
    Select(u32),
    Audio(AudioAction),
}

/// Returns the size line of the soundbank of size bytes once it is parsed: the size and the
/// number of WEMs it embeds.
pub(super) fn summary(texts: &Texts, bank: &SoundbankPreview, size: u64) -> Option<String> {
    let SoundbankStatus::Opened(sounds) = bank.status() else {
        return None;
    };
    let count = u64::try_from(sounds.len()).unwrap_or(u64::MAX);
    let args = [
        ("size", format::size(size, texts.language()).into()),
        ("count", FluentValue::from(count)),
        ("number", format::number(count, texts.language()).into()),
    ];
    Some(texts.format(AudioText::BankSummary, &args))
}

/// Shows the Audio tab of the soundbank at `path` of `size` bytes: the controls of the selected
/// WEM at `volume` above the list of the WEMs it embeds, or the message why there is no list.
/// Returns what the user did.
pub(super) fn show(
    ui: &mut Ui,
    texts: &Texts,
    (path, size): (&str, u64),
    bank: &SoundbankPreview,
    volume: f32,
) -> Option<BankAction> {
    ui.set_min_height(ui.available_height());
    let path_arg = || ("path", FluentValue::from(path.to_owned()));
    let language = texts.language();
    match bank.status() {
        SoundbankStatus::Opening => {
            frames::preview_body().show(ui, |ui| {
                ui.horizontal(|ui| {
                    spinner(ui, SPINNER_INLINE);
                    ui.label(texts.format(Text::PreviewLoading, &[path_arg()]));
                });
            });
            None
        }
        SoundbankStatus::TooLarge => {
            let args = [
                path_arg(),
                ("size", format::size(size, language).into()),
                ("limit", format::size(PREVIEW_SIZE_LIMIT, language).into()),
            ];
            frames::preview_body().show(ui, |ui| {
                ui.label(texts.format(Text::PreviewTooLarge, &args));
            });
            None
        }
        SoundbankStatus::Failed(reason) => {
            let text = reason.map_or_else(
                || texts.format(Text::PreviewStopped, &[path_arg()]),
                |reason| {
                    let reason = ("reason", FluentValue::from(reason.to_owned()));
                    texts.format(AudioText::BankFailed, &[path_arg(), reason])
                },
            );
            frames::preview_body().show(ui, |ui| {
                ui.colored_label(palette::DANGER_TEXT, text);
            });
            None
        }
        SoundbankStatus::ArchiveChanged => {
            frames::preview_body().show(ui, |ui| {
                ui.colored_label(palette::WARNING, texts.get(Text::PreviewArchiveChanged));
            });
            None
        }
        SoundbankStatus::Opened([]) => {
            frames::preview_body().show(ui, |ui| {
                ui.label(texts.format(AudioText::BankEmpty, &[path_arg()]));
            });
            None
        }
        SoundbankStatus::Opened(sounds) => opened(ui, texts, (path, bank, sounds), volume),
    }
}

fn opened(
    ui: &mut Ui,
    texts: &Texts,
    (path, bank, sounds): (&str, &SoundbankPreview, &[BankSound]),
    volume: f32,
) -> Option<BankAction> {
    let audio = selected(ui, texts, (path, bank), volume);
    let picked = soundbank_table::show(ui, texts, bank, sounds);
    picked
        .map(BankAction::Select)
        .or_else(|| audio.map(BankAction::Audio))
}

/// Shows the facts and controls of the selected WEM, or the note that it is only prefetched.
fn selected(
    ui: &mut Ui,
    texts: &Texts,
    (path, bank): (&str, &SoundbankPreview),
    volume: f32,
) -> Option<AudioAction> {
    let sound = bank.selected_sound()?;
    let id = sound.id().to_string();
    if !sound.is_playable() {
        let text = if sound.is_prefetch() {
            AudioText::PrefetchedNote
        } else if sound.is_midi() {
            AudioText::MidiNote
        } else {
            AudioText::PluginNote
        };
        let note = texts.format(text, &[("id", id.into())]);
        frames::preview_body().show(ui, |ui| {
            ui.colored_label(palette::WARNING, note);
        });
        return None;
    }
    let name = texts.format(
        AudioText::WemName,
        &[("id", id.into()), ("bank", path.to_owned().into())],
    );
    let audio = bank.audio()?;
    let named = Named {
        path: &name,
        size: sound.size(),
        embedded: true,
    };
    audio_preview::show(ui, texts, named, audio, volume)
}
