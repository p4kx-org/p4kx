// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use egui::{Frame, Panel, Rect, Ui, vec2};

use crate::theme::desktop::NOTICE_ICON;
use crate::theme::palette;
use crate::theme::tokens::{FS_SM, SPACE_3, Weight};
use crate::widgets::frames;
use crate::widgets::indicator::warning_icon;
use crate::widgets::text::plain;

pub(super) fn show(ui: &mut Ui, id: &'static str, notice: &str, detail: Option<&str>) {
    Panel::top(id)
        .frame(Frame::NONE.fill(palette::SURFACE_PANEL))
        .show_separator_line(false)
        .show(ui, |ui| {
            frames::warning_alert(ui, |ui| {
                ui.spacing_mut().item_spacing.x = SPACE_3;
                ui.horizontal(|ui| {
                    let (icon, _) = ui
                        .allocate_exact_size(vec2(NOTICE_ICON, NOTICE_ICON), egui::Sense::hover());
                    warning_icon(
                        ui,
                        Rect::from_min_size(icon.min, icon.size()),
                        palette::WARNING,
                    );
                    let text = plain(notice, FS_SM, Weight::Regular).color(palette::GRAY_1);
                    let label = ui.add(egui::Label::new(text).wrap());
                    if let Some(detail) = detail {
                        label.on_hover_text(detail);
                    }
                });
            });
        });
}
