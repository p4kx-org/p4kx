// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{HashMap, VecDeque};

use super::scope::PatchScope;
use crate::ValueOrigin;
use crate::markup::{Budget, Element};

/// Returns the children `new` as the replacement of the children `old`: each is paired with the
/// first unused old child of the same element name and `name` attribute, and takes over the
/// attributes of its partner that it does not state itself, such as an `id` a Modification
/// addresses; old children without a partner are dropped.
pub(super) fn children(
    old: Vec<Element>,
    new: &[Element],
    budget: &mut Budget,
    scope: &mut PatchScope<'_>,
) -> Result<Vec<Element>, String> {
    let mut partners: HashMap<(String, String), VecDeque<Element>> = HashMap::new();
    for element in old {
        partners
            .entry(pair_key(&element, scope))
            .or_default()
            .push_back(element);
    }
    let mut result = Vec::with_capacity(new.len());
    for source in new {
        budget.charge_shallow(source)?;
        let partner = partners
            .get_mut(&pair_key(source, scope))
            .and_then(VecDeque::pop_front);
        let mut element = Element {
            name: source.name.clone(),
            port_name: source.port_name.clone(),
            light_bone: source.light_bone.clone(),
            attributes: source.attributes.clone(),
            children: Vec::new(),
            origins: source.origins.clone(),
        };
        for (name, _) in &source.attributes {
            element.mark(name, ValueOrigin::Patch);
        }
        let old_children = match partner {
            Some(partner) => {
                for (name, value) in &partner.attributes {
                    scope.field(&element, name);
                    if element.attribute(name).is_none() {
                        budget.charge_set(element.attributes.len(), name, value)?;
                        element.set_attribute(
                            name,
                            value,
                            partner.origin_of(name),
                            partner.origins.evidence(name).cloned(),
                        );
                        if name.eq_ignore_ascii_case("name") {
                            element.port_name.clone_from(&partner.port_name);
                        }
                        if name.eq_ignore_ascii_case("bone") {
                            element.light_bone.clone_from(&partner.light_bone);
                        }
                    }
                }
                partner.children
            }
            None => Vec::new(),
        };
        element.children = children(old_children, &source.children, budget, scope)?;
        result.push(element);
    }
    Ok(result)
}

fn pair_key(element: &Element, scope: &mut PatchScope<'_>) -> (String, String) {
    scope.element(element, "$name");
    scope.field(element, "name");
    (
        element.name.to_ascii_lowercase(),
        element
            .attribute("name")
            .unwrap_or_default()
            .to_ascii_lowercase(),
    )
}
