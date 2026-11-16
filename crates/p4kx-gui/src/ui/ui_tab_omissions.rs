// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use egui::Ui;
use fluent_bundle::FluentValue;
use p4kx_screens::{ContentOmission, ContentOmissionCode};

use super::{column_width, detail_row, eyebrow};
use crate::theme::desktop::XML_ROW_HEIGHT;
use crate::theme::tokens::SPACE_2;
use crate::{Texts, UiText};

const REASONS: [UiText; 7] = [
    UiText::ReportOmissionSourceMissing,
    UiText::ReportOmissionRuntimeTarget,
    UiText::ReportOmissionRuntimeArray,
    UiText::ReportOmissionVariableMissing,
    UiText::ReportOmissionUnsupportedField,
    UiText::ReportOmissionUnresolvedInput,
    UiText::ReportOmissionOther,
];

/// Shows the visible original source details inside the report's scroll area.
pub(super) fn show(ui: &mut Ui, texts: &Texts, rows: &[ContentOmission]) {
    if rows.is_empty() {
        return;
    }
    ui.add_space(SPACE_2);
    ui.label(eyebrow(&texts.get(UiText::ReportOmissionDetails)));
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        let top = ui.next_widget_position().y;
        let clip = ui.clip_rect();
        let first = row_at(clip.top() - top).min(rows.len());
        let end = row_at(clip.bottom() - top)
            .saturating_add(1)
            .clamp(first, rows.len());
        ui.add_space(row_offset(first));
        ui.skip_ahead_auto_ids(first);
        if let Some(visible) = rows.get(first..end) {
            let column = column_width(ui, texts, &REASONS);
            for row in visible {
                detail_row(
                    ui,
                    &texts.get(reason_text(row.code)),
                    &detail(texts, row),
                    column,
                );
            }
        }
        ui.add_space(row_offset(rows.len() - end));
    });
}

#[expect(
    clippy::cast_precision_loss,
    reason = "a row count fits the precision of a scroll offset"
)]
fn row_offset(index: usize) -> f32 {
    index as f32 * XML_ROW_HEIGHT
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "a non-negative row position saturates into a row index"
)]
fn row_at(y: f32) -> usize {
    (y / XML_ROW_HEIGHT).floor().max(0.0) as usize
}

fn detail(texts: &Texts, row: &ContentOmission) -> String {
    let source = row.source.as_deref().map_or_else(
        || FluentValue::from(texts.get(UiText::ReportOmissionNoSource)),
        FluentValue::from,
    );
    texts.format(
        UiText::ReportOmissionRow,
        &[
            ("widget", row.widget.as_str().into()),
            ("element", row.element.as_str().into()),
            ("record", row.widget_input.record_index.to_string().into()),
            ("section", row.widget_input.section.field_name().into()),
            ("slot", row.widget_input.slot.to_string().into()),
            ("field", row.field.as_str().into()),
            ("source", source),
        ],
    )
}

const fn reason_text(code: ContentOmissionCode) -> UiText {
    match code {
        ContentOmissionCode::SourceMissing => UiText::ReportOmissionSourceMissing,
        ContentOmissionCode::RuntimeTarget => UiText::ReportOmissionRuntimeTarget,
        ContentOmissionCode::RuntimeArray => UiText::ReportOmissionRuntimeArray,
        ContentOmissionCode::VariableMissing => UiText::ReportOmissionVariableMissing,
        ContentOmissionCode::UnsupportedField => UiText::ReportOmissionUnsupportedField,
        ContentOmissionCode::UnresolvedInput => UiText::ReportOmissionUnresolvedInput,
        _ => UiText::ReportOmissionOther,
    }
}
