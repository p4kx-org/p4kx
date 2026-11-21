// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use egui::{
    CornerRadius, Frame, Id, Margin, Rect, Role, ScrollArea, Sense, Ui, WidgetInfo, pos2, vec2,
};
use p4kx_comparison::Side;

use super::status::build_label;
use crate::theme::desktop::{
    CELL_PADDING_X, COUNT_COLUMN, ROW_FONT, ROW_HEIGHT, SECTION_LIST_HEIGHT, TABLE_HEAD_SIZE,
    TABLE_HEAD_TRACKING, TOOLBAR_PADDING,
};
use crate::theme::palette;
use crate::theme::tokens::Weight;
use crate::widgets::chip::Tone;
use crate::widgets::frames::px;
use crate::widgets::rows::{paint_divider, section_header};
use crate::widgets::summary::{Fact, facts_bar, kpi_tile};
use crate::widgets::text::{caps, line, paint_left, paint_right, plain};
use crate::{ComparisonText, ComparisonView, Model, Summary, Texts, format};

/// The kinds of change in the order the summary shows them, with their tone.
pub(super) const KINDS: [(ComparisonText, Tone); 3] = [
    (ComparisonText::Added, Tone::Success),
    (ComparisonText::Removed, Tone::Danger),
    (ComparisonText::Changed, Tone::Warning),
];

/// Returns the text and the chip tone of `kind`.
pub(super) fn kind_look(kind: p4kx_comparison::ChangeKind) -> (ComparisonText, Tone) {
    KINDS
        .get(usize::from(crate::comparison::rank(kind)))
        .copied()
        .unwrap_or((ComparisonText::Changed, Tone::Warning))
}

/// The formatted texts of the summary, kept until the summary or the language changes.
#[derive(Debug, Default)]
struct SummaryTexts {
    old_key: String,
    old: String,
    new_key: String,
    new: String,
    kinds: [(String, String); 3],
    incomparable: Option<String>,
    directories_title: String,
    directory_head: [String; 4],
    directories: Vec<[String; 4]>,
}

/// Shows both sides, the counts per change kind and the counts per top-level directory.
pub(super) fn show(ui: &mut Ui, model: &Model) {
    let Some(view) = model.comparison().view() else {
        return;
    };
    let texts = model.texts();
    let summary = cached(ui, texts, view);
    facts_bar(
        ui,
        Fact {
            key: &summary.old_key,
            value: &summary.old,
        },
        Fact {
            key: &summary.new_key,
            value: &summary.new,
        },
    );
    Frame::NONE
        .inner_margin(Margin::same(px(TOOLBAR_PADDING)))
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing = vec2(TOOLBAR_PADDING, TOOLBAR_PADDING);
            let width = (2.0f32.mul_add(-TOOLBAR_PADDING, ui.available_width()) / 3.0).floor();
            ui.horizontal(|ui| {
                for ((label, value), (_, tone)) in summary.kinds.iter().zip(KINDS) {
                    kpi_tile(ui, label, value, tone, width);
                }
            });
            if let Some(message) = &summary.incomparable {
                ui.label(message);
            }
        });
    if section_header(
        ui,
        ui.scope_id().with("comparison-directories"),
        &summary.directories_title,
        false,
    ) {
        directories(ui, &summary);
    }
}

fn cached(ui: &Ui, texts: &Texts, view: &ComparisonView) -> Arc<SummaryTexts> {
    let key = Id::unique((texts.language(), view.generation(), view.revision()));
    let id = Id::unique("comparison-summary-texts");
    if let Some((known, kept)) = ui.data(|data| data.get_temp::<(Id, Arc<SummaryTexts>)>(id))
        && known == key
    {
        return kept;
    }
    let made = Arc::new(make(texts, view, view.summary()));
    ui.data_mut(|data| data.insert_temp(id, (key, Arc::clone(&made))));
    made
}
fn make(texts: &Texts, view: &ComparisonView, summary: &Summary) -> SummaryTexts {
    let language = texts.language();
    let number = |value: usize| format::number(u64::try_from(value).unwrap_or(u64::MAX), language);
    let counts = summary.counts;
    let comparison = view.comparison();
    let directory = |name: &str| {
        if name.is_empty() {
            texts.get(ComparisonText::Root)
        } else {
            name.to_owned()
        }
    };
    SummaryTexts {
        old_key: texts.get(ComparisonText::FactOld),
        old: side(texts, &comparison.old_side()),
        new_key: texts.get(ComparisonText::FactNew),
        new: side(texts, &comparison.new_side()),
        kinds: [
            (ComparisonText::Added, counts.added),
            (ComparisonText::Removed, counts.removed),
            (ComparisonText::Changed, counts.changed),
        ]
        .map(|(text, count)| (texts.get(text), number(count))),
        incomparable: (summary.incomparables != 0).then(|| {
            texts.format(
                ComparisonText::Incomparable,
                &[("number", number(summary.incomparables).into())],
            )
        }),
        directories_title: texts.format(
            ComparisonText::Directories,
            &[("number", number(summary.directories.len()).into())],
        ),
        directory_head: [
            texts.get(ComparisonText::ColumnDirectory),
            texts.get(ComparisonText::Added),
            texts.get(ComparisonText::Removed),
            texts.get(ComparisonText::Changed),
        ],
        directories: summary
            .directories
            .iter()
            .map(|row| {
                [
                    directory(&row.directory),
                    number(row.counts.added),
                    number(row.counts.removed),
                    number(row.counts.changed),
                ]
            })
            .collect(),
    }
}

/// Returns one side as its Channel and Build.
fn side(texts: &Texts, side: &Side<'_>) -> String {
    let channel = side
        .channel()
        .map_or_else(|| texts.get(ComparisonText::ChannelUnknown), str::to_owned);
    texts.format(
        ComparisonText::Side,
        &[
            ("channel", channel.into()),
            ("build", build_label(texts, side.build()).into()),
        ],
    )
}

fn directories(ui: &mut Ui, summary: &SummaryTexts) {
    ui.spacing_mut().item_spacing = egui::Vec2::ZERO;
    directory_row(ui, &summary.directory_head, true);
    ScrollArea::vertical()
        .id_salt("comparison-directories")
        .max_height(SECTION_LIST_HEIGHT)
        .show_rows(ui, ROW_HEIGHT, summary.directories.len(), |ui, rows| {
            for row in summary.directories.get(rows).unwrap_or_default() {
                directory_row(ui, row, false);
            }
        });
}

fn directory_row(ui: &mut Ui, cells: &[String; 4], head: bool) {
    let (rect, response) =
        ui.allocate_exact_size(vec2(ui.available_width(), ROW_HEIGHT), Sense::hover());
    let role = if head { Role::RowHeader } else { Role::Row };
    response.widget_info(|| WidgetInfo::labeled(role, true, cells.join(" ")));
    if head {
        ui.painter()
            .rect_filled(rect, CornerRadius::ZERO, palette::SURFACE_INPUT);
        let under = Rect::from_min_max(pos2(rect.left(), rect.bottom() - 2.0), rect.max);
        ui.painter()
            .rect_filled(under, CornerRadius::ZERO, palette::PRIMARY);
    } else {
        paint_divider(ui.painter(), rect);
    }
    let text = |value: &str| {
        if head {
            caps(value, TABLE_HEAD_SIZE, Weight::Bold, TABLE_HEAD_TRACKING)
        } else {
            plain(value, ROW_FONT, Weight::Regular)
        }
    };
    let counts_width = 3.0 * COUNT_COLUMN;
    let [first, counts @ ..] = cells;
    let name = rect
        .with_max_x(rect.right() - counts_width)
        .shrink2(vec2(CELL_PADDING_X, 0.0));
    let galley = line(ui, text(first), name.width());
    paint_left(ui, name, galley, palette::GRAY_1);
    let mut left = rect.right() - counts_width;
    for count in counts {
        let cell = Rect::from_min_max(
            pos2(left, rect.top()),
            pos2(left + COUNT_COLUMN, rect.bottom()),
        )
        .shrink2(vec2(CELL_PADDING_X, 0.0));
        let galley = line(ui, text(count), cell.width());
        paint_right(ui, cell, galley, palette::GRAY_1);
        left += COUNT_COLUMN;
    }
}
