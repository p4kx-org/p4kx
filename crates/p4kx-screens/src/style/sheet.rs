// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::rc::Rc;

use p4kx_dataforge::{DataForge, Guid};

use super::condition::{Uses, selects_part};
use super::entry::{Entry, Modifiers, parse_entries};
use super::matching::{Subject, TagPaths, applies, applies_to_lists};
use crate::canvas::Canvas;
use crate::reader::items;
use crate::widget::Widget;

pub(crate) type StyleCache = HashMap<Guid, Rc<[Entry]>>;

#[derive(Debug, Default)]
pub(crate) struct CanvasStyles {
    groups: Vec<Rc<[Entry]>>,
    uses: Uses,
}

impl CanvasStyles {
    pub(crate) fn build(
        forge: &DataForge<'_>,
        cache: &mut StyleCache,
        canvas: &Canvas,
        brand: Option<&str>,
    ) -> Self {
        let mut groups = Vec::new();
        for style in &canvas.shared {
            groups.push(style_entries(forge, cache, style.guid));
        }
        groups.push(Rc::clone(&canvas.defaults));
        if let Some(chosen) = canvas.pick_brand(brand) {
            groups.push(style_entries(forge, cache, chosen.style.guid));
            groups.push(Rc::clone(&chosen.entries));
        }
        groups.push(Rc::clone(&canvas.embedded));
        let uses = groups
            .iter()
            .flat_map(|group| group.iter())
            .chain(
                canvas
                    .widgets
                    .iter()
                    .flat_map(|widget| widget.inline.iter()),
            )
            .fold(Uses::default(), |found, entry| found.or(entry.uses));
        Self { groups, uses }
    }

    /// Returns the sheet of the entries of the style Record `style`, which a canvas widget lays
    /// over the canvas it holds.
    pub(crate) fn of_style(forge: &DataForge<'_>, cache: &mut StyleCache, style: Guid) -> Self {
        let group = style_entries(forge, cache, style);
        let uses = group
            .iter()
            .fold(Uses::default(), |found, entry| found.or(entry.uses));
        Self {
            groups: vec![group],
            uses,
        }
    }

    /// Returns the canvases the styles and bindings of `canvas` may put in one of its canvas
    /// widgets, whatever the brand: those of its shared styles, defaults, embedded styles, every
    /// brand's styles and entries, its inline styles and its operations.
    pub(crate) fn swapped_canvases(
        forge: &DataForge<'_>,
        cache: &mut StyleCache,
        canvas: &Canvas,
    ) -> Vec<Guid> {
        let styles = canvas
            .shared
            .iter()
            .chain(canvas.brands.iter().map(|brand| &brand.style));
        let mut groups: Vec<Rc<[Entry]>> = styles
            .map(|style| style_entries(forge, cache, style.guid))
            .collect();
        groups.push(Rc::clone(&canvas.defaults));
        groups.push(Rc::clone(&canvas.embedded));
        groups.extend(canvas.brands.iter().map(|brand| Rc::clone(&brand.entries)));
        let inline = canvas
            .widgets
            .iter()
            .flat_map(|widget| widget.inline.iter());
        groups
            .iter()
            .flat_map(|group| group.iter())
            .chain(inline)
            .filter_map(|entry| entry.modifiers.canvas)
            .chain(canvas.operations.canvas_choices())
            .collect()
    }

    /// Returns which selectors that need the shape of the tree the styles hold.
    pub(crate) const fn uses(&self) -> Uses {
        self.uses
    }

    /// Returns the canvases the entries of the sheet may put in a canvas widget.
    pub(crate) fn canvases(&self) -> impl Iterator<Item = Guid> + '_ {
        self.groups
            .iter()
            .flat_map(|group| group.iter())
            .filter_map(|entry| entry.modifiers.canvas)
    }

    pub(crate) fn unsupported(&self) -> impl Iterator<Item = (&str, &str)> {
        self.groups.iter().flat_map(|group| {
            group.iter().flat_map(|entry| {
                entry
                    .unsupported
                    .iter()
                    .map(move |selector| (entry.name.as_str(), selector.as_str()))
            })
        })
    }
}

fn style_entries(forge: &DataForge<'_>, cache: &mut StyleCache, style: Guid) -> Rc<[Entry]> {
    if let Some(cached) = cache.get(&style) {
        return Rc::clone(cached);
    }
    let entries: Rc<[Entry]> = forge
        .record_by_guid(style)
        .and_then(|record| record.tree().ok())
        .map(|tree| parse_entries(items(tree.root(), "entries")))
        .unwrap_or_default()
        .into();
    cache.insert(style, Rc::clone(&entries));
    entries
}

pub(crate) fn resolve(
    sheets: &[Rc<CanvasStyles>],
    widget: &Widget,
    (subject, above): (&Subject, &[Subject]),
    tags: &TagPaths,
    budget: &mut u64,
) -> Modifiers {
    let mut resolved = Modifiers::default();
    let all = sheets
        .iter()
        .flat_map(|sheet| sheet.groups.iter())
        .flat_map(|group| group.iter())
        .chain(widget.inline.iter());
    for entry in all {
        if *budget == 0 {
            break;
        }
        if applies(entry, subject, above, tags, budget) {
            merge(&mut resolved, &entry.modifiers);
        }
    }
    resolved
}

/// Resolves the entries of `sheets` that select the inner part of a widget, the text of a text
/// field or the shape of an icon: those that require the type of the part and those that name no
/// type and select by the parent (ADR-0296), by the selector lists that do so only. Other untyped
/// entries and inline styles are left to the widget, and only the modifiers a part draws with
/// apply.
pub(crate) fn resolve_part(
    sheets: &[Rc<CanvasStyles>],
    (subject, above): (&Subject, &[Subject]),
    tags: &TagPaths,
    budget: &mut u64,
) -> Modifiers {
    let mut resolved = Modifiers::default();
    let all = sheets
        .iter()
        .flat_map(|sheet| sheet.groups.iter())
        .flat_map(|group| group.iter());
    for entry in all {
        if *budget == 0 {
            break;
        }
        if applies_to_lists(entry, selects_part, (subject, above), tags, budget) {
            merge(&mut resolved, &entry.modifiers.for_part());
        }
    }
    resolved
}

pub(crate) fn merge(into: &mut Modifiers, from: &Modifiers) {
    fn set<T: Clone>(slot: &mut Option<T>, value: Option<&T>) {
        if let Some(value) = value {
            *slot = Some(value.clone());
        }
    }
    set(&mut into.background, from.background.as_ref());
    set(&mut into.fill, from.fill.as_ref());
    set(&mut into.stroke, from.stroke.as_ref());
    set(&mut into.enable_background, from.enable_background.as_ref());
    set(&mut into.active, from.active.as_ref());
    set(&mut into.alpha, from.alpha.as_ref());
    set(&mut into.canvas, from.canvas.as_ref());
    into.text.merge(&from.text);
    into.picture.merge(&from.picture);
    into.geometry.merge(&from.geometry);
    for (slot, value) in into.border_colours.iter_mut().zip(&from.border_colours) {
        set(slot, value.as_ref());
    }
    for (slot, value) in into.border_widths.iter_mut().zip(&from.border_widths) {
        set(slot, value.as_ref());
    }
}
