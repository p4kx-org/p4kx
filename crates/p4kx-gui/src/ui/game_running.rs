// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use egui::Ui;

use crate::{GAME_CHECK_INTERVAL, Model, Text};

/// Looks at the process list when due and shows the warning notice that the game is running.
pub(super) fn show(ui: &mut Ui, model: &mut Model) {
    let ctx = ui.ctx().clone();
    model.check_game_when_due(move || ctx.request_repaint());
    ui.ctx().request_repaint_after(GAME_CHECK_INTERVAL);
    if !model.game_running() {
        return;
    }
    let notice = model.texts().get(Text::GameRunning);
    super::warning_notice::show(ui, "game-running", &notice, None);
}
