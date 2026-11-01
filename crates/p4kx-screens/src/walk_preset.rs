// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::canvas::find_canvas;
use crate::style::Modifiers;
use crate::style::preset::{CANVAS, PresetStyles};
use crate::walk::Walk;
use crate::widget::Widget;

impl Walk<'_, '_, '_, '_> {
    /// Gives a text field whose style sets no font size the size of its text preset and, where
    /// its style sets no font style, the font style of the preset, read from the canvas of the
    /// standard text field under the brand of the screen. A field whose size is set, or a preset
    /// that sets no size, is left as it is.
    pub(crate) fn apply_preset(&mut self, widget: &Widget, style: &mut Modifiers) {
        if style.text.font_size.is_some() {
            return;
        }
        let Some(preset) = widget
            .text
            .as_ref()
            .filter(|spec| spec.font_size.is_none())
            .and_then(|spec| spec.preset.as_deref())
        else {
            return;
        };
        let presets = self.presets.get_or_init(|| {
            let canvas = find_canvas(self.store.forge(), CANVAS)
                .ok()
                .and_then(|guid| self.store.get(guid));
            let Some(canvas) = canvas else {
                self.cx.report.note(
                    &widget.type_name,
                    "standard text field not found in the Archive: text presets have no style",
                    &widget.name,
                );
                return PresetStyles::default();
            };
            PresetStyles::read(&canvas, self.tags, self.brand)
        });
        if let Some(found) = presets.style(preset)
            && let Some(size) = found.font_size
        {
            style.text.font_size = Some(size);
            style.text.font_style = style.text.font_style.or(found.font_style);
        }
    }
}
