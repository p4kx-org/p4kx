// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use egui::{Frame, Panel, Rect, Ui, vec2};
use p4kx_extraction::DEFAULT_WATCH_INTERVAL;

use crate::theme::desktop::NOTICE_ICON;
use crate::theme::palette;
use crate::theme::tokens::{FS_SM, SPACE_3, Weight};
use crate::widgets::button::{self, Kind, Size};
use crate::widgets::frames;
use crate::widgets::indicator::warning_icon;
use crate::widgets::text::plain;
use crate::{Extraction, Model, Text};

fn archive_path(model: &Model) -> String {
    model
        .browser()
        .map_or_default(|browser| browser.archive_index().path().display().to_string())
}

/// Compares the open Archive with the disk when due, keeps the window repainting so that this
/// happens while it is idle, and shows the warning notice with the reload action once it changed.
pub(super) fn show(ui: &mut Ui, model: &mut Model) {
    if model.browser().is_none() {
        return;
    }
    model.check_archive_when_due();
    ui.ctx().request_repaint_after(DEFAULT_WATCH_INTERVAL);
    if !model.archive_changed() || model.opening_path().is_some() {
        return;
    }
    let running = model.extraction().is_some_and(Extraction::is_running);
    let mut reload = false;
    Panel::top("archive-changed")
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
                    let path = archive_path(model);
                    let texts = model.texts();
                    let stopped = if model.change_stopped_extraction() {
                        "yes"
                    } else {
                        "no"
                    };
                    let notice = texts.format(
                        Text::ArchiveChanged,
                        &[("path", path.into()), ("stopped", stopped.into())],
                    );
                    let label = texts.get(Text::ActionReload);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        reload = button::button(ui, Kind::Outline, Size::Compact, &label, !running)
                            .clicked();
                        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                            let text = plain(notice, FS_SM, Weight::Regular).color(palette::GRAY_1);
                            ui.add(egui::Label::new(text).wrap());
                        });
                    });
                });
            });
        });
    if reload {
        let ctx = ui.ctx().clone();
        model.reload(move || ctx.request_repaint());
    }
}
