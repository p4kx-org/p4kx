// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;

use p4kx_dataforge::Guid;

use super::condition::Condition;
use super::entry::Entry;
use crate::canvas::Canvas;

/// The Record path of the canvas of the standard text field, which holds the font and the font
/// size of the text presets (`Heading1`, `Body`, ...) of every text field.
pub(crate) const CANVAS: &str =
    "libs/foundry/records/ui/buildingblocks/modularkit/standard/widgets/textfieldwidgetstandard";

/// The tag path of the preset tags, which the entries of the canvas select; the name of a preset
/// in lower case follows it.
pub(crate) const TAG_PATH: &str = "UI.Style._Standard.ModularKit.Widgets.textfield.textstyle-";

/// What a text preset sets: the font style Record and the size of the text.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct PresetStyle {
    pub(crate) font_style: Option<Guid>,
    pub(crate) font_size: Option<f32>,
}

impl PresetStyle {
    /// Returns the style with what `self` sets and, where it sets nothing, what `under` sets.
    fn over(self, under: Self) -> Self {
        Self {
            font_style: self.font_style.or(under.font_style),
            font_size: self.font_size.or(under.font_size),
        }
    }

    const fn is_empty(&self) -> bool {
        self.font_style.is_none() && self.font_size.is_none()
    }
}

/// The style each text preset of a text field has, by its tag: the defaults of the standard text
/// field and, per brand style, what that brand sets over them.
#[derive(Debug, Default)]
pub(crate) struct PresetStyles {
    defaults: HashMap<Guid, PresetStyle>,
    brands: HashMap<Guid, HashMap<Guid, PresetStyle>>,
}

/// Returns the style of every entry that selects one preset tag alone and sets a font or a size.
fn styles_of(entries: &[Entry]) -> HashMap<Guid, PresetStyle> {
    entries
        .iter()
        .filter_map(|entry| {
            let [list] = entry.lists.as_slice() else {
                return None;
            };
            let [Condition::Tag(tag)] = list.as_slice() else {
                return None;
            };
            let style = PresetStyle {
                font_style: entry.modifiers.text.font_style,
                font_size: entry.modifiers.text.font_size,
            };
            (!style.is_empty()).then_some((*tag, style))
        })
        .collect()
}

impl PresetStyles {
    /// Reads the styles from the canvas of the standard text field.
    pub(crate) fn read(canvas: &Canvas) -> Self {
        Self {
            defaults: styles_of(&canvas.defaults),
            brands: canvas
                .brands
                .iter()
                .map(|brand| (brand.style.guid, styles_of(&brand.entries)))
                .collect(),
        }
    }

    /// Returns the style of the preset `tag` under the brand style `brand`: what the brand sets,
    /// over the default.
    pub(crate) fn style(&self, tag: Guid, brand: Option<Guid>) -> PresetStyle {
        let default = self.defaults.get(&tag).copied().unwrap_or_default();
        brand
            .and_then(|brand| self.brands.get(&brand))
            .and_then(|styles| styles.get(&tag))
            .map_or(default, |own| own.over(default))
    }
}
