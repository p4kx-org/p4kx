// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::rc::Rc;

use crate::canvas::Canvas;
use crate::rest::TEXT_FIELD;
use crate::style::Modifiers;
use crate::text::{FontStyleInfo, is_editor_label};
use crate::walk::Walk;
use crate::widget::Widget;

/// The only parameter of a text field read as its text; `ParamInput1` and up are not read.
const FIRST_PARAMETER: &str = "ParamInput0";

impl Walk<'_, '_, '_, '_> {
    pub(crate) fn font_of(
        &mut self,
        widget: &Widget,
        style: &Modifiers,
    ) -> Option<Rc<FontStyleInfo>> {
        let spec = widget.text.as_ref()?;
        let guid = style.text.font_style.or(spec.font_style)?;
        self.cx.texts.styles.get(self.store.forge(), guid)
    }

    /// Returns the text a binding gives widget `index`: the localised value its text or its first
    /// parameter is bound to, which replaces the label it holds, or the component parameter that
    /// fills it.
    pub(crate) fn bound_text(&self, canvas: &Canvas, index: usize) -> Option<String> {
        let own = self.rests.last().and_then(|state| {
            let bound = state.localised(index);
            [TEXT_FIELD, FIRST_PARAMETER].into_iter().find_map(|field| {
                bound
                    .iter()
                    .find(|(name, _)| name == field)
                    .map(|(_, value)| value.clone())
            })
        });
        own.or_else(|| self.parameter_text(canvas, index))
            .or_else(|| self.unfilled_text(canvas, index))
    }

    /// Returns the empty text of a field that a binding fills and that still holds the label an
    /// editor gives a new text field: it shows nothing until the game fills it in.
    fn unfilled_text(&self, canvas: &Canvas, index: usize) -> Option<String> {
        let bound = self
            .rests
            .last()
            .is_some_and(|state| state.is_text_bound(index));
        let label = canvas.widgets.get(index)?.text.as_ref()?.source.as_str();
        (bound && is_editor_label(label)).then(String::new)
    }

    fn parameter_text(&self, canvas: &Canvas, index: usize) -> Option<String> {
        let binding = canvas.text_bindings.get(&index)?;
        let given = self.params.last().and_then(|values| {
            values
                .iter()
                .find(|(name, _)| *name == binding.parameter)
                .map(|(_, value)| value.clone())
        });
        given.or_else(|| binding.default.clone())
    }
}
