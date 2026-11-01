// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::rc::Rc;

use crate::canvas::{Canvas, find_canvas};
use crate::style::Modifiers;
use crate::style::preset::{CANVAS, PresetStyles, TAG_PATH};
use crate::walk::Walk;
use crate::widget::Widget;

impl Walk<'_, '_, '_, '_> {
    /// Gives a text field the font style and the font size of its text preset where its style sets
    /// none, read from the canvas of the standard text field under the brand style of `canvas`.
    /// A preset the Archive sets nothing for leaves the field as it is.
    pub(crate) fn apply_preset(&mut self, canvas: &Canvas, widget: &Widget, style: &mut Modifiers) {
        let Some(preset) = widget
            .text
            .as_ref()
            .filter(|spec| spec.font_size.is_none())
            .and_then(|spec| spec.preset.as_deref())
        else {
            return;
        };
        let Some(tag) = self
            .tags
            .guid_of(&format!("{TAG_PATH}{}", preset.to_ascii_lowercase()))
        else {
            return;
        };
        let brand = canvas.pick_brand(self.brand).map(|brand| brand.style.guid);
        let found = self.preset_styles().style(tag, brand);
        let text = &mut style.text;
        text.font_style = text.font_style.or(found.font_style);
        text.font_size = text.font_size.or(found.font_size);
    }

    fn preset_styles(&mut self) -> Rc<PresetStyles> {
        if let Some(loaded) = &self.presets {
            return Rc::clone(loaded);
        }
        let styles = find_canvas(self.store.forge(), CANVAS)
            .ok()
            .and_then(|guid| self.store.get(guid))
            .map(|canvas| PresetStyles::read(&canvas))
            .unwrap_or_default();
        let loaded = Rc::new(styles);
        self.presets = Some(Rc::clone(&loaded));
        loaded
    }
}
