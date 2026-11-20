// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::slice;

use p4kx_dataforge::StructValue;
use p4kx_extraction::SourceLocation;

use crate::geometry::{GeometryComponent, PaletteRef, PaletteReferenceSource};
use crate::read;

/// An authored material choice and its ordered child choices.
pub struct MaterialVariant {
    pub(crate) tags: Vec<String>,
    pub(crate) material: Option<String>,
    pub(crate) palette: Option<PaletteRef>,
    pub(crate) owner: GeometryComponent,
    pub(crate) slot: usize,
    pub(crate) children: Vec<Self>,
}

/// A matching material variant and its original position in the Entity Record.
#[derive(Debug)]
pub struct MaterialVariantSelection<'variant> {
    variant: &'variant MaterialVariant,
    ancestors: Vec<usize>,
}

impl<'variant> MaterialVariantSelection<'variant> {
    /// Returns the material path authored on the selected variant.
    #[must_use]
    pub fn material(&self) -> Option<&'variant str> {
        self.variant.material()
    }

    /// Returns the field address of the authored material path.
    #[must_use]
    pub fn material_location(&self) -> Option<SourceLocation> {
        self.material()?;
        Some(self.field_location(".Material.path"))
    }

    /// Returns the selected variant's palette reference bound to its original field.
    #[must_use]
    pub fn palette(&self) -> Option<PaletteRef> {
        let mut palette = self.variant.palette.clone()?;
        palette.selector =
            PaletteReferenceSource::Named(Some(self.field_location(".Palette.RootRecord")));
        Some(palette)
    }

    /// Returns the original field address of the selected variant.
    #[must_use]
    pub fn location(&self) -> SourceLocation {
        self.field_location("")
    }

    fn field_location(&self, member: &str) -> SourceLocation {
        let owner = self.variant.owner;
        let mut path = format!("Components[{}].Material", owner.ordinal);
        for slot in self.ancestors.iter().copied().chain([self.variant.slot]) {
            path.push_str(".materialVariants[");
            path.push_str(&slot.to_string());
            path.push(']');
        }
        path.push_str(member);
        SourceLocation::DataForge {
            record_index: owner.record,
            field: path,
        }
    }
}

struct Branch<'variant> {
    node: &'variant MaterialVariant,
    siblings: slice::Iter<'variant, MaterialVariant>,
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
        Self::find(
            variants,
            active,
            |node| node.material.is_some(),
            |node, _| node.material(),
        )
        .flatten()
    }

    /// Selects the first matching branch with an authored material or palette.
    ///
    /// Matching descendants take precedence over their parents. Both values come from the selected node.
    #[must_use]
    pub fn select<'variants, Tag: AsRef<str>>(
        variants: &'variants [Self],
        active: &[Tag],
    ) -> Option<MaterialVariantSelection<'variants>> {
        Self::find(
            variants,
            active,
            |node| node.material.is_some() || node.palette.is_some(),
            |variant, parents| MaterialVariantSelection {
                variant,
                ancestors: parents.iter().map(|branch| branch.node.slot).collect(),
            },
        )
    }

    fn find<'variants, Tag: AsRef<str>, Output>(
        variants: &'variants [Self],
        active: &[Tag],
        fits: impl Fn(&Self) -> bool,
        select: impl FnOnce(&'variants Self, &[Branch<'variants>]) -> Output,
    ) -> Option<Output> {
        let mut siblings = variants.iter();
        let mut parents = Vec::new();
        loop {
            if let Some(node) = siblings.next() {
                if !node.matches(active) {
                    continue;
                }
                parents.push(Branch { node, siblings });
                siblings = node.children.iter();
            } else if let Some(branch) = parents.pop() {
                if fits(branch.node) {
                    return Some(select(branch.node, &parents));
                }
                siblings = branch.siblings;
            } else {
                return None;
            }
        }
    }

    fn matches<Tag: AsRef<str>>(&self, active: &[Tag]) -> bool {
        !self.tags.is_empty()
            && self.tags.iter().all(|tag| {
                active
                    .iter()
                    .any(|own| own.as_ref().eq_ignore_ascii_case(tag))
            })
    }

    pub(crate) fn read(component: &StructValue<'_>, owner: GeometryComponent) -> Vec<Self> {
        let Some(root) = read::struct_field(component, "Material") else {
            return Vec::new();
        };
        let mut siblings = read::indexed_struct_items(root, "materialVariants");
        let mut found = Vec::new();
        let mut parents = Vec::new();
        loop {
            if let Some((slot, child)) = siblings.next() {
                let variant = Self::of_node(child, owner, slot);
                parents.push((siblings, found, variant));
                siblings = read::indexed_struct_items(child, "materialVariants");
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

    fn of_node(node: &StructValue<'_>, owner: GeometryComponent, slot: usize) -> Self {
        let material = read::struct_field(node, "Material")
            .map(|resource| read::text(resource, "path"))
            .filter(|path| !path.is_empty())
            .map(str::to_owned);
        Self {
            tags: read::text(node, "Tags")
                .split_whitespace()
                .map(str::to_owned)
                .collect(),
            material,
            palette: PaletteRef::read(node),
            owner,
            slot,
            children: Vec::new(),
        }
    }
}

#[cfg(test)]
#[path = "geometry_material_source_tests.rs"]
mod source_tests;

#[cfg(test)]
#[path = "material_variant_tests.rs"]
mod tests;
