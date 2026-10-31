// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_dataforge::{Guid, Reference, StructValue, Value};

use crate::colour::ColorSpec;
use crate::reader::{as_struct, child, flag, float, items, reference, text};
use crate::style::{GeometryModifiers, PictureModifiers, TextModifiers};

use super::animation::{ANIMATION, settled};
use super::condition::{Condition, Uses, collect_unsupported, conditions};
use super::sheet::merge;

#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Modifiers {
    pub(crate) background: Option<ColorSpec>,
    pub(crate) fill: Option<ColorSpec>,
    pub(crate) stroke: Option<ColorSpec>,
    pub(crate) border_colours: [Option<ColorSpec>; 4],
    pub(crate) border_widths: [Option<f32>; 4],
    pub(crate) enable_background: Option<bool>,
    pub(crate) active: Option<bool>,
    pub(crate) alpha: Option<f32>,
    /// The canvas a canvas widget holds (`CanvasReferenceRecord`).
    pub(crate) canvas: Option<Guid>,
    pub(crate) text: TextModifiers,
    pub(crate) picture: PictureModifiers,
    pub(crate) geometry: GeometryModifiers,
}

impl Modifiers {
    /// Returns the modifiers a text draws with: colours, background and text settings.
    pub(crate) fn for_part(&self) -> Self {
        Self {
            background: self.background.clone(),
            fill: self.fill.clone(),
            stroke: self.stroke.clone(),
            enable_background: self.enable_background,
            text: self.text.clone(),
            ..Self::default()
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Entry {
    pub(crate) name: String,
    pub(crate) lists: Vec<Vec<Condition>>,
    pub(crate) modifiers: Modifiers,
    pub(crate) unsupported: Vec<String>,
    pub(crate) uses: Uses,
}

pub(crate) fn parse_entries(values: &[Value<'_>]) -> Vec<Entry> {
    values
        .iter()
        .filter_map(as_struct)
        .map(parse_entry)
        .filter(|entry| entry.modifiers != Modifiers::default())
        .collect()
}

fn parse_entry(entry: &StructValue<'_>) -> Entry {
    let lists: Vec<Vec<Condition>> = items(entry, "conditionsList")
        .iter()
        .filter_map(as_struct)
        .map(|list| conditions(items(list, "conditions")))
        .collect();
    let mut unsupported = Vec::new();
    for condition in lists.iter().flatten() {
        collect_unsupported(condition, &mut unsupported);
    }
    let uses = lists
        .iter()
        .fold(Uses::default(), |found, list| found.or(Uses::of(list)));
    Entry {
        name: text(entry, "name").unwrap_or_default().to_owned(),
        uses,
        lists,
        modifiers: modifiers(items(entry, "modifiers").iter().filter_map(as_struct)),
        unsupported,
    }
}

const BORDER_SIDES: [&str; 4] = ["Top", "Right", "Bottom", "Left"];
const CANVAS_REFERENCE: &str = "BuildingBlocks_FieldModifierRecordRefTypeCanvasReferenceRecord";

pub(crate) fn modifiers<'a, 'f: 'a>(
    structs: impl Iterator<Item = &'a StructValue<'f>>,
) -> Modifiers {
    let mut found = Modifiers::default();
    for modifier in structs {
        if modifier.type_name() == ANIMATION {
            if let Some(settled) = settled(modifier) {
                merge(&mut found, &settled);
            }
            continue;
        }
        if matches!(
            modifier.type_name(),
            "BuildingBlocks_FieldModifierRecordRef" | "BuildingBlocks_FieldModifierEnumerated"
        ) {
            if let Some(inner) = child(modifier, "field") {
                found.text.typed(inner);
                found.geometry.typed(inner);
                if inner.type_name() == CANVAS_REFERENCE {
                    found.canvas = reference(inner, "value").map(Reference::guid);
                }
            }
            continue;
        }
        let Some(field) = text(modifier, "field") else {
            continue;
        };
        match modifier.type_name() {
            "BuildingBlocks_FieldModifierColor" => {
                let colour = modifier.field("color").and_then(ColorSpec::parse);
                colour_field(&mut found, field, colour);
            }
            "BuildingBlocks_FieldModifierBoolean" => {
                let value = flag(modifier, "value");
                match field {
                    "EnableBackground" => found.enable_background = value,
                    "IsActive" => found.active = value,
                    _ if found.geometry.flag(field, value) => {}
                    _ => {
                        found.text.boolean(field, value);
                        found.picture.boolean(field, value);
                    }
                }
            }
            "BuildingBlocks_FieldModifierNumber" => {
                number_field(&mut found, field, float(modifier, "value"));
            }
            "BuildingBlocks_FieldModifierString" => {
                found.picture.string(field, text(modifier, "value"));
            }
            "BuildingBlocks_FieldModifierLocalization" => {
                found.text.localised(field, text(modifier, "value"));
            }
            _ => {}
        }
    }
    found
}

fn colour_field(found: &mut Modifiers, field: &str, colour: Option<ColorSpec>) {
    match field {
        "BackgroundColor" => found.background = colour,
        "FillColor" => found.fill = colour,
        "StrokeColor" => found.stroke = colour,
        _ => {
            if let Some(side) = field.strip_prefix("BorderColor")
                && let Some(index) = BORDER_SIDES.iter().position(|name| *name == side)
                && let Some(slot) = found.border_colours.get_mut(index)
            {
                *slot = colour;
            }
        }
    }
}

fn number_field(found: &mut Modifiers, field: &str, value: Option<f32>) {
    if field == "Alpha" {
        found.alpha = value;
        return;
    }
    if found.geometry.number(field, value) {
        return;
    }
    found.text.number(field, value);
    if let Some(side) = field
        .strip_prefix("Border")
        .and_then(|rest| rest.strip_suffix("Width"))
        && let Some(index) = BORDER_SIDES.iter().position(|name| *name == side)
        && let Some(slot) = found.border_widths.get_mut(index)
    {
        *slot = value;
    }
}

pub(crate) fn inline_entries(widget: &StructValue<'_>) -> Vec<Entry> {
    parse_entries(items(widget, "inlineStyles"))
}
