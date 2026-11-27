// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use egui::{
    Align2, Color32, CornerRadius, Rect, Response, Role, Sense, Ui, WidgetInfo, pos2, vec2,
};
use p4kx_extraction::FieldOrigin;

use super::{Labelled, Table, TableKind};
use crate::theme::desktop::{CELL_PADDING_X, ROW_HEIGHT, ZEBRA};
use crate::theme::palette;
use crate::ui::source_origin;
use crate::widgets::{rows::paint_divider, tooltip};
use crate::{ModelInfoProvenance, Texts};

pub(super) fn show(
    ui: &mut Ui,
    table: usize,
    data: &Table,
    texts: &Texts,
    origins: Option<&ModelInfoProvenance>,
) {
    let (head, _) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_HEIGHT), Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(head, CornerRadius::ZERO, palette::SURFACE_INPUT);
    let rule = Rect::from_min_max(pos2(head.left(), head.bottom() - 2.0), head.max);
    painter.rect_filled(rule, CornerRadius::ZERO, palette::PRIMARY);
    cells(
        ui,
        (2, table, 0),
        head,
        &data.head,
        texts,
        [CellOrigin::Hidden; 4],
    );
    for (index, row) in data.rows.iter().enumerate() {
        let (rect, _) =
            ui.allocate_exact_size(vec2(ui.available_width(), ROW_HEIGHT), Sense::hover());
        if ui.rect_contains_pointer(rect) {
            ui.painter()
                .rect_filled(rect, CornerRadius::ZERO, palette::CONTROL_BG);
        } else if index % 2 == 1 {
            ui.painter().rect_filled(rect, CornerRadius::ZERO, ZEBRA);
        }
        paint_divider(ui.painter(), rect);
        cells(
            ui,
            (3, table, index + 1),
            rect,
            row,
            texts,
            row_origins(data.kind, index, origins),
        );
    }
}

#[derive(Clone, Copy)]
enum CellOrigin<'a> {
    Hidden,
    Shown(Option<&'a FieldOrigin>),
}

fn row_origins(
    kind: TableKind,
    index: usize,
    origins: Option<&ModelInfoProvenance>,
) -> [CellOrigin<'_>; 4] {
    match kind {
        TableKind::Lods => {
            let row = origins.and_then(|origins| origins.mesh_summary().lod_counts.get(index));
            [
                CellOrigin::Hidden,
                CellOrigin::Shown(row.map(|row| &row.vertices)),
                CellOrigin::Shown(row.map(|row| &row.triangles)),
                CellOrigin::Hidden,
            ]
        }
        TableKind::UnreadChunks => {
            let row = origins.and_then(|origins| origins.unread_chunks().get(index));
            row.map_or([None; 4], |row| {
                [
                    Some(&row.file),
                    Some(&row.kind),
                    Some(&row.type_id),
                    Some(&row.usage),
                ]
            })
            .map(CellOrigin::Shown)
        }
    }
}

fn cells(
    ui: &Ui,
    (section, table, row): (u8, usize, usize),
    rect: Rect,
    cells: &[Labelled],
    texts: &Texts,
    markers: [CellOrigin<'_>; 4],
) {
    let columns = u16::try_from(cells.len()).map_or(1.0, f32::from).max(1.0);
    let width = rect.width() / columns;
    for ((column, step), cell) in (0_usize..).zip(0_u16..).zip(cells) {
        let left = f32::from(step).mul_add(width, rect.left());
        let cell_rect = Rect::from_min_size(pos2(left, rect.top()), vec2(width, rect.height()))
            .shrink2(vec2(CELL_PADDING_X, 0.0));
        let shown = labelled(
            ui,
            (section, table * 1_000_000 + row, column),
            cell,
            (cell_rect, palette::GRAY_1),
        );
        let marker = markers.get(column).copied().unwrap_or(CellOrigin::Hidden);
        if cell.1.elided || matches!(marker, CellOrigin::Shown(_)) {
            tooltip::hover(shown, |ui| {
                if cell.1.elided {
                    tooltip::line(ui, cell.0.as_str());
                }
                if let CellOrigin::Shown(origin) = marker {
                    tooltip::line(ui, texts.get(source_origin::label(origin)));
                }
            });
        }
    }
}

pub(super) fn labelled(
    ui: &Ui,
    salt: (u8, usize, usize),
    (name, galley): &Labelled,
    (rect, colour): (Rect, Color32),
) -> Response {
    let at = Align2::LEFT_CENTER
        .align_size_within_rect(galley.size(), rect)
        .min;
    let response = ui.interact(
        rect,
        ui.scope_id().with(("model-info-text", salt)),
        Sense::hover(),
    );
    response.widget_info(|| WidgetInfo::labeled(Role::Label, true, name));
    ui.painter().galley(at, Arc::clone(galley), colour);
    response
}
