// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use egui::{Key, KeyboardShortcut, ModifierNames, Modifiers, Ui};

use crate::{Extraction, ExtractionScope, Model};

/// Opens the Archive again: F5.
pub(super) const RELOAD: KeyboardShortcut = KeyboardShortcut::new(Modifiers::NONE, Key::F5);
/// Opens the Extraction setup for the selected Entries: Ctrl+E, Cmd+E on macOS.
pub(super) const EXTRACT_SELECTED: KeyboardShortcut =
    KeyboardShortcut::new(Modifiers::COMMAND, Key::E);
/// Runs the current Query.
pub(super) const RUN_QUERY: KeyboardShortcut =
    KeyboardShortcut::new(Modifiers::COMMAND, Key::Enter);

/// Formats a Command shortcut for the current window and language.
pub(super) fn command_hint(
    ui: &Ui,
    shortcut: KeyboardShortcut,
    localized: impl FnOnce() -> String,
) -> String {
    if ui.ctx().os().is_mac() {
        shortcut.format(&ModifierNames::NAMES, true)
    } else {
        localized()
    }
}

/// Returns whether the open Archive may be opened again: one is open, none is opening, no
/// Extraction runs and no Extraction setup is open.
pub(super) fn reload_enabled(model: &Model) -> bool {
    model.browser().is_some() && model.opening_path().is_none() && idle(model)
}

/// Returns whether the selected Entries may be extracted: some are selected and neither an
/// Extraction nor its setup is open.
pub(super) fn extract_selected_enabled(model: &Model) -> bool {
    model.scope_len(ExtractionScope::Selected) > 0 && idle(model)
}

fn idle(model: &Model) -> bool {
    !model.extraction().is_some_and(Extraction::is_running) && model.extraction_setup().is_none()
}

/// Applies the keyboard shortcuts of the Archive menu that the user pressed this frame.
pub(super) fn handle(ui: &Ui, model: &mut Model) {
    let (reload, extract) = ui.input_mut(|input| {
        (
            input.consume_shortcut(&RELOAD),
            input.consume_shortcut(&EXTRACT_SELECTED),
        )
    });
    if reload && reload_enabled(model) {
        let ctx = ui.ctx().clone();
        model.reload(move || ctx.request_repaint());
    }
    if extract && extract_selected_enabled(model) {
        model.begin_extraction(ExtractionScope::Selected);
    }
}
