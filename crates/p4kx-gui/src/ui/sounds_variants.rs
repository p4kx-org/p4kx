// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use egui::Ui;

use super::sound_grid::{self, Cell, Grid, Laid, Row, Tint, Width};
use crate::theme::desktop::ROW_HEIGHT;
use crate::theme::sound_metrics::{
    SOUND_CODEC_COLUMN, SOUND_ID_COLUMN, SOUND_LANGUAGE_COLUMN, SOUND_NAME_COLUMN,
    SOUNDBANK_VISIBLE_ROWS,
};
use crate::{Language, Playable, SoundList, SoundsText, SourceKind, Texts, Variant};

const WIDTHS: [Width; 5] = [
    Width::Initial(SOUND_NAME_COLUMN),
    Width::Initial(SOUND_ID_COLUMN),
    Width::Initial(SOUND_CODEC_COLUMN),
    Width::Initial(SOUND_LANGUAGE_COLUMN),
    Width::Rest,
];

/// Returns the name of the kind of a source.
pub(super) fn source_name(texts: &Texts, kind: SourceKind) -> String {
    let text = match kind {
        SourceKind::Embedded => SoundsText::SourceEmbedded,
        SourceKind::PrefetchStreamed => SoundsText::SourcePrefetch,
        SourceKind::Streamed => SoundsText::SourceStreamed,
        SourceKind::Plugin => SoundsText::SourcePlugin,
        SourceKind::External => SoundsText::SourceExternal,
        SourceKind::Midi => SoundsText::SourceMidi,
        _ => SoundsText::SourceOther,
    };
    texts.get(text)
}

fn language(texts: &Texts, variant: &Variant) -> String {
    match (&variant.language, variant.fallback) {
        (Some(language), true) => texts.format(
            SoundsText::LanguageFallback,
            &[("language", language.clone().into())],
        ),
        (Some(language), false) => language.clone(),
        (None, _) => String::new(),
    }
}

fn lay(texts: &Texts, list: &SoundList) -> Laid {
    let rows = list
        .variants
        .iter()
        .map(|variant| {
            let id = variant.wem_id.map_or_default(|id| id.to_string());
            let tint = match variant.playable() {
                Playable::Yes => Tint::Normal,
                Playable::PrefetchOnly
                | Playable::Plugin
                | Playable::External
                | Playable::Midi
                | Playable::Missing => Tint::Muted,
            };
            Row {
                name: texts.format(
                    SoundsText::VariantRow,
                    &[
                        ("name", variant.event.clone().into()),
                        ("bank", variant.soundbank.clone().into()),
                        ("id", id.clone().into()),
                    ],
                ),
                cells: vec![
                    Cell::new(variant.event.clone(), tint),
                    Cell::new(id, tint),
                    Cell::new(source_name(texts, variant.kind), tint),
                    Cell::new(language(texts, variant), tint),
                    Cell::new(variant.soundbank.clone(), tint),
                ],
            }
        })
        .collect();
    Laid {
        heads: vec![
            texts.get(SoundsText::ColumnEvent),
            texts.get(SoundsText::ColumnWem),
            texts.get(SoundsText::ColumnSource),
            texts.get(SoundsText::ColumnLanguage),
            texts.get(SoundsText::ColumnSoundbank),
        ],
        rows,
    }
}

#[derive(Clone, Copy, PartialEq)]
struct Key {
    serial: u64,
    language: Language,
}

/// Shows the variants of `list` as a table of event, WEM ID, source, language and soundbank,
/// `selected` highlighted and the table `room` tall, but never below the rows that stay visible; returns the position of the row that was clicked.
pub(super) fn show(
    ui: &mut Ui,
    texts: &Texts,
    (list, serial): (&SoundList, u64),
    (selected, room): (Option<usize>, f32),
) -> Option<usize> {
    let key = Key {
        serial,
        language: texts.language(),
    };
    let laid = sound_grid::cached(ui, "sound-variants", key, || lay(texts, list));
    let grid = Grid {
        salt: "sound-variants",
        label: texts.get(SoundsText::ScrollVariants),
        widths: &WIDTHS,
        laid: &laid,
        selected,
        height: Some(room.max(ROW_HEIGHT * (1.0 + SOUNDBANK_VISIBLE_ROWS))),
    };
    sound_grid::show(ui, &grid)
}
