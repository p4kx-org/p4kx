// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use egui::{Align, Layout, Rect, Ui, vec2};

use super::Requests;
use crate::theme::desktop::{
    DROP_TARGET_FILL, EMPTY_ICON, EMPTY_PADDING, TITLE_SIZE, TITLE_TRACKING,
};
use crate::theme::palette;
use crate::theme::tokens::{CORNER_BOX, FS_XS, SPACE_3, Weight};
use crate::widgets::button::{self, Kind, Size};
use crate::widgets::frames;
use crate::widgets::text::{caps, plain};
use crate::{Model, Text};

use super::Dialog;

/// Shows the `.empty-state` before an Archive is open, lit as a drop target while a file is
/// dragged over the window.
pub(super) fn show(ui: &mut Ui, model: &Model, dragging: bool, requests: &mut Requests) {
    let texts = model.texts();
    let rect = ui.available_rect_before_wrap();
    let (fill, border) = if dragging {
        (DROP_TARGET_FILL, palette::PRIMARY)
    } else {
        (palette::BG_DARK_GRAY, palette::GRAY_3)
    };
    ui.painter()
        .rect_filled(rect, CORNER_BOX, palette::BG_DARK_GRAY);
    ui.painter().rect_filled(rect, CORNER_BOX, fill);
    frames::dashed(ui, rect.shrink(0.5), border);
    let title = if dragging {
        Text::DropRelease
    } else {
        Text::EmptyTitle
    };
    ui.with_layout(Layout::top_down(Align::Center), |ui| {
        ui.add_space(
            3.0f32
                .mul_add(-EMPTY_ICON, rect.height() / 2.0)
                .max(EMPTY_PADDING),
        );
        let (icon, _) = ui.allocate_exact_size(vec2(EMPTY_ICON, EMPTY_ICON), egui::Sense::hover());
        archive_icon(ui, icon);
        ui.add_space(SPACE_3);
        ui.label(
            caps(&texts.get(title), TITLE_SIZE, Weight::Bold, TITLE_TRACKING).color(palette::TITLE),
        );
        ui.add_space(SPACE_3);
        ui.label(
            plain(texts.get(Text::DropHint), FS_XS, Weight::Regular).color(palette::GRAY_2_TEXT),
        );
        if !dragging {
            ui.add_space(SPACE_3);
            let open = texts.get(Text::ActionOpen);
            if button::button(ui, Kind::Outline, Size::Normal, &open, true).clicked() {
                requests.dialog = Some(Dialog::OpenArchive);
            }
        }
    });
}

fn archive_icon(ui: &Ui, rect: Rect) {
    let stroke = egui::Stroke::new(2.0, palette::GRAY_2_TEXT);
    let at = |x: f32, y: f32| {
        egui::pos2(
            x.mul_add(rect.width(), rect.left()),
            y.mul_add(rect.height(), rect.top()),
        )
    };
    let painter = ui.painter();
    painter.line_segment([at(0.5, 0.12), at(0.5, 0.62)], stroke);
    painter.line_segment([at(0.3, 0.44), at(0.5, 0.64)], stroke);
    painter.line_segment([at(0.7, 0.44), at(0.5, 0.64)], stroke);
    painter.line_segment([at(0.12, 0.62), at(0.12, 0.86)], stroke);
    painter.line_segment([at(0.12, 0.86), at(0.88, 0.86)], stroke);
    painter.line_segment([at(0.88, 0.86), at(0.88, 0.62)], stroke);
}

/// Shows a dashed `.empty-state` that fills the rest of `ui` with `title` and `hint`.
pub(super) fn notice(ui: &mut Ui, title: &str, hint: &str) {
    let rect = ui.available_rect_before_wrap();
    ui.painter()
        .rect_filled(rect, CORNER_BOX, palette::BG_DARK_GRAY);
    frames::dashed(ui, rect.shrink(0.5), palette::GRAY_3);
    egui::Frame::NONE
        .inner_margin(egui::Margin::same(frames::px(EMPTY_PADDING)))
        .show(ui, |ui| {
            ui.set_min_size(rect.size() - vec2(2.0 * EMPTY_PADDING, 2.0 * EMPTY_PADDING));
            ui.with_layout(Layout::top_down(Align::Center), |ui| {
                ui.add_space((rect.height() / 2.0 - EMPTY_ICON).max(0.0));
                ui.label(
                    caps(title, TITLE_SIZE, Weight::Bold, TITLE_TRACKING).color(palette::TITLE),
                )
                .widget_info(|| egui::WidgetInfo::labeled(egui::Role::Label, true, title));
                ui.add_space(SPACE_3);
                ui.label(plain(hint, FS_XS, Weight::Regular).color(palette::GRAY_2_TEXT));
            });
        });
}
