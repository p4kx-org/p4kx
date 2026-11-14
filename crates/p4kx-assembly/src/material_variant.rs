// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_dataforge::{StructValue, Value};

use crate::read;

/// An authored material choice and its ordered child choices.
pub struct MaterialVariant {
    pub(crate) tags: Vec<String>,
    pub(crate) material: Option<String>,
    pub(crate) children: Vec<Self>,
}

impl MaterialVariant {
    /// Borrows the source node's whitespace-separated tags.
    #[must_use]
    pub fn tags(&self) -> &[String] {
        &self.tags
    }

    /// Borrows the material path when the source names one.
    #[must_use]
    pub fn material(&self) -> Option<&str> {
        self.material.as_deref()
    }

    /// Borrows every authored child in source order.
    #[must_use]
    pub fn children(&self) -> &[Self] {
        &self.children
    }

    /// Resolves the first matching branch, preferring its deepest material over parent fallbacks.
    #[must_use]
    pub fn resolve<'variants>(
        variants: &'variants [Self],
        active: &[String],
    ) -> Option<&'variants str> {
        let mut siblings = variants.iter();
        let mut parents = Vec::new();
        loop {
            if let Some(node) = siblings.next() {
                if node.tags.is_empty()
                    || !node
                        .tags
                        .iter()
                        .all(|tag| active.iter().any(|own| own.eq_ignore_ascii_case(tag)))
                {
                    continue;
                }
                parents.push((siblings, node.material.as_deref()));
                siblings = node.children.iter();
            } else if let Some((remaining, fallback)) = parents.pop() {
                if fallback.is_some() {
                    return fallback;
                }
                siblings = remaining;
            } else {
                return None;
            }
        }
    }

    pub(crate) fn read(component: &StructValue<'_>) -> Vec<Self> {
        let Some(root) = read::struct_field(component, "Material") else {
            return Vec::new();
        };
        let mut siblings = child_values(root).iter();
        let mut found = Vec::new();
        let mut parents = Vec::new();
        loop {
            if let Some(child) = siblings.find_map(read::as_struct) {
                let material = read::struct_field(child, "Material")
                    .map(|resource| read::text(resource, "path"))
                    .filter(|path| !path.is_empty())
                    .map(str::to_owned);
                let variant = Self {
                    tags: read::text(child, "Tags")
                        .split_whitespace()
                        .map(str::to_owned)
                        .collect(),
                    material,
                    children: Vec::new(),
                };
                parents.push((siblings, found, variant));
                siblings = child_values(child).iter();
                found = Vec::new();
            } else if let Some((remaining, mut ancestors, mut parent)) = parents.pop() {
                parent.children = found;
                ancestors.push(parent);
                siblings = remaining;
                found = ancestors;
            } else {
                return found;
            }
        }
    }
}

fn child_values<'node, 'forge>(node: &'node StructValue<'forge>) -> &'node [Value<'forge>] {
    match node.field("materialVariants") {
        Some(Value::Array(children)) => children,
        _ => &[],
    }
}

#[cfg(test)]
#[path = "geometry_material_source_tests.rs"]
mod source_tests;

#[cfg(test)]
#[path = "material_variant_tests.rs"]
mod tests;
