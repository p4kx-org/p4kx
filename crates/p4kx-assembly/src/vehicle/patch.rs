// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{HashMap, HashSet};

#[path = "patch_overlay.rs"]
mod overlay;
#[path = "patch_scope.rs"]
mod scope;

use crate::VehicleAggregateEvidence;
use scope::PatchScope;

use super::{ElemOverride, ValueOrigin, VehicleAttributeEvidence};
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
    aggregate: Option<&mut VehicleAggregateEvidence>,
) -> Result<ModificationReport, String> {
    let mut scope = PatchScope::new(aggregate);
    scope.element(patch, "$name");
    let mut report = ModificationReport::default();
    let mut replaced = HashSet::new();
    if patch.name.eq_ignore_ascii_case("Vehicle") {
        replace_children(base, patch, &mut report, &mut replaced, budget, &mut scope)?;
        return Ok(report);
    }
    scope.element(patch, "$children");
    for child in &patch.children {
        scope.element(child, "$name");
        if child.name.eq_ignore_ascii_case("Vehicle") {
            replace_children(base, child, &mut report, &mut replaced, budget, &mut scope)?;
        } else {
            apply_child(base, child, &mut report, &mut replaced, budget, &mut scope)?;
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
    scope: &mut PatchScope<'_>,
) -> Result<(), String> {
    scope.field(child, "id");
    let Some(id) = id_of(child) else {
        report.wholesale_children = report.wholesale_children.saturating_add(1);
        return replace_child(base, child, report, replaced, budget, scope);
    };
    if merge_by_id(base, id, child, budget, scope)? > 0 {
        report.id_children = report.id_children.saturating_add(1);
        return Ok(());
    }
    scope.element(base, "$children");
    let Some(at) = base.children.iter().position(|existing| {
        scope.element(existing, "$name");
        existing.name.eq_ignore_ascii_case(&child.name)
    }) else {
        report.unmatched_ids.push(id.to_owned());
        return Ok(());
    };
    let mut matched = 0_usize;
    let mut missed = Vec::new();
    if let Some(target) = base.children.get_mut(at) {
        let mut addressed = Vec::new();
        top_ids(child, &mut addressed, scope);
        for part in addressed {
            let Some(part_id) = id_of(part) else {
                continue;
            };
            if merge_by_id(target, part_id, part, budget, scope)? > 0 {
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
        replace_child(base, child, report, replaced, budget, scope)?;
    }
    Ok(())
}

/// Collects the descendants of `element` that have an `id`, not looking below one that has.
fn top_ids<'a>(element: &'a Element, found: &mut Vec<&'a Element>, scope: &mut PatchScope<'_>) {
    scope.element(element, "$children");
    for child in &element.children {
        scope.field(child, "id");
        if id_of(child).is_some() {
            found.push(child);
        } else {
            top_ids(child, found, scope);
        }
    }
}

fn replace_children(
    base: &mut Element,
    source: &Element,
    report: &mut ModificationReport,
    replaced: &mut HashSet<String>,
    budget: &mut Budget,
    scope: &mut PatchScope<'_>,
) -> Result<(), String> {
    scope.element(source, "$children");
    for child in &source.children {
        scope.element(child, "$name");
        report.vehicle_children = report.vehicle_children.saturating_add(1);
        replace_child(base, child, report, replaced, budget, scope)?;
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
    scope: &mut PatchScope<'_>,
) -> Result<(), String> {
    scope.element(child, "$name");
    if child.name.eq_ignore_ascii_case("Modifications") {
        return Ok(());
    }
    budget.charge_copy(child)?;
    scope.element(base, "$children");
    let mut added = child.clone();
    added.mark_all(ValueOrigin::Patch);
    if replaced.insert(child.name.to_ascii_lowercase()) {
        if let Some(at) = base.children.iter().position(|existing| {
            scope.element(existing, "$name");
            existing.name.eq_ignore_ascii_case(&child.name)
        }) {
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
            .rposition(|existing| {
                scope.element(existing, "$name");
                existing.name.eq_ignore_ascii_case(&child.name)
            })
            .map_or(base.children.len(), |last| last + 1);
        base.children.insert(after, added);
    }
    Ok(())
}

fn merge_by_id(
    element: &mut Element,
    id: &str,
    patch: &Element,
    budget: &mut Budget,
    scope: &mut PatchScope<'_>,
) -> Result<usize, String> {
    scope.element(element, "$children");
    let mut matched = 0_usize;
    for child in &mut element.children {
        scope.field(child, "id");
        if id_of(child) == Some(id) {
            matched = matched.saturating_add(1);
            scope.element(patch, "$attributes");
            let target = child.origins.evidence("id").cloned();
            let selectors = [patch.origins.evidence("id"), target.as_ref()];
            for (ordinal, (name, value)) in patch.attributes.iter().enumerate() {
                budget.charge_set(child.attributes.len(), name, value)?;
                child.set_attribute(
                    name,
                    value,
                    ValueOrigin::Patch,
                    patch
                        .origins
                        .donor_evidence(name, ordinal)
                        .map(|assignment| {
                            VehicleAttributeEvidence::with_targets(assignment, &selectors)
                        }),
                );
                if name.eq_ignore_ascii_case("name") {
                    child.port_name.clone_from(&patch.port_name);
                }
                if name.eq_ignore_ascii_case("bone") {
                    child.light_bone.clone_from(&patch.light_bone);
                }
            }
            scope.element(child, "$children");
            scope.element(patch, "$children");
            let old = std::mem::take(&mut child.children);
            child.children = overlay::children(old, &patch.children, budget, scope)?;
        } else {
            scope.element(child, "$name");
            if !child.name.eq_ignore_ascii_case("Modifications") {
                matched = matched.saturating_add(merge_by_id(child, id, patch, budget, scope)?);
            }
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
    aggregate: Option<&mut VehicleAggregateEvidence>,
) -> Result<Vec<String>, String> {
    if elems.is_empty() {
        return Ok(Vec::new());
    }
    let mut by_id: HashMap<&str, Vec<&ElemOverride>> = HashMap::new();
    for elem in elems {
        by_id.entry(elem.id_ref.as_str()).or_default().push(elem);
    }
    let mut seen = HashSet::new();
    let mut scope = PatchScope::new(aggregate);
    set_elems(base, &by_id, &mut seen, budget, &mut scope)?;
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
    scope: &mut PatchScope<'_>,
) -> Result<(), String> {
    scope.field(element, "id");
    if let Some(id) = id_of(element)
        && let Some((key, elems)) = by_id.get_key_value(id)
    {
        seen.insert(*key);
        let target = element.origins.evidence("id").cloned();
        for elem in elems {
            budget.charge_set(element.attributes.len(), &elem.name, &elem.value)?;
            let evidence = elem.evidence.as_ref().map(|assignment| {
                VehicleAttributeEvidence::with_targets(assignment, &[target.as_ref()])
            });
            element.set_attribute(&elem.name, &elem.value, ValueOrigin::Elem, evidence);
            if elem.name.eq_ignore_ascii_case("name") {
                element.port_name.clone_from(&elem.port_name);
            }
            if elem.name.eq_ignore_ascii_case("bone") {
                element.light_bone.clone_from(&elem.light_bone);
            }
        }
    }
    scope.element(element, "$children");
    for child in &mut element.children {
        scope.element(child, "$name");
        if !child.name.eq_ignore_ascii_case("Modifications") {
            set_elems(child, by_id, seen, budget, scope)?;
        }
    }
    Ok(())
}
