// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use crate::loadout_xml::{MissingAttribute, Node, Scope};
use crate::{LoadoutPath, VehicleAggregateEvidence, VehicleAttributeEvidence, VehicleXmlSite};

use super::{ITEM_TYPE_ATTRIBUTE, PORT_OBSERVED_ATTRIBUTES};

/// A Modification of the base implementation: its selected patch and Elem overrides.
#[derive(Debug)]
pub(super) struct Modification {
    pub(super) patch: Option<ModificationPatch>,
    pub(super) elems: Vec<ElemOverride>,
}

#[derive(Debug)]
pub(super) struct ModificationPatch {
    pub(super) path: LoadoutPath,
    pub(super) name: LoadoutPath,
    pub(super) selector: Option<VehicleXmlSite>,
}

/// Selects the original Modification, preferring an exact name over an ASCII case match.
pub(super) fn find_modification(
    vehicle: Node<'_, '_>,
    scope: &Scope<'_, '_>,
    requested: &str,
    mut aggregate: Option<&mut VehicleAggregateEvidence>,
) -> Result<Option<Modification>, String> {
    let Some((node, authored_name)) =
        select_modification(vehicle, scope, requested, &mut aggregate)
    else {
        return Ok(None);
    };
    let patch_site = scope.vehicle_site(node, "patchFile", MissingAttribute::Element);
    if let Some(aggregate) = &mut aggregate {
        aggregate.selection(patch_site.clone());
    }
    let patch = match node.attribute_ascii("patchFile") {
        Some((name, value)) if !value.trim().is_empty() => Some(ModificationPatch {
            path: scope
                .path(node, name)?
                .ok_or("the selected patchFile attribute is absent")?,
            name: scope
                .modification_name(node, authored_name)?
                .ok_or("the selected Modification name is absent")?,
            selector: patch_site.filter(|site| !site.checked_absence()),
        }),
        _ => None,
    };
    let modification = scope.vehicle_site(node, authored_name, MissingAttribute::Omit);
    let elems = read_elems(node, scope, modification.as_ref(), &mut aggregate)?;
    Ok(Some(Modification { patch, elems }))
}

fn read_elems(
    node: Node<'_, '_>,
    scope: &Scope<'_, '_>,
    modification: Option<&VehicleXmlSite>,
    aggregate: &mut Option<&mut VehicleAggregateEvidence>,
) -> Result<Vec<ElemOverride>, String> {
    let mut elems = Vec::new();
    observe_element(aggregate, scope, node, "$children");
    for group in node.children() {
        observe_element(aggregate, scope, group, "$name");
        if !group.name().eq_ignore_ascii_case("Elems") {
            continue;
        }
        observe_element(aggregate, scope, group, "$children");
        for node in group.children() {
            observe_element(aggregate, scope, node, "$name");
            if !node.name().eq_ignore_ascii_case("Elem") {
                continue;
            }
            observe_attribute(aggregate, scope, node, "idRef");
            let Some((_, id_ref)) = node
                .attribute_ascii("idRef")
                .filter(|(_, id)| !id.is_empty())
            else {
                continue;
            };
            observe_attribute(aggregate, scope, node, "name");
            let Some((_, name)) = node.attribute_ascii("name") else {
                continue;
            };
            let value = node.attribute_ascii("value");
            let port_name = match value {
                Some((attribute, _)) if name.eq_ignore_ascii_case("name") => {
                    scope.port_name(node, attribute)?
                }
                _ => None,
            };
            let evidence = assignment_evidence(node, scope, name, modification);
            elems.push(ElemOverride {
                id_ref: id_ref.to_owned(),
                name: name.to_owned(),
                value: value.map_or("", |(_, value)| value).to_owned(),
                port_name,
                structure_value: (aggregate.is_some()
                    && evidence.is_none()
                    && super::structure::is_compared(name, false))
                .then(|| scope.vehicle_site(node, "value", MissingAttribute::Element))
                .flatten(),
                evidence,
                light_bone: value
                    .filter(|_| name.eq_ignore_ascii_case("bone"))
                    .and_then(|(attribute, _)| scope.attribute_ascii_position(node, attribute)),
            });
        }
    }
    Ok(elems)
}

fn assignment_evidence(
    node: Node<'_, '_>,
    scope: &Scope<'_, '_>,
    name: &str,
    modification: Option<&VehicleXmlSite>,
) -> Option<Arc<VehicleAttributeEvidence>> {
    let visibility = ["flags", "id"]
        .into_iter()
        .any(|field| name.eq_ignore_ascii_case(field));
    let totals = scope.observes_vehicle_totals()
        && ["mass", "damageMax", "skipPart"]
            .into_iter()
            .any(|field| name.eq_ignore_ascii_case(field));
    let port = scope.observes_vehicle_port_attributes()
        && PORT_OBSERVED_ATTRIBUTES
            .into_iter()
            .chain(["name", "skipPart", ITEM_TYPE_ATTRIBUTE])
            .any(|field| name.eq_ignore_ascii_case(field));
    if !visibility && !totals && !port {
        return None;
    }
    let value = scope.vehicle_site(node, "value", MissingAttribute::Element)?;
    let selectors = ["name", "idRef"]
        .into_iter()
        .filter_map(|name| scope.vehicle_site(node, name, MissingAttribute::Omit))
        .chain(modification.cloned())
        .collect();
    Some(Arc::new(VehicleAttributeEvidence::new(value, selectors)))
}

fn select_modification<'document, 'input>(
    vehicle: Node<'document, 'input>,
    scope: &Scope<'_, '_>,
    requested: &str,
    aggregate: &mut Option<&mut VehicleAggregateEvidence>,
) -> Option<(Node<'document, 'input>, &'document str)> {
    let mut fallback = None;
    observe_element(aggregate, scope, vehicle, "$children");
    for group in vehicle.children() {
        observe_element(aggregate, scope, group, "$name");
        if !group.name().eq_ignore_ascii_case("Modifications") {
            continue;
        }
        observe_element(aggregate, scope, group, "$children");
        for candidate in group.children() {
            observe_element(aggregate, scope, candidate, "$name");
            if !candidate.name().eq_ignore_ascii_case("Modification") {
                continue;
            }
            observe_attribute(aggregate, scope, candidate, "name");
            let Some((name, value)) = candidate.attribute_ascii("name") else {
                continue;
            };
            if value == requested {
                return Some((candidate, name));
            }
            if fallback.is_none() && value.eq_ignore_ascii_case(requested) {
                fallback = Some((candidate, name));
            }
        }
    }
    fallback
}

fn observe_element(
    aggregate: &mut Option<&mut VehicleAggregateEvidence>,
    scope: &Scope<'_, '_>,
    node: Node<'_, '_>,
    field: &str,
) {
    if let Some(aggregate) = aggregate {
        aggregate.selection(
            scope
                .vehicle_element_site(node)
                .map(|site| site.with_field(field)),
        );
    }
}

fn observe_attribute(
    aggregate: &mut Option<&mut VehicleAggregateEvidence>,
    scope: &Scope<'_, '_>,
    node: Node<'_, '_>,
    name: &str,
) {
    if let Some(aggregate) = aggregate {
        aggregate.selection(scope.vehicle_site(node, name, MissingAttribute::Element));
    }
}

/// One `Elem` of a Modification: the attribute `name` of every element with the `id` `id_ref`
/// is set to `value`.
#[derive(Debug, Clone)]
pub struct ElemOverride {
    pub(super) id_ref: String,
    pub(super) name: String,
    pub(super) value: String,
    pub(super) port_name: Option<LoadoutPath>,
    pub(super) light_bone: Option<p4kx_cryxml::SourceAttributePosition>,
    pub(super) evidence: Option<Arc<VehicleAttributeEvidence>>,
    pub(super) structure_value: Option<VehicleXmlSite>,
}

impl PartialEq for ElemOverride {
    fn eq(&self, other: &Self) -> bool {
        self.id_ref == other.id_ref
            && self.name == other.name
            && self.value == other.value
            && self.port_name == other.port_name
            && self.light_bone == other.light_bone
    }
}

impl Eq for ElemOverride {}

impl ElemOverride {
    /// Returns the `id` of the elements the override addresses.
    #[must_use]
    pub fn id_ref(&self) -> &str {
        &self.id_ref
    }

    /// Returns the name of the attribute the override sets.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the value the override sets.
    #[must_use]
    pub fn value(&self) -> &str {
        &self.value
    }
}
