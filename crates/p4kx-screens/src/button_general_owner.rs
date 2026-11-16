// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::Cell;
use std::collections::HashSet;

use p4kx_dataforge::{
    DataForge, Location, RecordTree, Reference, Step, StructValue, TreeBuilder, Value,
};

use p4kx_gfx::{Localise, Localised, localise_text};

use super::{CANVAS_TYPE, Check, GENERAL, WidgetInput, scene_input};
use crate::button_component::LARGE;
use crate::reader::{as_struct, child, flag, float, items, reference, text};
use crate::text::{is_editor_label, is_placeholder};

const UI_ROOT: &str = "libs/foundry/records/ui/buildingblocks/";
const MFD_PATH: &str = "/mfdscreens/";

pub(super) struct Owner<'f> {
    pub(super) canvas: RecordTree<'f>,
    pub(super) input: WidgetInput,
    pub(super) ancestors: Vec<WidgetInput>,
    pub(super) label_key: String,
    pub(super) on_mfd: bool,
}

impl Owner<'_> {
    fn preference(&self) -> (bool, bool) {
        (self.ancestors.is_empty(), self.on_mfd)
    }
}

pub(super) fn select_owner<'f>(
    forge: &'f DataForge<'_>,
    localise: Localise<'_>,
) -> Check<Owner<'f>> {
    let mut trees = TreeBuilder::new();
    let mut selected: Option<Owner<'f>> = None;
    for record in forge.records() {
        if record.type_name().ok() != Some(CANVAS_TYPE) {
            continue;
        }
        let path = record
            .record_path()
            .map_err(|_| "unreadable original Canvas path")?;
        let on_mfd = path.contains(MFD_PATH);
        if !path.starts_with(UI_ROOT)
            || selected
                .as_ref()
                .is_some_and(|owner| owner.preference() >= (true, on_mfd))
        {
            continue;
        }
        let canvas = trees
            .build(&record)
            .map_err(|_| "cannot read an original Canvas")?;
        let Some(owner) = original_owner(canvas, on_mfd, localise) else {
            continue;
        };
        if owner.preference() == (true, true) {
            return Ok(owner);
        }
        if selected
            .as_ref()
            .is_none_or(|existing| owner.preference() > existing.preference())
        {
            selected = Some(owner);
        }
    }
    selected.ok_or("no original General button with a default frame, localized label and active scene ancestry with a nonempty English translation")
}

pub(in crate::power_source_witness::button) fn large_owner(
    canvas: &RecordTree<'_>,
    localise: Localise<'_>,
) -> Check<(WidgetInput, String)> {
    let scene = items(canvas.root(), "scene");
    let mut holders = scene.iter().enumerate().filter_map(|(slot, value)| {
        let widget = as_struct(value)?;
        (widget.type_name() == LARGE).then_some((slot, widget))
    });
    let (slot, holder) = holders.next().ok_or("no original Large scene holder")?;
    if holders.next().is_some() || !matches!(holder.field("canvas"), Some(Value::Null) | None) {
        return Err("the selected Canvas has no unique implicit Large scene holder");
    }
    active_ancestors(canvas, scene, &visibility_targets(canvas), slot)
        .ok_or("the original Large holder does not have a static active scene ancestry")?;
    let label = child(holder, "labelProperties")
        .filter(|label| flag(label, "show") == Some(true))
        .and_then(|label| text(label, "label"))
        .and_then(|source| english_key(source, localise))
        .ok_or("the original Large label is hidden or has no nonempty English translation")?;
    Ok((scene_input(canvas, slot), label))
}

struct Candidate {
    input: WidgetInput,
    ancestors: Vec<WidgetInput>,
    label_key: String,
}

fn original_owner<'f>(
    canvas: RecordTree<'f>,
    on_mfd: bool,
    localise: Localise<'_>,
) -> Option<Owner<'f>> {
    let selected = candidate(&canvas, localise)?;
    Some(Owner {
        canvas,
        input: selected.input,
        ancestors: selected.ancestors,
        label_key: selected.label_key,
        on_mfd,
    })
}

fn candidate(canvas: &RecordTree<'_>, localise: Localise<'_>) -> Option<Candidate> {
    let targets = visibility_targets(canvas);
    let scene = items(canvas.root(), "scene");
    let mut nested = None;
    for (slot, value) in scene.iter().enumerate() {
        let Some(source) = as_struct(value).and_then(default_label) else {
            continue;
        };
        let Some(ancestors) = active_ancestors(canvas, scene, &targets, slot) else {
            continue;
        };
        let Some(label_key) = english_key(source, localise) else {
            continue;
        };
        let candidate = Candidate {
            input: scene_input(canvas, slot),
            ancestors,
            label_key,
        };
        if candidate.ancestors.is_empty() {
            return Some(candidate);
        }
        if nested.is_none() {
            nested = Some(candidate);
        }
    }
    nested
}

fn visibility_targets(canvas: &RecordTree<'_>) -> HashSet<usize> {
    items(canvas.root(), "operations")
        .iter()
        .filter_map(as_struct)
        .filter(|operation| {
            operation.type_name() == "BuildingBlocks_BindingsBooleanField"
                && matches!(text(operation, "field"), Some("IsActive" | "Instantiated"))
        })
        .filter_map(|operation| scene_slot(canvas, reference(operation, "widget")?))
        .collect()
}

fn scene_slot(canvas: &RecordTree<'_>, reference: &Reference<'_>) -> Option<usize> {
    if reference.guid() != canvas.guid() || reference.record_index() != canvas.record_index() {
        return None;
    }
    let [Step::Field("scene"), Step::Element(slot)] = reference.location().map(Location::steps)?
    else {
        return None;
    };
    usize::try_from(*slot).ok()
}

fn active_ancestors(
    canvas: &RecordTree<'_>,
    scene: &[Value<'_>],
    targets: &HashSet<usize>,
    holder: usize,
) -> Option<Vec<WidgetInput>> {
    let mut next = holder;
    let mut seen = HashSet::new();
    let mut ancestors = Vec::new();
    while seen.insert(next) {
        let widget = scene.get(next).and_then(as_struct)?;
        if targets.contains(&next) || !active(widget) {
            return None;
        }
        if next != holder {
            ancestors.push(scene_input(canvas, next));
        }
        if matches!(widget.field("parent"), Some(Value::Null) | None) {
            return Some(ancestors);
        }
        next = scene_slot(canvas, reference(widget, "parent")?)?;
    }
    None
}

fn active(widget: &StructValue<'_>) -> bool {
    if matches!(
        widget.field("isActive"),
        Some(Value::Bool(false) | Value::Error(_))
    ) {
        return false;
    }
    match widget.field("scale") {
        None | Some(Value::Null) => true,
        Some(Value::Struct(scale)) => ["x", "y"].into_iter().all(|axis| {
            float(scale, axis).is_some_and(|number| number.is_finite() && number != 0.0)
        }),
        _ => false,
    }
}

fn default_label<'a>(widget: &'a StructValue<'_>) -> Option<&'a str> {
    if widget.type_name() != GENERAL
        || !matches!(widget.field("canvas"), Some(Value::Null))
        || text(widget, "fillStyle") != Some("Filled")
        || text(widget, "iconPosition") != Some("Top")
    {
        return None;
    }
    let label = child(widget, "labelProperties")?;
    if flag(label, "show") != Some(true) {
        return None;
    }
    text(label, "label")
}

fn english_key(source: &str, localise: Localise<'_>) -> Option<String> {
    let authored = source.trim();
    if is_editor_label(authored) || is_placeholder(authored) {
        return None;
    }
    let key = Cell::new(None);
    match localise_text(authored, &|name| {
        let value = localise(name)?;
        key.set(Some(name.to_owned()));
        Some(value)
    }) {
        Localised::Text(text) if !text.trim().is_empty() => key.take(),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::english_key;

    #[test]
    fn the_original_label_requires_nonempty_english_and_uses_the_actual_lookup_key() {
        let english = |key: &str| match key {
            "fixture" => Some("Button %s".to_owned()),
            "empty" => Some("   ".to_owned()),
            "ui_leaderboards_Loadout" | "LOC_UNINITIALIZED" => Some("Editor".to_owned()),
            _ => None,
        };
        assert_eq!(
            english_key(" @fixture ,P=1 ", &english).as_deref(),
            Some("fixture"),
            "whitespace and format arguments are not part of the actual lookup key"
        );
        for source in [
            "plain",
            "@missing",
            "@empty",
            "@LOC_UNINITIALIZED",
            " @ui_leaderboards_Loadout ",
        ] {
            assert_eq!(
                english_key(source, &english),
                None,
                "{source}: a General witness requires an actual non-placeholder English label"
            );
        }
    }
}
