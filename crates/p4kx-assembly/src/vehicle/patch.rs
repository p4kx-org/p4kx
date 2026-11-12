// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{HashMap, HashSet, VecDeque};

use super::{ElemOverride, ValueOrigin};
use crate::markup::{Budget, Element};

/// What applying a Modification did, to judge the patch rule of ADR-0050 on real data.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ModificationReport {
    pub(crate) vehicle_children: usize,
    pub(crate) wholesale_children: usize,
    pub(crate) id_children: usize,
    pub(crate) unmatched_children: Vec<String>,
    pub(crate) unmatched_ids: Vec<String>,
    pub(crate) replaced_ids: Vec<String>,
    pub(crate) unmatched_elems: Vec<String>,
}

impl ModificationReport {
    /// Returns how many children of the patch came from a `Vehicle` element and replaced the
    /// base's child of the same name.
    #[must_use]
    pub const fn vehicle_children(&self) -> usize {
        self.vehicle_children
    }

    /// Returns how many children of the patch root had no `id` and replaced the base's child of
    /// the same name, e.g. `Parts`.
    #[must_use]
    pub const fn wholesale_children(&self) -> usize {
        self.wholesale_children
    }

    /// Returns how many children of the patch root had an `id` to merge by.
    #[must_use]
    pub const fn id_children(&self) -> usize {
        self.id_children
    }

    /// Returns the names of the patch's replacing children the base has no child of.
    #[must_use]
    pub fn unmatched_children(&self) -> &[String] {
        &self.unmatched_children
    }

    /// Returns the `id`s of the patch that no base element has.
    #[must_use]
    pub fn unmatched_ids(&self) -> &[String] {
        &self.unmatched_ids
    }

    /// Returns the `id`s of the patch's wrappers that no element has, and none of whose children
    /// has an `id` the base's child of that name has either, so they replaced that child wholesale.
    #[must_use]
    pub fn replaced_ids(&self) -> &[String] {
        &self.replaced_ids
    }

    /// Returns the `idRef`s of the Modification's `Elem`s that no element has.
    #[must_use]
    pub fn unmatched_elems(&self) -> &[String] {
        &self.unmatched_elems
    }
}

/// Returns the `id` of `element`, none when it has none or an empty one.
fn id_of(element: &Element) -> Option<&str> {
    element.attribute("id").filter(|id| !id.is_empty())
}

/// Applies a patch file to the base implementation `base`.
///
/// The patch root is `Modifications` (or `Vehicle`). A `Vehicle` child, and any child without an
/// `id`, such as `Parts`, replaces the child of the same name of the base wholesale. Any other
/// child with an `id` is merged into every element of the base with that `id`: its attributes
/// are set and its children replace the element's, each keeping the attributes of the old child
/// it pairs with (same element name and `name`) that it does not state itself, such as an `id`. A
/// child with an `id` that no base element has is merged into the base's child of its name by the
/// `id`s of its descendants, or replaces it wholesale when none matches, as a wrapper such as
/// `<Parts id="modParts">` does.
///
/// # Errors
///
/// Returns why the budget is spent; `base` is then partly patched and must be discarded.
pub(super) fn apply_patch(
    base: &mut Element,
    patch: &Element,
    budget: &mut Budget,
) -> Result<ModificationReport, String> {
    let mut report = ModificationReport::default();
    let mut replaced = HashSet::new();
    if patch.name.eq_ignore_ascii_case("Vehicle") {
        replace_children(base, patch, &mut report, &mut replaced, budget)?;
        return Ok(report);
    }
    for child in &patch.children {
        if child.name.eq_ignore_ascii_case("Vehicle") {
            replace_children(base, child, &mut report, &mut replaced, budget)?;
        } else {
            apply_child(base, child, &mut report, &mut replaced, budget)?;
        }
    }
    Ok(report)
}

/// Applies one child of the patch root that is not a `Vehicle`: a child with an `id` some base
/// element has is merged by `id`; a wrapper whose `id` no element has but whose name a base child
/// has is merged into that child by the `id`s of its descendants, or replaces it wholesale when
/// none of them matches; anything else replaces the base's child of its name wholesale.
fn apply_child(
    base: &mut Element,
    child: &Element,
    report: &mut ModificationReport,
    replaced: &mut HashSet<String>,
    budget: &mut Budget,
) -> Result<(), String> {
    let Some(id) = id_of(child) else {
        report.wholesale_children = report.wholesale_children.saturating_add(1);
        return replace_child(base, child, report, replaced, budget);
    };
    if merge_by_id(base, id, child, budget)? > 0 {
        report.id_children = report.id_children.saturating_add(1);
        return Ok(());
    }
    let Some(at) = base
        .children
        .iter()
        .position(|existing| existing.name.eq_ignore_ascii_case(&child.name))
    else {
        report.unmatched_ids.push(id.to_owned());
        return Ok(());
    };
    let mut matched = 0_usize;
    let mut missed = Vec::new();
    if let Some(target) = base.children.get_mut(at) {
        let mut addressed = Vec::new();
        top_ids(child, &mut addressed);
        for part in addressed {
            let Some(part_id) = id_of(part) else {
                continue;
            };
            if merge_by_id(target, part_id, part, budget)? > 0 {
                matched = matched.saturating_add(1);
            } else {
                missed.push(part_id.to_owned());
            }
        }
    }
    if matched > 0 {
        report.id_children = report.id_children.saturating_add(1);
        report.unmatched_ids.extend(missed);
    } else {
        report.replaced_ids.push(id.to_owned());
        report.wholesale_children = report.wholesale_children.saturating_add(1);
        replace_child(base, child, report, replaced, budget)?;
    }
    Ok(())
}

/// Collects the descendants of `element` that have an `id`, not looking below one that has.
fn top_ids<'a>(element: &'a Element, found: &mut Vec<&'a Element>) {
    for child in &element.children {
        if id_of(child).is_some() {
            found.push(child);
        } else {
            top_ids(child, found);
        }
    }
}

fn replace_children(
    base: &mut Element,
    source: &Element,
    report: &mut ModificationReport,
    replaced: &mut HashSet<String>,
    budget: &mut Budget,
) -> Result<(), String> {
    for child in &source.children {
        report.vehicle_children = report.vehicle_children.saturating_add(1);
        replace_child(base, child, report, replaced, budget)?;
    }
    Ok(())
}

/// Replaces the first base child named like `child` by `child`, leaving further base children of
/// that name; when the patch has several children of that name, the others follow the last such
/// child, so that none replaces another.
fn replace_child(
    base: &mut Element,
    child: &Element,
    report: &mut ModificationReport,
    replaced: &mut HashSet<String>,
    budget: &mut Budget,
) -> Result<(), String> {
    if child.name.eq_ignore_ascii_case("Modifications") {
        return Ok(());
    }
    budget.charge_copy(child)?;
    let same = |existing: &Element| existing.name.eq_ignore_ascii_case(&child.name);
    let mut added = child.clone();
    added.mark_all(ValueOrigin::Patch);
    if replaced.insert(child.name.to_ascii_lowercase()) {
        if let Some(at) = base.children.iter().position(same) {
            if let Some(slot) = base.children.get_mut(at) {
                *slot = added;
            }
        } else {
            report.unmatched_children.push(child.name.clone());
            base.children.push(added);
        }
    } else {
        let after = base
            .children
            .iter()
            .rposition(same)
            .map_or(base.children.len(), |last| last + 1);
        base.children.insert(after, added);
    }
    Ok(())
}

/// Returns the children `new` as the replacement of the children `old`: each is paired with the
/// first unused old child of the same element name and `name` attribute, and takes over the
/// attributes of its partner that it does not state itself, such as an `id` a Modification
/// addresses; old children without a partner are dropped.
fn overlay_children(
    old: Vec<Element>,
    new: &[Element],
    budget: &mut Budget,
) -> Result<Vec<Element>, String> {
    let mut partners: HashMap<(String, String), VecDeque<Element>> = HashMap::new();
    for element in old {
        partners
            .entry(pair_key(&element))
            .or_default()
            .push_back(element);
    }
    let mut children = Vec::with_capacity(new.len());
    for source in new {
        budget.charge_shallow(source)?;
        let partner = partners
            .get_mut(&pair_key(source))
            .and_then(VecDeque::pop_front);
        let mut element = Element {
            name: source.name.clone(),
            port_name: source.port_name.clone(),
            attributes: source.attributes.clone(),
            children: Vec::new(),
            origins: Vec::new(),
        };
        for (name, _) in &source.attributes {
            element.mark(name, ValueOrigin::Patch);
        }
        let old_children = match partner {
            Some(partner) => {
                for (name, value) in &partner.attributes {
                    if element.attribute(name).is_none() {
                        budget.charge_set(element.attributes.len(), name, value)?;
                        element.set_attribute(name, value);
                        if name.eq_ignore_ascii_case("name") {
                            element.port_name.clone_from(&partner.port_name);
                        }
                        match partner.origin_of(name) {
                            ValueOrigin::Base => {}
                            origin => element.mark(name, origin),
                        }
                    }
                }
                partner.children
            }
            None => Vec::new(),
        };
        element.children = overlay_children(old_children, &source.children, budget)?;
        children.push(element);
    }
    Ok(children)
}

fn pair_key(element: &Element) -> (String, String) {
    (
        element.name.to_ascii_lowercase(),
        element
            .attribute("name")
            .unwrap_or_default()
            .to_ascii_lowercase(),
    )
}

fn merge_by_id(
    element: &mut Element,
    id: &str,
    patch: &Element,
    budget: &mut Budget,
) -> Result<usize, String> {
    let mut matched = 0_usize;
    for child in &mut element.children {
        if id_of(child) == Some(id) {
            matched = matched.saturating_add(1);
            for (name, value) in &patch.attributes {
                budget.charge_set(child.attributes.len(), name, value)?;
                child.set_attribute(name, value);
                if name.eq_ignore_ascii_case("name") {
                    child.port_name.clone_from(&patch.port_name);
                }
                child.mark(name, ValueOrigin::Patch);
            }
            let old = std::mem::take(&mut child.children);
            child.children = overlay_children(old, &patch.children, budget)?;
        } else if !child.name.eq_ignore_ascii_case("Modifications") {
            matched = matched.saturating_add(merge_by_id(child, id, patch, budget)?);
        }
    }
    Ok(matched)
}

/// Applies the `Elem` overrides to every element of `base` whose `id` they name; returns the
/// `idRef`s that no element has.
///
/// # Errors
///
/// Returns why the budget is spent; `base` is then partly changed and must be discarded.
pub(super) fn apply_elems(
    base: &mut Element,
    elems: &[ElemOverride],
    budget: &mut Budget,
) -> Result<Vec<String>, String> {
    if elems.is_empty() {
        return Ok(Vec::new());
    }
    let mut by_id: HashMap<&str, Vec<&ElemOverride>> = HashMap::new();
    for elem in elems {
        by_id.entry(elem.id_ref.as_str()).or_default().push(elem);
    }
    let mut seen = HashSet::new();
    set_elems(base, &by_id, &mut seen, budget)?;
    let mut missed: Vec<String> = by_id
        .keys()
        .filter(|id| !seen.contains(**id))
        .map(|id| (*id).to_owned())
        .collect();
    missed.sort_unstable();
    Ok(missed)
}

fn set_elems<'e>(
    element: &mut Element,
    by_id: &HashMap<&'e str, Vec<&ElemOverride>>,
    seen: &mut HashSet<&'e str>,
    budget: &mut Budget,
) -> Result<(), String> {
    if let Some(id) = id_of(element)
        && let Some((key, elems)) = by_id.get_key_value(id)
    {
        seen.insert(*key);
        for elem in elems {
            budget.charge_set(element.attributes.len(), &elem.name, &elem.value)?;
            element.set_attribute(&elem.name, &elem.value);
            if elem.name.eq_ignore_ascii_case("name") {
                element.port_name = elem.port_name.clone();
            }
            element.mark(&elem.name, ValueOrigin::Elem);
        }
    }
    for child in &mut element.children {
        if !child.name.eq_ignore_ascii_case("Modifications") {
            set_elems(child, by_id, seen, budget)?;
        }
    }
    Ok(())
}
