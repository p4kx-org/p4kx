// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;

use p4kx_dataforge::{Guid, RecordTree, StructValue, Value};
use p4kx_extraction::SourceLocation;

use super::animation::{ANIMATION, settled};
use super::condition::Condition;
use super::entry::parse_entry;
use super::matching::TagPaths;
use crate::canvas::brand_pattern;
use crate::colour::ColorSpec;
use crate::reader::{as_struct, child, items, reference, text};

/// The Record path of the canvas holding the standard `TextField` presets.
pub(crate) const CANVAS: &str =
    "libs/foundry/records/ui/buildingblocks/modularkit/standard/widgets/textfieldwidgetstandard";

const TAG_PATH: &str = "UI.Style._Standard.ModularKit.Widgets.textfield.textstyle-";

const PRESETS: [&str; 12] = [
    "Title1", "Title2", "Title3", "Title4", "Title5", "Heading1", "Heading2", "Heading3",
    "Heading4", "Heading5", "Heading6", "Body",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Branch {
    Default,
    Brand(usize),
}

/// The original standard Canvas modifier selected for text paint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct PresetFillSource {
    record_index: u32,
    branch: Branch,
    entry: usize,
    modifier: usize,
}

impl PresetFillSource {
    pub(crate) fn location(self) -> SourceLocation {
        let branch = match self.branch {
            Branch::Default => "defaultStyles".to_owned(),
            Branch::Brand(index) => format!("brandStyles[{index}]"),
        };
        SourceLocation::DataForge {
            record_index: self.record_index,
            field: format!(
                "{branch}.entries[{}].modifiers[{}].color",
                self.entry, self.modifier
            ),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PresetFill {
    pub(crate) colour: ColorSpec,
    pub(crate) source: PresetFillSource,
}

#[derive(Debug, Default)]
pub(crate) struct PresetStyle {
    pub(crate) font_style: Option<Guid>,
    pub(crate) font_size: Option<f32>,
    pub(crate) fill: Option<PresetFill>,
}

impl PresetStyle {
    fn over(self, under: Self) -> Self {
        Self {
            font_style: self.font_style.or(under.font_style),
            font_size: self.font_size.or(under.font_size),
            fill: self.fill.or(under.fill),
        }
    }

    const fn is_empty(&self) -> bool {
        self.font_style.is_none() && self.font_size.is_none() && self.fill.is_none()
    }
}

/// Presets from the default and selected brand branches of the standard `TextField` Canvas.
#[derive(Debug, Default)]
pub(crate) struct PresetStyles {
    by_name: HashMap<&'static str, PresetStyle>,
}

fn styles_of(
    values: &[Value<'_>],
    record_index: u32,
    branch: Branch,
) -> HashMap<Guid, PresetStyle> {
    let mut styles = HashMap::<Guid, PresetStyle>::new();
    for (slot, value) in values.iter().enumerate() {
        let Some(original) = as_struct(value) else {
            continue;
        };
        let entry = parse_entry(original);
        let [list] = entry.lists.as_slice() else {
            continue;
        };
        let [Condition::Tag(tag)] = list.as_slice() else {
            continue;
        };
        let style = styles.entry(*tag).or_default();
        if entry.modifiers.text.font_style.is_some() || entry.modifiers.text.font_size.is_some() {
            style.font_style = entry.modifiers.text.font_style;
            style.font_size = entry.modifiers.text.font_size;
        }
        if let Some((modifier, colour)) = direct_fill(original) {
            style.fill = Some(PresetFill {
                colour,
                source: PresetFillSource {
                    record_index,
                    branch,
                    entry: slot,
                    modifier,
                },
            });
        }
    }
    styles
}

fn direct_fill(entry: &StructValue<'_>) -> Option<(usize, ColorSpec)> {
    let [list] = items(entry, "conditionsList") else {
        return None;
    };
    let [_] = items(as_struct(list)?, "conditions") else {
        return None;
    };
    let modifiers = items(entry, "modifiers").iter().enumerate().rev();
    for (slot, modifier) in modifiers {
        let Some(modifier) = as_struct(modifier) else {
            continue;
        };
        if modifier.type_name() == ANIMATION
            && settled(modifier).is_some_and(|style| style.fill.is_some())
        {
            return None;
        }
        if modifier.type_name() == "BuildingBlocks_FieldModifierColor"
            && text(modifier, "field") == Some("FillColor")
        {
            return Some((slot, ColorSpec::parse(modifier.field("color")?)?));
        }
    }
    None
}

impl PresetStyles {
    /// Reads the selected brand's fields over the defaults of the standard Canvas.
    pub(crate) fn read(canvas: &RecordTree<'_>, tags: &TagPaths, code: Option<&str>) -> Self {
        let root = canvas.root();
        let index = canvas.record_index();
        let mut defaults = child(root, "defaultStyles").map_or_else(HashMap::new, |branch| {
            styles_of(items(branch, "entries"), index, Branch::Default)
        });
        let mut brand = code.map_or_else(HashMap::new, |code| {
            let wanted = brand_pattern(code);
            items(root, "brandStyles")
                .iter()
                .enumerate()
                .filter_map(|(slot, value)| Some((slot, as_struct(value)?)))
                .find(|(_, branch)| {
                    reference(branch, "brandIdentifier").is_some_and(|style| {
                        style.record_path().to_ascii_lowercase().contains(&wanted)
                    })
                })
                .map_or_else(HashMap::new, |(slot, branch)| {
                    styles_of(items(branch, "entries"), index, Branch::Brand(slot))
                })
        });
        let by_name = PRESETS
            .into_iter()
            .filter_map(|name| {
                let tag = tags.guid_of(&format!("{TAG_PATH}{}", name.to_ascii_lowercase()))?;
                let base = defaults.remove(&tag).unwrap_or_default();
                let style = match brand.remove(&tag) {
                    Some(chosen) => chosen.over(base),
                    None => base,
                };
                (!style.is_empty()).then_some((name, style))
            })
            .collect();
        Self { by_name }
    }

    pub(crate) fn style(&self, name: &str) -> Option<&PresetStyle> {
        self.by_name.get(name)
    }
}

#[cfg(test)]
#[path = "preset_tests.rs"]
mod tests;
