// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;

use p4kx_dataforge::Guid;

use super::condition::Condition;
use super::entry::Entry;
use super::matching::TagPaths;
use crate::canvas::Canvas;

/// The Record path of the canvas of the standard text field, which holds the font and the font
/// size of the text presets (`Heading1`, `Body`, ...) of every text field.
pub(crate) const CANVAS: &str =
    "libs/foundry/records/ui/buildingblocks/modularkit/standard/widgets/textfieldwidgetstandard";

/// The tag path of the preset tags, which the entries of the canvas select; the name of a preset
/// in lower case follows it.
const TAG_PATH: &str = "UI.Style._Standard.ModularKit.Widgets.textfield.textstyle-";

/// The presets of a text field, as `labelProperties.style` names them.
const PRESETS: [&str; 12] = [
    "Title1", "Title2", "Title3", "Title4", "Title5", "Heading1", "Heading2", "Heading3",
    "Heading4", "Heading5", "Heading6", "Body",
];

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

    const fn is_empty(self) -> bool {
        self.font_style.is_none() && self.font_size.is_none()
    }
}

/// The style each text preset of a text field has under one brand: what the brand style of the
/// standard text field sets over its defaults.
#[derive(Debug, Default)]
pub(crate) struct PresetStyles {
    by_name: HashMap<&'static str, PresetStyle>,
}

/// Returns the style of every entry that selects one preset tag alone and sets a font or a size;
/// entries with several tags or other selectors are left out.
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
    /// Reads the styles of the presets from the canvas of the standard text field under the brand
    /// `code`: the entries of the brand style `s_<code>_` over the defaults; without a brand style
    /// of that code, the defaults.
    pub(crate) fn read(canvas: &Canvas, tags: &TagPaths, code: Option<&str>) -> Self {
        let defaults = styles_of(&canvas.defaults);
        let brand = code
            .and_then(|code| canvas.brand_of(code))
            .map(|brand| styles_of(&brand.entries))
            .unwrap_or_default();
        let by_name = PRESETS
            .into_iter()
            .filter_map(|name| {
                let tag = tags.guid_of(&format!("{TAG_PATH}{}", name.to_ascii_lowercase()))?;
                let base = defaults.get(&tag).copied().unwrap_or_default();
                let style = brand.get(&tag).map_or(base, |own| own.over(base));
                (!style.is_empty()).then_some((name, style))
            })
            .collect();
        Self { by_name }
    }

    /// Returns the style of the preset `name`, when the canvas sets one.
    pub(crate) fn style(&self, name: &str) -> Option<PresetStyle> {
        self.by_name.get(name).copied()
    }
}
