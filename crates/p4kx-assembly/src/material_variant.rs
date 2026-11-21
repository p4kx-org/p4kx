// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::ControlFlow;
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

    fn from_branch(variant: &'variant MaterialVariant, ancestors: &[Branch<'variant>]) -> Self {
        Self {
            variant,
            ancestors: ancestors.iter().map(|branch| branch.node.slot).collect(),
        }
    }

    pub(crate) fn tag_locations(&self) -> Box<[SourceLocation]> {
        let mut path = self.branch_root();
        self.slots()
            .map(|slot| {
                append_variant(&mut path, slot);
                self.at(format!("{path}.Tags"))
            })
            .collect()
    }

    fn field_location(&self, member: &str) -> SourceLocation {
        let mut path = self.branch_root();
        for slot in self.slots() {
            append_variant(&mut path, slot);
        }
        path.push_str(member);
        self.at(path)
    }

    fn slots(&self) -> impl Iterator<Item = usize> + '_ {
        self.ancestors.iter().copied().chain([self.variant.slot])
    }

    fn branch_root(&self) -> String {
        format!("Components[{}].Material", self.variant.owner.ordinal)
    }

    const fn at(&self, field: String) -> SourceLocation {
        SourceLocation::DataForge {
            record_index: self.variant.owner.record,
            field,
        }
    }
}

fn append_variant(path: &mut String, slot: usize) {
    path.push_str(".materialVariants[");
    path.push_str(&slot.to_string());
    path.push(']');
}

struct Branch<'variant> {
    node: &'variant MaterialVariant,
    siblings: slice::Iter<'variant, MaterialVariant>,
    selected_descendant: bool,
}

/// Several authored branches can supply the appearance.
pub(crate) struct AmbiguousMaterialVariant;

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
            |node, _| ControlFlow::Break(node.material()),
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
            Self::has_appearance,
            |variant, parents| {
                ControlFlow::Break(MaterialVariantSelection::from_branch(variant, parents))
            },
        )
    }

    /// Selects one authored appearance, keeping descendants ahead of their ancestor.
    pub(crate) fn select_unique<'variants>(
        variants: &'variants [Self],
        active: &[&str],
    ) -> Result<Option<MaterialVariantSelection<'variants>>, AmbiguousMaterialVariant> {
        let mut selected = None;
        let ambiguous = Self::find(
            variants,
            active,
            Self::has_appearance,
            |variant, parents| {
                if selected.is_some() {
                    return ControlFlow::Break(AmbiguousMaterialVariant);
                }
                selected = Some(MaterialVariantSelection::from_branch(variant, parents));
                ControlFlow::Continue(())
            },
        );
        ambiguous.map_or(Ok(selected), Err)
    }

    const fn has_appearance(&self) -> bool {
        self.material.is_some() || self.palette.is_some()
    }

    fn find<'variants, Tag: AsRef<str>, Output>(
        variants: &'variants [Self],
        active: &[Tag],
        fits: impl Fn(&Self) -> bool,
        mut select: impl FnMut(&'variants Self, &[Branch<'variants>]) -> ControlFlow<Output>,
    ) -> Option<Output> {
        let mut siblings = variants.iter();
        let mut parents = Vec::new();
        loop {
            if let Some(node) = siblings.next() {
                if !node.matches(active) {
                    continue;
                }
                parents.push(Branch {
                    node,
                    siblings,
                    selected_descendant: false,
                });
                siblings = node.children.iter();
            } else {
                let branch = parents.pop()?;
                let selected = !branch.selected_descendant && fits(branch.node);
                if selected && let ControlFlow::Break(output) = select(branch.node, &parents) {
                    return Some(output);
                }
                if (selected || branch.selected_descendant)
                    && let Some(parent) = parents.last_mut()
                {
                    parent.selected_descendant = true;
                }
                siblings = branch.siblings;
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
