// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::LoadoutPath;
use crate::loadout_xml::{Node, Scope};

/// A Modification of the base implementation: its selected patch and Elem overrides.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Modification {
    pub(super) patch: Option<ModificationPatch>,
    pub(super) elems: Vec<ElemOverride>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ModificationPatch {
    pub(super) path: LoadoutPath,
    pub(super) name: LoadoutPath,
}

/// Selects the original Modification, preferring an exact name over an ASCII case match.
pub(super) fn find_modification(
    vehicle: Node<'_, '_>,
    scope: &Scope<'_, '_>,
    requested: &str,
) -> Result<Option<Modification>, String> {
    let Some((node, authored_name)) = select_modification(vehicle, requested) else {
        return Ok(None);
    };
    let patch = match node.attribute_ascii("patchFile") {
        Some((name, value)) if !value.trim().is_empty() => Some(ModificationPatch {
            path: scope
                .path(node, name)?
                .ok_or("the selected patchFile attribute is absent")?,
            name: scope
                .modification_name(node, authored_name)?
                .ok_or("the selected Modification name is absent")?,
        }),
        _ => None,
    };
    let mut elems = Vec::new();
    for node in named_children(node, "Elems").flat_map(|node| named_children(node, "Elem")) {
        let Some((_, id_ref)) = node
            .attribute_ascii("idRef")
            .filter(|(_, id)| !id.is_empty())
        else {
            continue;
        };
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
        elems.push(ElemOverride {
            id_ref: id_ref.to_owned(),
            name: name.to_owned(),
            value: value.map_or("", |(_, value)| value).to_owned(),
            port_name,
            light_bone: value
                .filter(|_| name.eq_ignore_ascii_case("bone"))
                .and_then(|(attribute, _)| scope.attribute_ascii_position(node, attribute)),
        });
    }
    Ok(Some(Modification { patch, elems }))
}

fn select_modification<'document, 'input>(
    vehicle: Node<'document, 'input>,
    requested: &str,
) -> Option<(Node<'document, 'input>, &'document str)> {
    let mut fallback = None;
    for candidate in named_children(vehicle, "Modifications")
        .flat_map(|group| named_children(group, "Modification"))
    {
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
    fallback
}

fn named_children<'document, 'input>(
    node: Node<'document, 'input>,
    name: &str,
) -> impl Iterator<Item = Node<'document, 'input>> {
    node.children()
        .into_iter()
        .filter(move |child| child.name().eq_ignore_ascii_case(name))
}

/// One `Elem` of a Modification: the attribute `name` of every element with the `id` `id_ref`
/// is set to `value`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ElemOverride {
    pub(super) id_ref: String,
    pub(super) name: String,
    pub(super) value: String,
    pub(super) port_name: Option<LoadoutPath>,
    pub(super) light_bone: Option<p4kx_cryxml::SourceAttributePosition>,
}

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
