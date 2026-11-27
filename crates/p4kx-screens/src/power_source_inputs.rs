// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_dataforge::{DataForge, Guid, RecordTree, Reference, Step, StructValue};
use p4kx_gfx::Rgba;

use super::{Check, DISPLAY, FONT_STYLE, black};
use crate::WidgetInput;
use crate::colour::{ColorSpec, PaletteSite, scale_alpha};
use crate::reader::{as_struct, child, flag, items, reference, text};

pub(super) fn tree<'a>(forge: &'a DataForge<'_>, index: u32) -> Check<RecordTree<'a>> {
    forge
        .record(index)
        .ok_or("missing physical source Record")?
        .tree()
        .map_err(|_| "cannot open physical source Record")
}

pub(super) fn widget_source<'a, 'b>(
    tree: &'a RecordTree<'b>,
    input: WidgetInput,
) -> Check<&'a StructValue<'b>> {
    items(tree.root(), input.section.field_name())
        .get(input.slot)
        .and_then(as_struct)
        .ok_or("missing original widget occurrence")
}

pub(super) fn same_widget(reference: &Reference<'_>, guid: Guid, input: WidgetInput) -> bool {
    reference.guid() == guid
        && matches!(reference.location().map(p4kx_dataforge::Location::steps),
            Some([Step::Field(field), Step::Element(slot)])
                if *field == input.section.field_name() && usize::try_from(*slot).ok() == Some(input.slot))
}

pub(super) fn check_pair(
    tree: &RecordTree<'_>,
    label: WidgetInput,
    parent: WidgetInput,
    slot: &str,
    font: Guid,
) -> Check {
    if parent.record_index != label.record_index {
        return Err("painted parent and label are in different original Records");
    }
    let label = widget_source(tree, label)?;
    let parent_source = widget_source(tree, parent)?;
    if !reference(label, "parent")
        .is_some_and(|reference| same_widget(reference, tree.guid(), parent))
    {
        return Err("painted parent disagrees with the original parent Reference");
    }
    let background = child(parent_source, "background").ok_or("no original parent background")?;
    if parent_source.type_name() != DISPLAY
        || flag(background, "enable") != Some(true)
        || !matches!(background.field("color").and_then(ColorSpec::parse),
            Some(ColorSpec::Slot { name, .. }) if name == slot)
    {
        return Err("the original DisplayWidget source signature changed");
    }
    let inline = || {
        items(label, "inlineStyles")
            .iter()
            .filter_map(as_struct)
            .flat_map(|entry| items(entry, "modifiers").iter().filter_map(as_struct))
    };
    if !inline().any(|modifier| {
        text(modifier, "field") == Some("FillColor")
            && black(modifier.field("color").and_then(ColorSpec::parse).as_ref())
    }) {
        return Err("no original black inline label fill");
    }
    let original_font = reference(label, "fontStyle").is_some_and(|value| value.guid() == font)
        || inline().any(|modifier| {
            child(modifier, "field")
                .filter(|inner| inner.type_name() == FONT_STYLE)
                .and_then(|inner| reference(inner, "value"))
                .is_some_and(|value| value.guid() == font)
        });
    if !original_font {
        return Err("no original declared font matches the effective label font");
    }
    Ok(())
}

pub(super) fn heavy_font(forge: &DataForge<'_>, font: Guid) -> Check<bool> {
    let record = forge
        .record_by_guid(font)
        .ok_or("missing original font Record")?;
    let tree = record
        .tree()
        .map_err(|_| "cannot open original font Record")?;
    Ok(text(tree.root(), "paintFile").is_some_and(|file| {
        let filename = file.rsplit(['/', '\\']).next().unwrap_or(file);
        let stem = filename.rsplit_once('.').map_or(filename, |(stem, _)| stem);
        stem.eq_ignore_ascii_case("blenderpro-heavy")
            || stem.eq_ignore_ascii_case("blenderproheavy")
    }))
}

pub(super) fn palette_colour(
    forge: &DataForge<'_>,
    slots: &[String],
    slot: &str,
    site: PaletteSite,
) -> Check<Rgba> {
    if slots.get(site.slot).map(String::as_str) != Some(slot) {
        return Err("palette enumeration and physical source slot disagree");
    }
    let style = tree(forge, site.record_index)?;
    let source = items(style.root(), "colorStyles")
        .get(site.slot)
        .and_then(ColorSpec::parse);
    match source {
        Some(ColorSpec::Solid(colour)) => Ok(colour),
        Some(ColorSpec::Record { guid, alpha }) => {
            let record = forge
                .record_by_guid(guid)
                .ok_or("missing original colour Record")?;
            let colour = record
                .tree()
                .map_err(|_| "cannot open original colour Record")?;
            let Some(ColorSpec::Solid(value)) = colour.field("color").and_then(ColorSpec::parse)
            else {
                return Err("the referenced colour is not a solid original value");
            };
            Ok(scale_alpha(value, alpha))
        }
        _ => Err("the selected palette slot has an unrecognized source shape"),
    }
}
