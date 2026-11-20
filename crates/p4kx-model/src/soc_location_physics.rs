// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_cryxml::{SourceAttribute, SourceElement};

use crate::soc_entities;
use crate::soc_source_node::{SocSourceNode, SocSourceValue};

/// Location physics declarations of one authored `ObjectContainer` descriptor.
#[derive(Debug, Clone, Copy)]
pub struct SocLocationPhysics<'document> {
    root: SocSourceNode<'document>,
}

impl<'document> SocLocationPhysics<'document> {
    pub(crate) const fn new(root: SourceElement<'document>) -> Self {
        Self {
            root: SocSourceNode::new(0, root),
        }
    }

    /// Reads original location physics from one authenticated `ObjectContainer` root.
    #[must_use]
    pub fn from_root(root: SourceElement<'document>) -> Option<Self> {
        (root.namespace().is_none() && root.name().eq_ignore_ascii_case("ObjectContainer"))
            .then(|| Self::new(root))
    }

    /// Reads the descriptor-wide grid radius, including absence and invalid values.
    #[must_use]
    pub fn grid_radius(self) -> SocPhysicsField<'document, f64> {
        SocPhysicsField::read(self.root, "gridRadius", real)
    }

    /// Visits original root-level grid and part declarations in source order.
    pub fn declarations(self) -> impl Iterator<Item = SocPhysicsDeclaration<'document>> {
        declarations(self.root.element(), Placement::Root)
    }
}

/// An original location-physics declaration and its physical position.
#[derive(Debug, Clone, Copy)]
pub struct SocPhysicsDeclaration<'document> {
    owner: SourceElement<'document>,
    node: SocSourceNode<'document>,
    kind: SocPhysicsKind,
}

/// The exact declaration family within an `ObjectContainer` descriptor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum SocPhysicsKind {
    /// A `PhysicsGrid` declaration.
    Grid,
    /// A `PhysicsGridPart` declaration.
    Part,
    /// A `PhysicsGridVoxel` declaration.
    Voxel,
    /// A `PhysicsGridChildParts` group, including authored empty groups.
    ChildParts,
}

impl<'document> SocPhysicsDeclaration<'document> {
    /// Returns the original sibling ordinal, including intervening unrelated elements.
    #[must_use]
    pub const fn ordinal(self) -> usize {
        self.node.ordinal()
    }

    /// Returns the exact typed declaration family.
    #[must_use]
    pub const fn kind(self) -> SocPhysicsKind {
        self.kind
    }

    /// Returns the original declaration element and its unknown content.
    #[must_use]
    pub const fn element(self) -> SourceElement<'document> {
        self.node.element()
    }

    /// Returns the original parent element.
    #[must_use]
    pub const fn owner(self) -> SourceElement<'document> {
        self.owner
    }

    /// Visits the immediate typed child declarations in original sibling order.
    pub fn children(self) -> impl Iterator<Item = Self> {
        declarations(self.element(), Placement::Child(self.kind))
    }

    /// Returns a reader for the authored grid settings.
    #[must_use]
    pub fn grid(self) -> Option<SocPhysicsGrid<'document>> {
        matches!(self.kind, SocPhysicsKind::Grid).then_some(SocPhysicsGrid { node: self.node })
    }

    /// Reads a part's authored shape label.
    #[must_use]
    pub fn shape(self) -> Option<SocPhysicsField<'document, &'document str>> {
        matches!(self.kind, SocPhysicsKind::Part)
            .then(|| SocPhysicsField::read(self.node, "shape", nonempty))
    }

    /// Reads a voxel's authored file selector, including an explicit empty string.
    #[must_use]
    pub fn filename(self) -> Option<SocPhysicsField<'document, &'document str>> {
        matches!(self.kind, SocPhysicsKind::Voxel)
            .then(|| SocPhysicsField::read(self.node, "filename", Some))
    }
}

/// Original settings of one location physics grid.
#[derive(Debug, Clone, Copy)]
pub struct SocPhysicsGrid<'document> {
    node: SocSourceNode<'document>,
}

impl<'document> SocPhysicsGrid<'document> {
    /// Reads the authored gravity inheritance switch.
    #[must_use]
    pub fn inherit_gravity(self) -> SocPhysicsField<'document, bool> {
        SocPhysicsField::read(self.node, "inheritGravity", boolean)
    }

    /// Reads the three authored gravity components in source order.
    #[must_use]
    pub fn gravity(self) -> SocPhysicsField<'document, [f64; 3]> {
        SocPhysicsField::read(self.node, "gravity", |value| {
            soc_entities::numbers(Some(value))
        })
    }

    /// Reads the authored grid-type label.
    #[must_use]
    pub fn grid_type(self) -> SocPhysicsField<'document, &'document str> {
        SocPhysicsField::read(self.node, "gridType", nonempty)
    }

    /// Reads the authored cell size.
    #[must_use]
    pub fn cell_size(self) -> SocPhysicsField<'document, f64> {
        SocPhysicsField::read(self.node, "cellSize", real)
    }

    /// Reads the authored part-presence switch.
    #[must_use]
    pub fn has_part(self) -> SocPhysicsField<'document, bool> {
        SocPhysicsField::read(self.node, "hasPart", boolean)
    }

    /// Reads the authored numeric portal mode.
    #[must_use]
    pub fn portal_exclusive_mode(self) -> SocPhysicsField<'document, i64> {
        SocPhysicsField::read(self.node, "portalExclusiveMode", |value| {
            value.trim().parse().ok()
        })
    }
}

/// One typed field beside its original element and all matching attributes.
#[derive(Debug, Clone, Copy)]
pub struct SocPhysicsField<'document, T> {
    node: SocSourceNode<'document>,
    name: &'static str,
    value: SocSourceValue<T>,
}

impl<'document, T> SocPhysicsField<'document, T> {
    fn read(
        node: SocSourceNode<'document>,
        name: &'static str,
        parse: impl FnOnce(&'document str) -> Option<T>,
    ) -> Self {
        Self {
            node,
            name,
            value: node.read(name, parse),
        }
    }

    /// Returns the exact field label.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        self.name
    }

    /// Returns the declaring element, including for absent values.
    #[must_use]
    pub const fn element(&self) -> SourceElement<'document> {
        self.node.element()
    }

    /// Visits matching original attributes in source order.
    pub fn attributes(&self) -> impl Iterator<Item = SourceAttribute<'document>> {
        self.node.attributes(self.name)
    }

    /// Returns the typed field state.
    #[must_use]
    pub fn into_value(self) -> SocSourceValue<T> {
        self.value
    }
}

#[derive(Clone, Copy)]
enum Placement {
    Root,
    Child(SocPhysicsKind),
}

fn declarations(
    owner: SourceElement<'_>,
    placement: Placement,
) -> impl Iterator<Item = SocPhysicsDeclaration<'_>> {
    SocSourceNode::direct_children(owner).filter_map(move |node| {
        let element = node.element();
        if element.namespace().is_some() {
            return None;
        }
        let kind = match (placement, element.name()) {
            (Placement::Root, "PhysicsGrid") => SocPhysicsKind::Grid,
            (
                Placement::Root
                | Placement::Child(
                    SocPhysicsKind::Grid | SocPhysicsKind::ChildParts | SocPhysicsKind::Part,
                ),
                "PhysicsGridPart",
            ) => SocPhysicsKind::Part,
            (Placement::Child(SocPhysicsKind::Part), "PhysicsGridVoxel") => SocPhysicsKind::Voxel,
            (
                Placement::Child(SocPhysicsKind::Grid | SocPhysicsKind::Part),
                "PhysicsGridChildParts",
            ) => SocPhysicsKind::ChildParts,
            _ => return None,
        };
        Some(SocPhysicsDeclaration { owner, node, kind })
    })
}

fn real(value: &str) -> Option<f64> {
    value
        .trim()
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite())
}

fn boolean(value: &str) -> Option<bool> {
    let value = value.trim();
    if value == "1" || value.eq_ignore_ascii_case("true") {
        Some(true)
    } else if value == "0" || value.eq_ignore_ascii_case("false") {
        Some(false)
    } else {
        None
    }
}

fn nonempty(value: &str) -> Option<&str> {
    (!value.trim().is_empty()).then_some(value)
}
