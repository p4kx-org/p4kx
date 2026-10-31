// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::placement::is_character_item;
use crate::resolver::LoadoutResolver;
use crate::tree::GeometryChoice;
use crate::{Attachment, BodyVariant, EntityNode, MaterialVariant, PortNode};

const HEAD_TYPE: &str = "Char_Head";

/// The body a character wears and the head Entity whose sub-type names it.
#[derive(Debug, Clone, Copy)]
pub struct CharacterBody<'n> {
    variant: BodyVariant,
    head: &'n EntityNode,
}

impl<'n> CharacterBody<'n> {
    /// Returns the body variant the head's sub-type names.
    #[must_use]
    pub const fn variant(&self) -> BodyVariant {
        self.variant
    }

    /// Returns the head Entity that names the body.
    #[must_use]
    pub const fn head(&self) -> &'n EntityNode {
        self.head
    }
}

impl EntityNode {
    /// Returns the body of the character in the tree below this Entity: the one the sub-type of
    /// the first `Char_Head` Entity names; `None` for a tree without a head or with a head whose
    /// sub-type names no body.
    #[must_use]
    pub fn character_body(&self) -> Option<CharacterBody<'_>> {
        let head = find_head(self.ports())?;
        Some(CharacterBody {
            variant: variant_of(head)?,
            head,
        })
    }
}

impl LoadoutResolver<'_> {
    /// Gives the worn Entities of a character the sub-geometry of the character's body.
    ///
    /// The body is the one the sub-type of the `Char_Head` Entity in the tree names. An Entity
    /// whose item type starts with `Char_` (armour, clothing, head parts), whose Record names
    /// geometry per body (`Male`, `Female`) and that no port flag gave a sub-geometry takes the
    /// one tagged for the body; one without a sub-geometry for the body has no geometry. A tree
    /// without a head is left as it is.
    pub(crate) fn dress_body(&mut self, root: &mut EntityNode) {
        let Some(body) = root.character_body().map(|found| found.variant) else {
            return;
        };
        self.dress_ports(&mut root.ports, body);
    }

    fn dress_ports(&mut self, ports: &mut [PortNode], body: BodyVariant) {
        for port in ports {
            if let Attachment::Entity(child) = &mut port.attachment {
                self.dress_entity(child, body);
                self.dress_ports(&mut child.ports, body);
            }
            self.dress_ports(&mut port.nested, body);
        }
    }

    fn dress_entity(&mut self, child: &mut EntityNode, body: BodyVariant) {
        if child.choice != GeometryChoice::Main || !is_character_item(child.attach_type()) {
            return;
        }
        let spec = self.spec(child.guid);
        let Ok(spec) = spec.as_ref() else {
            return;
        };
        let per_body = spec.variants.iter().any(|variant| {
            BodyVariant::ALL
                .iter()
                .any(|other| variant.has_tag(other.tag()))
        });
        if !per_body {
            return;
        }
        child.choice = GeometryChoice::Body(body);
        child.fallback = None;
        child.geometry = spec
            .variants
            .iter()
            .find(|variant| variant.has_tag(body.tag()))
            .map(|variant| {
                let mut worn = variant.geometry().clone();
                worn.material = worn.material.or_else(|| {
                    MaterialVariant::resolve(&spec.material_variants, &[body.tag().to_owned()])
                        .map(str::to_owned)
                });
                worn
            });
    }
}

/// Returns whether `item_type` is the item type of a head, `Char_Head`.
pub(crate) const fn is_head_type(item_type: &str) -> bool {
    item_type.eq_ignore_ascii_case(HEAD_TYPE)
}

/// Returns the first Entity of the item type `Char_Head` below `ports`, depth first.
pub(crate) fn find_head(ports: &[PortNode]) -> Option<&EntityNode> {
    ports.iter().find_map(|port| {
        if let Attachment::Entity(node) = port.attachment() {
            if is_head_type(node.attach_type()) {
                return Some(&**node);
            }
            if let Some(found) = find_head(node.ports()) {
                return Some(found);
            }
        }
        find_head(port.nested())
    })
}

/// Returns the body the sub-type of the head Entity `head` names.
pub(crate) fn variant_of(head: &EntityNode) -> Option<BodyVariant> {
    BodyVariant::from_code(&head.attach_sub_type)
}
