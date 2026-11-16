// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use egui::{Color32, Frame, Label, Margin, Rect, Role, ScrollArea, Sense, Ui, WidgetInfo, vec2};
use fluent_bundle::FluentValue;

use super::search::eyebrow;
use super::ui_tab_toolbar::pack;
use crate::theme::desktop::{
    CELL_PADDING_X, FACT_KEY_GAP, FACT_VALUE_SIZE, FACTS_GAP, FACTS_PADDING_X, FACTS_PADDING_Y,
    ROW_FONT, SCROLLBAR_THIN, TOOLBAR_PADDING, XML_ROW_HEIGHT,
};
use crate::theme::palette;
use crate::theme::tokens::{CORNER_SM, FS_SM, SPACE_2, Weight};
use crate::theme::ui_metrics::UI_REPORT_MAX_LINES as MAX_REPORT_LINES;
use crate::widgets::frames::px;
use crate::widgets::tab::{self, SegmentLabel};
use crate::widgets::text::{line, paint_left, plain};
use crate::widgets::tooltip;
use crate::{
    ContentRow, Fact, Model, Note, ReportLine, RowDetail, Texts, UiContents, UiDetail, UiReport,
    UiText, format,
};

#[path = "ui_tab_omissions.rs"]
mod omissions;

/// Shows the choice between what the picture is made of and what it leaves out, and the chosen
/// part.
pub(super) fn show(ui: &mut Ui, model: &mut Model) {
    let texts = model.texts();
    let tab = model.ui();
    let contents = tab.contents();
    let report = tab.rendered().map(crate::UiRendered::report);
    let count =
        |value: usize| format::number(u64::try_from(value).unwrap_or(u64::MAX), texts.language());
    let labels = [
        SegmentLabel::new(
            ui,
            &texts.get(UiText::DetailContents),
            Some(&count(contents.map_or(0, |c| c.rows().len()))),
        ),
        SegmentLabel::new(
            ui,
            &texts.get(UiText::DetailReport),
            Some(&count(report.map_or(0, UiReport::len))),
        ),
    ];
    let active = match tab.detail() {
        UiDetail::Contents => 0,
        UiDetail::Report => 1,
    };
    let clicked = ui
        .horizontal(|ui| tab::counted_segments(ui, &labels, Some(active)))
        .inner;
    ui.add_space(SPACE_2);
    match tab.detail() {
        UiDetail::Contents => contents_body(ui, texts, contents),
        UiDetail::Report => report_body(ui, texts, report),
    }
    match clicked {
        Some(0) => model.ui_mut().set_detail(UiDetail::Contents),
        Some(1) => model.ui_mut().set_detail(UiDetail::Report),
        _ => {}
    }
}

fn contents_body(ui: &mut Ui, texts: &Texts, contents: Option<&UiContents>) {
    let Some(contents) = contents else {
        quiet(ui, texts.get(UiText::ContentsNone));
        return;
    };
    facts(ui, texts, contents.facts());
    ui.add_space(SPACE_2);
    let rows = contents.rows();
    if rows.is_empty() {
        quiet(ui, texts.get(UiText::ContentsNone));
        return;
    }
    let kinds: Vec<UiText> = rows.iter().map(|row| row.kind).collect();
    let column = column_width(ui, texts, &kinds);
    ui.spacing_mut().scroll.bar_width = SCROLLBAR_THIN;
    ui.spacing_mut().item_spacing.y = 0.0;
    ScrollArea::vertical()
        .id_salt("ui-contents")
        .auto_shrink(false)
        .show_rows(ui, XML_ROW_HEIGHT, rows.len(), |ui, visible| {
            for position in visible {
                if let Some(row) = rows.get(position) {
                    content_row(ui, texts, row, column);
                }
            }
        });
}

/// Returns the width of the column of kinds: the widest label of the kinds that occur.
fn column_width(ui: &Ui, texts: &Texts, kinds: &[UiText]) -> f32 {
    let mut seen: Vec<UiText> = Vec::new();
    for kind in kinds.iter().take(4096) {
        if !seen.contains(kind) {
            seen.push(*kind);
        }
    }
    let widest = seen
        .iter()
        .map(|kind| {
            line(
                ui,
                plain(texts.get(*kind), ROW_FONT, Weight::Regular),
                f32::INFINITY,
            )
            .size()
            .x
        })
        .fold(0.0, f32::max);
    widest + SPACE_2 + CELL_PADDING_X
}

fn content_row(ui: &mut Ui, texts: &Texts, row: &ContentRow, column: f32) {
    let kind = texts.get(row.kind);
    let detail = detail_text(texts, &row.detail);
    detail_row(ui, &kind, &detail, column);
}

fn detail_row(ui: &mut Ui, kind: &str, detail: &str, column: f32) {
    let (rect, response) =
        ui.allocate_exact_size(vec2(ui.available_width(), XML_ROW_HEIGHT), Sense::hover());
    response.widget_info(|| WidgetInfo::labeled(Role::Row, true, format!("{kind} {detail}")));
    let left = rect.with_max_x(rect.left() + column);
    let label = line(ui, plain(kind, ROW_FONT, Weight::Regular), column);
    paint_left(ui, left, label, palette::GRAY_2_TEXT);
    let right = rect.with_min_x(rect.left() + column);
    let galley = line(ui, plain(detail, ROW_FONT, Weight::Regular), right.width());
    let elided = galley.elided;
    paint_left(ui, right, galley, palette::GRAY_1);
    if elided {
        tooltip::hover(response, |ui| {
            tooltip::line(ui, detail);
        });
    }
}

fn detail_text(texts: &Texts, detail: &RowDetail) -> String {
    match detail {
        RowDetail::Plain(text) => text.clone(),
        RowDetail::Definition { id, note, name } => {
            let what = note_text(texts, note);
            name.as_ref().map_or_else(
                || format!("#{id}  {what}"),
                |name| format!("#{id}  {what}  {name}"),
            )
        }
    }
}

fn note_text(texts: &Texts, note: &Note) -> String {
    let size = |text: &str| [("size", FluentValue::from(text.to_owned()))];
    let counted = |value: u32| {
        [
            ("count", FluentValue::from(u64::from(value))),
            (
                "number",
                FluentValue::from(format::number(u64::from(value), texts.language())),
            ),
        ]
    };
    match note {
        Note::Size(text) => text.clone(),
        Note::Lossless(text) => texts.format(UiText::NoteLossless, &size(text)),
        Note::Embedded => texts.get(UiText::NoteEmbedded),
        Note::External(text) => texts.format(UiText::NoteExternal, &size(text)),
        Note::Part => texts.get(UiText::NotePart),
        Note::Font { name, glyphs } => {
            let [count, number] = counted(*glyphs);
            texts.format(
                UiText::NoteFont,
                &[("name", FluentValue::from(name.clone())), count, number],
            )
        }
        Note::Sprite { frames } => texts.format(UiText::NoteSprite, &counted(*frames)),
    }
}

fn facts(ui: &mut Ui, texts: &Texts, facts: &[(UiText, Fact)]) {
    if facts.is_empty() {
        return;
    }
    let shown: Vec<(UiText, String)> = facts
        .iter()
        .map(|(key, fact)| {
            let value = match fact {
                Fact::Count(count) => {
                    format::number(u64::try_from(*count).unwrap_or(u64::MAX), texts.language())
                }
                Fact::Text(text) => text.clone(),
            };
            (*key, value)
        })
        .collect();
    Frame::NONE
        .fill(palette::SURFACE_BAR)
        .corner_radius(CORNER_SM)
        .inner_margin(Margin::symmetric(px(FACTS_PADDING_X), px(FACTS_PADDING_Y)))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            let names: Vec<String> = shown.iter().map(|(key, _)| texts.get(*key)).collect();
            let widths: Vec<f32> = names
                .iter()
                .zip(&shown)
                .map(|(name, (_, value))| {
                    let key = line(ui, eyebrow(name), f32::INFINITY).size().x;
                    let value = line(
                        ui,
                        plain(value, FACT_VALUE_SIZE, Weight::Bold),
                        f32::INFINITY,
                    )
                    .size()
                    .x;
                    key + FACT_KEY_GAP + value
                })
                .collect();
            for row in pack(&widths, ui.available_width(), FACTS_GAP) {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = FACT_KEY_GAP;
                    for position in row {
                        let (Some(name), Some((_, value))) =
                            (names.get(position), shown.get(position))
                        else {
                            continue;
                        };
                        ui.label(eyebrow(name));
                        ui.add(
                            Label::new(
                                plain(value, FACT_VALUE_SIZE, Weight::Bold).color(palette::WHITE),
                            )
                            .extend()
                            .selectable(true),
                        );
                        ui.add_space(FACTS_GAP - FACT_KEY_GAP);
                    }
                });
            }
        });
}

fn report_body(ui: &mut Ui, texts: &Texts, report: Option<&UiReport>) {
    let Some(report) = report else {
        quiet(ui, texts.get(UiText::ContentsNone));
        return;
    };
    if !report.counts().is_empty() {
        let counts: Vec<(UiText, Fact)> = report
            .counts()
            .iter()
            .map(|(key, value)| (*key, Fact::Count(*value)))
            .collect();
        facts(ui, texts, &counts);
        ui.add_space(SPACE_2);
    }
    if report.is_empty() && report.content_omissions().is_empty() {
        quiet(ui, texts.get(UiText::ReportNone));
        return;
    }
    let kinds: Vec<UiText> = report.lines().iter().map(|line| line.label).collect();
    let column = column_width(ui, texts, &kinds);
    ScrollArea::vertical()
        .id_salt("ui-report")
        .auto_shrink(false)
        .show(ui, |ui| {
            for entry in report.lines().iter().take(MAX_REPORT_LINES) {
                report_line(ui, texts, entry, column);
            }
            if report.len() > MAX_REPORT_LINES {
                let more = u64::try_from(report.len() - MAX_REPORT_LINES).unwrap_or(u64::MAX);
                let args = [(
                    "number",
                    FluentValue::from(format::number(more, texts.language())),
                )];
                quiet(ui, texts.format(UiText::ReportMore, &args));
            }
            omissions::show(ui, texts, report.content_omissions());
        });
}

fn report_line(ui: &mut Ui, texts: &Texts, entry: &ReportLine, column: f32) {
    let colour: Color32 = if entry.label == UiText::ReportWarning {
        palette::WARNING
    } else {
        palette::GRAY_2_TEXT
    };
    ui.horizontal_top(|ui| {
        let label = texts.get(entry.label);
        let (rect, response) = ui.allocate_exact_size(vec2(column, XML_ROW_HEIGHT), Sense::hover());
        response.widget_info(|| WidgetInfo::labeled(Role::Label, true, &label));
        let galley = line(ui, plain(&label, ROW_FONT, Weight::Regular), column);
        paint_left(
            ui,
            Rect::from_min_size(rect.min, rect.size()),
            galley,
            colour,
        );
        if !entry.detail.is_empty() {
            ui.add(
                Label::new(plain(&entry.detail, ROW_FONT, Weight::Regular).color(palette::GRAY_1))
                    .wrap()
                    .selectable(true),
            );
        }
    });
}

fn quiet(ui: &mut Ui, text: String) {
    ui.add_space(TOOLBAR_PADDING / 2.0);
    ui.add(Label::new(
        plain(text, FS_SM, Weight::Regular).color(palette::GRAY_2_TEXT),
    ));
}
