// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use egui::scroll_area::ScrollAreaOutput;
use egui::{Align, Id, Label, Layout, Response, Role, Sense, Ui, Vec2, WidgetInfo, pos2, vec2};
use egui_extras::{Column, TableBuilder, TableRow};

use super::audio_controls::clock;
use super::regions;
use super::soundbank_origins;
use super::table::{cell_text, head_fill};
use crate::theme::desktop::{
    CELL_PADDING_X, ROW_FONT, ROW_HEIGHT, TABLE_HEAD_SIZE, TABLE_HEAD_TRACKING,
};
use crate::theme::palette;
use crate::theme::sound_metrics::{
    SOUND_CODEC_COLUMN, SOUND_COLUMN_MIN, SOUND_DURATION_COLUMN, SOUND_ID_COLUMN,
};
use crate::theme::tokens::Weight;
use crate::widgets::rows::{RowState, focus_inside, row_weight};
use crate::widgets::scroll_region::{self, Stand};
use crate::widgets::table_row::RowSlots;
use crate::widgets::text::{caps, line, plain};
use crate::{AudioText, BankSound, Language, ScrollText, SoundbankPreview, Text, Texts, format};

/// The shown texts of one row, and whether the codec and the duration stand for a missing value
/// and are shown muted.
struct Row {
    id: String,
    codec: String,
    codec_muted: bool,
    duration: String,
    duration_muted: bool,
    size: String,
    name: String,
}

/// The texts of the table, formatted once per soundbank and language.
struct Laid {
    heads: [String; 4],
    rows: Vec<Row>,
}

#[derive(Clone, Copy, PartialEq)]
struct Key {
    serial: u64,
    language: Language,
}

impl Laid {
    fn of(texts: &Texts, sounds: &[BankSound]) -> Self {
        let unknown = texts.get(AudioText::Unknown);
        let prefetched = texts.get(AudioText::Prefetched);
        let plugin = texts.get(AudioText::Plugin);
        let midi = texts.get(AudioText::Midi);
        let rows = sounds
            .iter()
            .map(|sound| {
                let info = sound.info();
                let (codec, codec_muted) = if sound.is_prefetch() {
                    (prefetched.clone(), true)
                } else if sound.is_plugin() {
                    (plugin.clone(), true)
                } else if sound.is_midi() {
                    (midi.clone(), true)
                } else {
                    info.map_or_else(
                        || (unknown.clone(), true),
                        |info| (info.codec.to_string(), false),
                    )
                };
                let (duration, duration_muted) = info.and_then(|info| info.duration).map_or_else(
                    || (unknown.clone(), true),
                    |duration| (clock(duration, true), false),
                );
                let id = sound.id().to_string();
                let args = [
                    ("id", id.clone().into()),
                    ("codec", codec.clone().into()),
                    ("duration", duration.clone().into()),
                ];
                Row {
                    name: texts.format(AudioText::WemRow, &args),
                    size: format::size(sound.size(), texts.language()),
                    id,
                    codec,
                    codec_muted,
                    duration,
                    duration_muted,
                }
            })
            .collect();
        Self {
            heads: [
                texts.get(AudioText::ColumnId),
                texts.get(AudioText::Codec),
                texts.get(AudioText::Duration),
                texts.get(Text::ColumnSize),
            ],
            rows,
        }
    }
}

/// Returns the texts of the table of the soundbank `serial`, formatted again only when the
/// language changes.
fn cached(ui: &Ui, texts: &Texts, serial: u64, sounds: &[BankSound]) -> Arc<Laid> {
    let id = Id::unique("p4kx-soundbank-texts");
    let key = Key {
        serial,
        language: texts.language(),
    };
    if let Some((cached, laid)) = ui.data(|data| data.get_temp::<(Key, Arc<Laid>)>(id))
        && cached == key
    {
        return laid;
    }
    let laid = Arc::new(Laid::of(texts, sounds));
    ui.data_mut(|data| data.insert_temp(id, (key, Arc::clone(&laid))));
    laid
}

/// What the user did in the table in this frame.
#[derive(Default)]
struct Outcome {
    clicked: Option<u32>,
    hovered: Option<usize>,
}

/// What the rows of the table are made of.
struct Rows<'a> {
    laid: &'a Laid,
    sounds: &'a [BankSound],
    bank: &'a SoundbankPreview,
    texts: &'a Texts,
    selected: Option<u32>,
    hovered: Option<usize>,
    left: f32,
}

/// Shows the WEMs of the soundbank `serial` as a virtual table of ID, codec, duration and size,
/// `selected` highlighted; returns the ID of the WEM that was clicked.
pub(super) fn show(
    ui: &mut Ui,
    texts: &Texts,
    bank: &SoundbankPreview,
    sounds: &[BankSound],
) -> Option<u32> {
    let selected = bank.selected();
    let region = scroll_region::begin(
        ui,
        (ui.scope_id().with("soundbank-region"), || {
            texts.get(ScrollText::Soundbank)
        }),
        ROW_HEIGHT,
    );
    let hovered_id = ui.scope_id().with("soundbank-hovered-row");
    let hovered = ui.data(|data| data.get_temp::<usize>(hovered_id));
    let reveal = unrevealed_row(ui, sounds, selected);
    let laid = cached(ui, texts, bank.serial(), sounds);
    ui.spacing_mut().item_spacing = Vec2::ZERO;
    let left = ui.max_rect().left();
    let mut table = builder(ui);
    if let Some(offset) = region.requested() {
        table = table.vertical_scroll_offset(offset.y);
    }
    if let Some(row) = reveal {
        table = table.scroll_to_row(row, None);
    }
    let rows = Rows {
        laid: &laid,
        sounds,
        bank,
        texts,
        selected,
        hovered,
        left,
    };
    let mut outcome = Outcome::default();
    let output = grid(table, &rows, &mut outcome);
    let stand = Stand {
        offset: output.state.clamped_offset(),
        view: output.inner_rect.size(),
        content: output.content_size,
    };
    region.finish(ui, output.inner_rect, stand);
    ui.data_mut(|data| match outcome.hovered {
        Some(index) => {
            data.insert_temp(hovered_id, index);
        }
        None => {
            data.remove_temp::<usize>(hovered_id);
        }
    });
    outcome.clicked
}

fn grid(table: TableBuilder<'_>, rows: &Rows<'_>, outcome: &mut Outcome) -> ScrollAreaOutput<()> {
    table
        .header(ROW_HEIGHT, |mut header| {
            for (index, label) in rows.laid.heads.iter().enumerate() {
                header.col(|ui| {
                    let from = if index == 0 {
                        rows.left
                    } else {
                        ui.max_rect().left()
                    };
                    head_cell(ui, label, from);
                });
            }
        })
        .body(|body| {
            regions::rows(body, ROW_HEIGHT, rows.sounds.len(), |row| {
                let index = row.index();
                let (Some(sound), Some(texts)) =
                    (rows.sounds.get(index), rows.laid.rows.get(index))
                else {
                    return;
                };
                let active = rows.selected == Some(sound.id());
                let state = RowState::of(active, false, rows.hovered == Some(index));
                let response = show_row(row, texts, state);
                if response.clicked() {
                    outcome.clicked = Some(sound.id());
                }
                if response.hovered() {
                    outcome.hovered = Some(index);
                }
                soundbank_origins::row(response, rows.texts, rows.bank, sound.id());
            });
        })
}

/// Returns the row of `selected` when it was not selected in the previous frame, so the table
/// scrolls it into view once.
fn unrevealed_row(ui: &Ui, sounds: &[BankSound], selected: Option<u32>) -> Option<usize> {
    let id = ui.scope_id().with("soundbank-revealed");
    let revealed = ui.data(|data| data.get_temp::<Option<u32>>(id));
    if revealed == Some(selected) {
        return None;
    }
    ui.data_mut(|data| data.insert_temp(id, selected));
    let wanted = selected?;
    sounds.binary_search_by_key(&wanted, BankSound::id).ok()
}

fn builder(ui: &mut Ui) -> TableBuilder<'_> {
    TableBuilder::new(ui)
        .id_salt("soundbank-sounds")
        .striped(false)
        .resizable(true)
        .auto_shrink(false)
        .min_scrolled_height(0.0)
        .sense(Sense::click())
        .cell_layout(Layout::left_to_right(Align::Center))
        .column(Column::exact(SOUND_ID_COLUMN))
        .column(
            Column::initial(SOUND_CODEC_COLUMN)
                .at_least(SOUND_COLUMN_MIN)
                .clip(true),
        )
        .column(
            Column::initial(SOUND_DURATION_COLUMN)
                .at_least(SOUND_COLUMN_MIN)
                .clip(true),
        )
        .column(Column::remainder().at_least(SOUND_COLUMN_MIN).clip(true))
}

fn head_cell(ui: &Ui, label: &str, left: f32) {
    head_fill(ui, left);
    let rect = ui.max_rect();
    let response = ui.interact(
        rect,
        ui.scope_id().with(("soundbank-head", label)),
        Sense::hover(),
    );
    response.widget_info(|| WidgetInfo::labeled(Role::ColumnHeader, true, label));
    let inner = rect.shrink2(vec2(CELL_PADDING_X, 0.0));
    let text = caps(label, TABLE_HEAD_SIZE, Weight::Bold, TABLE_HEAD_TRACKING);
    let galley = line(ui, text, inner.width());
    ui.painter()
        .with_clip_rect(inner.intersect(ui.clip_rect()))
        .galley(
            pos2(inner.left(), inner.center().y - galley.size().y / 2.0),
            galley,
            palette::GRAY_1,
        );
}

fn show_row(mut row: TableRow<'_, '_>, texts: &Row, state: RowState) -> Response {
    row.set_hovered(false);
    let (_, colour) = state.colours();
    let active = state == RowState::Active;
    let weight = row_weight(active);
    let index = row.index();
    let tint = |muted: bool| if muted { palette::GRAY_2_TEXT } else { colour };
    let mut slots = None;
    row.col(|ui| {
        slots = Some(RowSlots::reserve(ui));
        cell_text(ui, &texts.id, colour, weight);
    });
    row.col(|ui| cell_text(ui, &texts.codec, tint(texts.codec_muted), weight));
    row.col(|ui| cell_text(ui, &texts.duration, tint(texts.duration_muted), weight));
    row.col(|ui| {
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.add_space(CELL_PADDING_X);
            ui.add(
                Label::new(plain(&texts.size, ROW_FONT, weight).color(colour)).selectable(false),
            );
        });
    });
    let response = row.response();
    if let Some(slots) = slots {
        slots.paint(&response, state, index % 2 == 1);
    }
    response.widget_info(|| WidgetInfo::selected(Role::Row, true, active, &texts.name));
    focus_inside(
        &response.ctx.layer_painter(response.layer_id),
        response.rect,
        &response,
    );
    response
}
