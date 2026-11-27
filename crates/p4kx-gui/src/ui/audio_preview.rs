// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use egui::{Id, Sense, Ui};
use fluent_bundle::FluentValue;

use super::audio_controls;
use super::source_origin;
use crate::theme::desktop::SPINNER_INLINE;
use crate::theme::palette;
use crate::widgets::frames;
use crate::widgets::indicator::spinner;
use crate::widgets::summary::{Fact, FactsStrip};
use crate::widgets::tooltip;
use crate::{
    AudioPreview, AudioStatus, AudioText, Language, MAX_PREVIEW_PCM, Model, PREVIEW_SIZE_LIMIT,
    SoundInfo, SoundStatus, Text, Texts, WemHeaderProvenance, format,
};

/// What the user did with the controls of the Audio tab.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum AudioAction {
    Toggle,
    Seek(f32),
    Volume(f32),
}

/// Applies `action` to the playback of the selected WEM.
pub(super) fn apply(model: &mut Model, action: AudioAction) {
    match action {
        AudioAction::Toggle => model.toggle_playback(),
        AudioAction::Seek(fraction) => model.seek_playback(fraction),
        AudioAction::Volume(volume) => model.set_audio_volume(volume),
    }
}

/// The WEM an audio preview shows: its name in the messages, its size and whether it is embedded
/// in a soundbank rather than an Entry of its own.
#[derive(Debug, Clone, Copy)]
pub(super) struct Named<'a> {
    pub(super) path: &'a str,
    pub(super) size: u64,
    pub(super) embedded: bool,
}

/// Shows the Audio tab of the WEM `named`: its facts, the playback controls at `volume` and the
/// messages. Returns what the user did.
pub(super) fn show(
    ui: &mut Ui,
    texts: &Texts,
    named: Named<'_>,
    audio: &AudioPreview,
    volume: f32,
) -> Option<AudioAction> {
    let Named {
        path,
        size,
        embedded,
    } = named;
    let status = audio.status();
    if let AudioStatus::Opened(info, _) = status {
        let rect = facts(ui, texts, audio.serial(), info).show(ui);
        if let Some(origins) = audio.header_provenance() {
            let response = ui.interact(rect, ui.scope_id().with("wem-origins"), Sense::hover());
            tooltip::hover(response, |ui| header_origins(ui, texts, origins.ok()));
        }
    }
    let path_arg = || ("path", FluentValue::from(path.to_owned()));
    let language = texts.language();
    frames::preview_body()
        .show(ui, |ui| match status {
            AudioStatus::Opening => {
                loading(ui, &texts.format(Text::PreviewLoading, &[path_arg()]));
                None
            }
            AudioStatus::TooLarge => {
                let args = [
                    path_arg(),
                    ("size", format::size(size, language).into()),
                    ("limit", format::size(PREVIEW_SIZE_LIMIT, language).into()),
                ];
                ui.label(texts.format(Text::PreviewTooLarge, &args));
                None
            }
            AudioStatus::Failed(reason) => {
                ui.colored_label(palette::DANGER_TEXT, failure(texts, path, reason));
                None
            }
            AudioStatus::ArchiveChanged => {
                ui.colored_label(palette::WARNING, texts.get(Text::PreviewArchiveChanged));
                None
            }
            AudioStatus::Opened(_, sound) => {
                sound_body(ui, texts, (path, embedded), (audio, sound), volume)
            }
        })
        .inner
}

fn header_origins(ui: &mut Ui, texts: &Texts, origins: Option<&WemHeaderProvenance>) {
    for (label, origin) in [
        (AudioText::Codec, origins.map(|source| &source.codec)),
        (AudioText::Channels, origins.map(|source| &source.channels)),
        (
            AudioText::SampleRate,
            origins.map(|source| &source.sample_rate),
        ),
        (AudioText::Duration, origins.map(|source| &source.duration)),
    ] {
        source_origin::row(ui, texts, label, origin);
    }
}

fn sound_body(
    ui: &mut Ui,
    texts: &Texts,
    (path, embedded): (&str, bool),
    (audio, sound): (&AudioPreview, SoundStatus<'_>),
    volume: f32,
) -> Option<AudioAction> {
    let path_arg = || ("path", FluentValue::from(path.to_owned()));
    match sound {
        SoundStatus::Decoding => {
            loading(ui, &texts.format(AudioText::Decoding, &[path_arg()]));
            None
        }
        SoundStatus::Unsupported(codec) => {
            let codec = ("codec", FluentValue::from(codec.to_string()));
            let message = if embedded {
                AudioText::UnsupportedWem
            } else {
                AudioText::Unsupported
            };
            let args = if embedded {
                [codec, ("name", FluentValue::from(path.to_owned()))]
            } else {
                [codec, path_arg()]
            };
            let text = texts.format(message, &args);
            ui.colored_label(palette::DANGER_TEXT, text);
            None
        }
        SoundStatus::TooLong => {
            let limit = format::size(MAX_PREVIEW_PCM, texts.language());
            let args = [path_arg(), ("limit", limit.into())];
            ui.colored_label(
                palette::DANGER_TEXT,
                texts.format(AudioText::TooLong, &args),
            );
            None
        }
        SoundStatus::Failed(reason) => {
            ui.colored_label(palette::DANGER_TEXT, failure(texts, path, reason));
            None
        }
        SoundStatus::Ready(player) => {
            audio_controls::controls(ui, texts, audio.serial(), player, volume)
        }
    }
}

fn loading(ui: &mut Ui, text: &str) {
    ui.horizontal(|ui| {
        spinner(ui, SPINNER_INLINE);
        ui.label(text);
    });
}

fn failure(texts: &Texts, path: &str, reason: Option<&str>) -> String {
    let path_arg = || ("path", FluentValue::from(path.to_owned()));
    reason.map_or_else(
        || texts.format(Text::PreviewStopped, &[path_arg()]),
        |reason| {
            let reason = ("reason", FluentValue::from(reason.to_owned()));
            texts.format(AudioText::Failed, &[path_arg(), reason])
        },
    )
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Key {
    serial: u64,
    language: Language,
    pixels_per_point: f32,
}

/// Returns the facts of `info`, laid out once per audio preview, language and scale.
fn facts(ui: &Ui, texts: &Texts, serial: u64, info: &SoundInfo) -> Arc<FactsStrip> {
    let id = Id::unique("p4kx-audio-facts");
    let key = Key {
        serial,
        language: texts.language(),
        pixels_per_point: ui.ctx().pixels_per_point(),
    };
    if let Some((cached, strip)) = ui.data(|data| data.get_temp::<(Key, Arc<FactsStrip>)>(id))
        && cached == key
    {
        return strip;
    }
    let language = texts.language();
    let codec = info.codec.to_string();
    let channels = info.channels.to_string();
    let rate = format::number(u64::from(info.sample_rate), language);
    let rate = texts.format(AudioText::Hertz, &[("rate", rate.into())]);
    let duration = info.duration.map_or_else(
        || texts.get(AudioText::Unknown),
        |duration| audio_controls::clock(duration, true),
    );
    let keys = [
        AudioText::Codec,
        AudioText::Channels,
        AudioText::SampleRate,
        AudioText::Duration,
    ]
    .map(|text| texts.get(text));
    let values = [&codec, &channels, &rate, &duration];
    let laid: Vec<Fact<'_>> = keys
        .iter()
        .zip(values)
        .map(|(key, value)| Fact { key, value })
        .collect();
    let strip = Arc::new(FactsStrip::lay_out(ui, &laid));
    ui.data_mut(|data| data.insert_temp(id, (key, Arc::clone(&strip))));
    strip
}
