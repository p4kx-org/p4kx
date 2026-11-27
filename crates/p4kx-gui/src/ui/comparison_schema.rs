// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use egui::{
    CornerRadius, Frame, Margin, Rect, Role, ScrollArea, Sense, Stroke, Ui, Vec2, WidgetInfo, pos2,
    vec2,
};
use p4kx_comparison::{KindCounts, SchemaChangeKind};

use crate::theme::desktop::{
    CELL_PADDING_X, CHANGE_COLUMN, COUNT_COLUMN, ROW_FONT, ROW_HEIGHT, TABLE_HEAD_SIZE,
    TABLE_HEAD_TRACKING, TOOLBAR_PADDING, TREE_INDENT, TREE_RAIL_OFFSET,
};
use crate::theme::palette;
use crate::theme::tokens::{FS_SM, FS_XS, Weight};
use crate::widgets::chip::{self, Tone};
use crate::widgets::frames::px;
use crate::widgets::rows::{RowState, paint_divider, paint_table_row};
use crate::widgets::text::{caps, line, paint_left, paint_right, plain};
use crate::{
    ComparisonText, ComparisonView, Model, SchemaRow, SchemaRowKind, SchemaText, SchemaView, Text,
    Texts, format,
};

const KIND_COLUMN: f32 = COUNT_COLUMN;
const RECORDS_COLUMN: f32 = CHANGE_COLUMN;
const FIXED: f32 = 2.0f32.mul_add(RECORDS_COLUMN, CHANGE_COLUMN + KIND_COLUMN);
const NAME_SHARE: f32 = 0.4;
const FIRST_RECORDS_COLUMN: usize = 5;

const HEADS: [Text; 7] = [
    Text::Comparison(ComparisonText::ColumnChange),
    Text::Schema(SchemaText::ColumnKind),
    Text::Schema(SchemaText::ColumnName),
    Text::Schema(SchemaText::ColumnOld),
    Text::Schema(SchemaText::ColumnNew),
    Text::Schema(SchemaText::ColumnOldRecords),
    Text::Schema(SchemaText::ColumnNewRecords),
];

/// Shows the schema list: the counts per level and the virtual list of the structs, enums and
/// their members that differ, or why the schemas could not be compared.
pub(super) fn show(ui: &mut Ui, model: &Model) {
    let Some(schema) = model.comparison().view().and_then(ComparisonView::schema) else {
        return;
    };
    let texts = model.texts();
    let schema = match schema {
        Ok(schema) => schema,
        Err(reason) => {
            let text = texts.format(SchemaText::Failed, &[("reason", reason.to_owned().into())]);
            note(ui, text, palette::DANGER_TEXT);
            return;
        }
    };
    counts(ui, texts, schema);
    if schema.rows().is_empty() {
        note(
            ui,
            texts.get(SchemaText::NoDifferences),
            palette::GRAY_2_TEXT,
        );
        return;
    }
    ui.spacing_mut().item_spacing = Vec2::ZERO;
    let labels = Labels::of(ui, texts);
    head_row(ui, &labels);
    ScrollArea::vertical()
        .id_salt("comparison-schema")
        .auto_shrink(false)
        .show_rows(ui, ROW_HEIGHT, schema.rows().len(), |ui, rows| {
            for (index, row) in schema.rows().iter().enumerate().skip(rows.start) {
                if index >= rows.end {
                    break;
                }
                data_row(ui, texts, &labels, (row, index));
            }
        });
}

fn note(ui: &mut Ui, text: String, colour: egui::Color32) {
    Frame::NONE
        .inner_margin(Margin::same(px(TOOLBAR_PADDING)))
        .show(ui, |ui| {
            ui.label(plain(text, FS_XS, Weight::Regular).color(colour));
        });
}

fn counts(ui: &mut Ui, texts: &Texts, schema: &SchemaView) {
    let language = texts.language();
    let number = |value: u64| format::number(value, language);
    let counts = schema.counts();
    Frame::NONE
        .inner_margin(Margin::same(px(TOOLBAR_PADDING)))
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing = vec2(TOOLBAR_PADDING, TOOLBAR_PADDING / 2.0);
            ui.horizontal_wrapped(|ui| {
                for (label, kinds) in [
                    (SchemaText::LabelStructs, counts.structs),
                    (SchemaText::LabelProperties, counts.properties),
                    (SchemaText::LabelEnums, counts.enums),
                    (SchemaText::LabelOptions, counts.options),
                ] {
                    let KindCounts {
                        added,
                        removed,
                        changed,
                    } = kinds;
                    let text = texts.format(
                        SchemaText::Counts,
                        &[
                            ("label", texts.get(label).into()),
                            ("added", number(added).into()),
                            ("removed", number(removed).into()),
                            ("changed", number(changed).into()),
                        ],
                    );
                    ui.label(plain(text, FS_SM, Weight::Regular).color(palette::GRAY_1));
                }
            });
        });
}

struct Labels {
    heads: [String; 7],
    chips: [std::sync::Arc<egui::Galley>; 3],
    none: String,
}

impl Labels {
    fn of(ui: &Ui, texts: &Texts) -> Self {
        Self {
            heads: HEADS.map(|text| texts.get(text)),
            chips: [
                ComparisonText::Added,
                ComparisonText::Removed,
                ComparisonText::Changed,
            ]
            .map(|text| chip::galley(ui, &texts.get(text))),
            none: texts.get(ComparisonText::NoSize),
        }
    }
}

fn columns(rect: Rect) -> [Rect; 7] {
    let spare = (rect.width() - FIXED).max(0.0);
    let name = (spare * NAME_SHARE).floor();
    let side = ((spare - name) / 2.0).floor();
    let widths = [
        CHANGE_COLUMN,
        KIND_COLUMN,
        name,
        side,
        side,
        RECORDS_COLUMN,
        RECORDS_COLUMN,
    ];
    let mut left = rect.left();
    widths.map(|width| {
        let cell = Rect::from_min_max(pos2(left, rect.top()), pos2(left + width, rect.bottom()));
        left += width;
        cell
    })
}

fn head_row(ui: &mut Ui, labels: &Labels) {
    let (rect, response) =
        ui.allocate_exact_size(vec2(ui.available_width(), ROW_HEIGHT), Sense::hover());
    response.widget_info(|| WidgetInfo::labeled(Role::RowHeader, true, labels.heads.join(" ")));
    ui.painter()
        .rect_filled(rect, CornerRadius::ZERO, palette::SURFACE_INPUT);
    let under = Rect::from_min_max(pos2(rect.left(), rect.bottom() - 2.0), rect.max);
    ui.painter()
        .rect_filled(under, CornerRadius::ZERO, palette::PRIMARY);
    for (index, (cell, head)) in columns(rect).into_iter().zip(&labels.heads).enumerate() {
        let cell = cell.shrink2(vec2(CELL_PADDING_X, 0.0));
        let text = caps(head, TABLE_HEAD_SIZE, Weight::Bold, TABLE_HEAD_TRACKING);
        let galley = line(ui, text, cell.width());
        if index >= FIRST_RECORDS_COLUMN {
            paint_right(ui, cell, galley, palette::GRAY_1);
        } else {
            paint_left(ui, cell, galley, palette::GRAY_1);
        }
    }
}

const fn tone(kind: SchemaChangeKind) -> (usize, ComparisonText, Tone) {
    match kind {
        SchemaChangeKind::Added => (0, ComparisonText::Added, Tone::Success),
        SchemaChangeKind::Removed => (1, ComparisonText::Removed, Tone::Danger),
        _ => (2, ComparisonText::Changed, Tone::Warning),
    }
}

const fn kind_text(kind: SchemaRowKind) -> SchemaText {
    match kind {
        SchemaRowKind::Struct => SchemaText::KindStruct,
        SchemaRowKind::Parent => SchemaText::KindParent,
        SchemaRowKind::Property => SchemaText::KindProperty,
        SchemaRowKind::Enum => SchemaText::KindEnum,
        SchemaRowKind::Option => SchemaText::KindOption,
    }
}

fn data_row(ui: &mut Ui, texts: &Texts, labels: &Labels, (row, index): (&SchemaRow, usize)) {
    let (rect, response) =
        ui.allocate_exact_size(vec2(ui.available_width(), ROW_HEIGHT), Sense::hover());
    let (slot, change_text, row_tone) = tone(row.change());
    let kind = texts.get(kind_text(row.kind()));
    response.widget_info(|| {
        let change = texts.get(change_text);
        let name = texts.format(
            SchemaText::Row,
            &[
                ("change", change.into()),
                ("kind", kind.clone().into()),
                ("name", row.name().to_owned().into()),
            ],
        );
        WidgetInfo::labeled(Role::Row, true, name)
    });
    paint_table_row(ui.painter(), rect, RowState::Rest, index % 2 == 1);
    paint_divider(ui.painter(), rect);
    let [change, kind_cell, name, old, new, old_records, new_records] = columns(rect);
    if let Some(label) = labels.chips.get(slot) {
        let at = pos2(change.left() + CELL_PADDING_X, change.center().y);
        chip::paint(ui, at, label.clone(), row_tone);
    }
    let weight = if row.is_member() {
        Weight::Regular
    } else {
        Weight::Bold
    };
    text(ui, &kind, pad(kind_cell, 0.0), Weight::Regular);
    let indent = if row.is_member() {
        let cell = pad(name, 0.0);
        ui.painter().vline(
            cell.left() + TREE_RAIL_OFFSET,
            cell.y_range(),
            Stroke::new(1.0, palette::GRAY_3),
        );
        TREE_INDENT
    } else {
        0.0
    };
    text(ui, row.name(), pad(name, indent), weight);
    for (value, cell) in [(row.old_text(), old), (row.new_text(), new)] {
        let shown = if value.is_empty() {
            labels.none.as_str()
        } else {
            value
        };
        text(ui, shown, pad(cell, 0.0), Weight::Regular);
    }
    if let Some(records) = row.records() {
        for (value, cell) in [(records.old, old_records), (records.new, new_records)] {
            let cell = cell.shrink2(vec2(CELL_PADDING_X, 0.0));
            let number = format::number(value, texts.language());
            let galley = line(ui, plain(number, ROW_FONT, weight), cell.width());
            paint_right(ui, cell, galley, palette::GRAY_1);
        }
    }
}

fn text(ui: &Ui, value: &str, cell: Rect, weight: Weight) {
    let galley = line(ui, plain(value, ROW_FONT, weight), cell.width());
    paint_left(ui, cell, galley, palette::GRAY_1);
}

fn pad(cell: Rect, indent: f32) -> Rect {
    Rect::from_min_max(
        pos2(cell.left() + CELL_PADDING_X + indent, cell.top()),
        pos2(cell.right() - CELL_PADDING_X, cell.bottom()),
    )
}
