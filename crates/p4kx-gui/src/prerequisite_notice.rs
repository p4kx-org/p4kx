// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use egui::{Align, Layout, Margin, Ui, vec2};

use crate::theme::desktop::{MODAL_WIDE_WIDTH, TITLE_SIZE, TITLE_TRACKING};
use crate::theme::palette;
use crate::theme::tokens::{FS_SM, SPACE_3, SPACE_6, Weight};
use crate::widgets::button::{self, Kind, Size};
use crate::widgets::frames;
use crate::widgets::text::{caps, plain};
use crate::{ExtractionText, Language, ProtectionText, Texts};

/// The non-operational window shown when a Flatpak prerequisite is unavailable.
#[derive(Debug)]
pub struct PrerequisiteNotice {
    texts: Texts,
}

impl PrerequisiteNotice {
    /// Creates the notice without opening profiles, Archives or an operational model.
    #[must_use]
    pub fn new(language: Language) -> Self {
        Self {
            texts: Texts::new(language),
        }
    }

    /// Draws the notice and returns whether its close action was chosen.
    #[must_use]
    pub fn show(&self, ui: &mut Ui) -> bool {
        let mut closed = ui.input(|input| input.key_pressed(egui::Key::Escape));
        let size = ui.available_size();
        frames::canvas()
            .inner_margin(Margin::same(frames::px(SPACE_6)))
            .show(ui, |ui| {
                ui.set_min_size((size - vec2(2.0 * SPACE_6, 2.0 * SPACE_6)).max(vec2(0.0, 0.0)));
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.with_layout(Layout::top_down(Align::Center), |ui| {
                        ui.set_max_width(MODAL_WIDE_WIDTH);
                        ui.label(
                            caps(
                                &self.texts.get(ProtectionText::PrerequisiteTitle),
                                TITLE_SIZE,
                                Weight::Bold,
                                TITLE_TRACKING,
                            )
                            .color(palette::DANGER_TEXT),
                        );
                        ui.add_space(SPACE_3);
                        frames::danger_alert(ui, |ui| {
                            ui.add(
                                egui::Label::new(
                                    plain(
                                        self.texts.get(ProtectionText::Prerequisite),
                                        FS_SM,
                                        Weight::Regular,
                                    )
                                    .color(palette::GRAY_1),
                                )
                                .wrap(),
                            );
                        });
                        ui.add_space(SPACE_3);
                        closed |= button::button(
                            ui,
                            Kind::Ghost,
                            Size::Normal,
                            &self.texts.get(ExtractionText::Close),
                            true,
                        )
                        .clicked();
                    });
                });
            });
        closed
    }
}

impl eframe::App for PrerequisiteNotice {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        if self.show(ui) {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}
