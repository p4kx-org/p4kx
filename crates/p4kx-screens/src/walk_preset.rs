// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::canvas::find_canvas;
use crate::style::Modifiers;
use crate::style::preset::{CANVAS, PresetStyles};
use crate::walk::Walk;
use crate::widget::Widget;

impl Walk<'_, '_, '_, '_> {
    /// Applies the standard `TextField` preset where the widget has no size or text colour.
    pub(crate) fn apply_preset(&mut self, widget: &Widget, style: &mut Modifiers) {
        let Some(spec) = widget.text.as_ref() else {
            return;
        };
        let Some(preset) = spec.preset.as_deref() else {
            return;
        };
        let needs_size = style.text.font_size.is_none() && spec.font_size.is_none();
        let needs_fill =
            style.fill.is_none() && widget.shape.fill.is_none() && spec.colour.is_none();
        if !needs_size && !needs_fill {
            return;
        }
        let presets = self.presets.get_or_init(|| {
            let forge = self.store.forge();
            let canvas = find_canvas(forge, CANVAS)
                .ok()
                .and_then(|guid| forge.record_by_guid(guid))
                .and_then(|record| record.tree().ok());
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
        if let Some(found) = presets.style(preset) {
            if needs_size && let Some(size) = found.font_size {
                style.text.font_size = Some(size);
                style.text.font_style = style.text.font_style.or(found.font_style);
            }
            if needs_fill {
                style.text.preset_fill.clone_from(&found.fill);
            }
        }
    }
}
