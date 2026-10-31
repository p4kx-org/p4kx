// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::character_path::is_world_prop_path;
use crate::placement::is_character_item;
use crate::resolver::LoadoutResolver;
use crate::tree::GeometryChoice;
use crate::{Attachment, BodyVariant, EntityNode, MaterialVariant, PortNode};

/// The item type of the head of a character.
const HEAD_TYPE: &str = "Char_Head";

/// The item type of the body of a character.
pub(crate) const BODY_TYPE: &str = "Char_Body";

/// The body a character wears, the head Entity of the character and the Entity that names the
/// body: the head, or for a generic head the `Char_Body` Entity.
#[derive(Debug, Clone, Copy)]
pub struct CharacterBody<'n> {
    variant: BodyVariant,
    head: &'n EntityNode,
    named_by: &'n EntityNode,
    by_head: bool,
}

/// The outcome of `EntityNode::naming`: the body, the head Entity if the tree has one and the
/// Entity that names the body.
struct Naming<'n> {
    variant: BodyVariant,
    head: Option<&'n EntityNode>,
    named_by: &'n EntityNode,
    by_head: bool,
}

impl<'n> CharacterBody<'n> {
    /// Returns the body variant.
    #[must_use]
    pub const fn variant(&self) -> BodyVariant {
        self.variant
    }

    /// Returns the head Entity of the character.
    #[must_use]
    pub const fn head(&self) -> &'n EntityNode {
        self.head
    }

    /// Returns the Entity that names the body: the head, else the `Char_Body` Entity.
    #[must_use]
    pub const fn named_by(&self) -> &'n EntityNode {
        self.named_by
    }

    /// Returns whether the head names the body, not the `Char_Body` Entity.
    #[must_use]
    pub const fn is_named_by_head(&self) -> bool {
        self.by_head
    }
}

impl EntityNode {
    /// Returns the body of the character in the tree below this Entity, named as
    /// `EntityNode::naming` says, and its first `Char_Head` Entity; `None` for a tree without a
    /// head or whose head and `Char_Body` Entity name no body.
    #[must_use]
    pub fn character_body(&self) -> Option<CharacterBody<'_>> {
        let naming = self.naming()?;
        Some(CharacterBody {
            variant: naming.variant,
            head: naming.head?,
            named_by: naming.named_by,
            by_head: naming.by_head,
        })
    }

    /// Returns the body the character in the tree below this Entity wears, see
    /// `EntityNode::naming`.
    pub(crate) fn body_variant(&self) -> Option<BodyVariant> {
        self.naming().map(|naming| naming.variant)
    }

    /// Decides the body of the character, the one place that does: the first source that names
    /// one wins, the sub-type of its `Char_Head` Entity (the generic head of most NPCs names
    /// none), the sub-type of its `Char_Body` Entity, the folder of the model of that body.
    fn naming(&self) -> Option<Naming<'_>> {
        let head = find_head(self.ports());
        if let Some(found) = head
            && let Some(variant) = variant_of(found)
        {
            return Some(Naming {
                variant,
                head,
                named_by: found,
                by_head: true,
            });
        }
        let body = find_item(self.ports(), is_body_type)?;
        let variant = BodyVariant::from_code(&body.attach_sub_type)
            .or_else(|| BodyVariant::of_model_path(body.geometry()?.path()))?;
        Some(Naming {
            variant,
            head,
            named_by: body,
            by_head: false,
        })
    }
}

impl LoadoutResolver<'_> {
    /// Gives the worn Entities of a character the sub-geometry of the character's body.
    ///
    /// The body is [`EntityNode::body_variant`]. An Entity whose item type starts with `Char_`
    /// (armour, clothing, head parts), whose Record names geometry per body (`Male`, `Female`)
    /// and that no port flag gave a sub-geometry takes the one tagged for the body; one without a
    /// sub-geometry for the body has no geometry, and neither has one whose only geometry is a
    /// world prop. A tree without a body is left as it is.
    pub(crate) fn dress_body(&mut self, root: &mut EntityNode) {
        let Some(body) = root.body_variant() else {
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
            leave_out_world_prop(child);
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

/// Takes the geometry of a worn item without sub-geometry per body when it is a world prop and
/// keeps it in `fallback`, so that the Assembly can say which prop it left out.
fn leave_out_world_prop(child: &mut EntityNode) {
    if child
        .geometry
        .as_ref()
        .is_some_and(|main| is_world_prop_path(main.path()))
    {
        child.choice = GeometryChoice::WorldProp;
        child.fallback = child.geometry.take();
    }
}

/// Returns whether `item_type` is the item type of a head, `Char_Head`.
pub(crate) const fn is_head_type(item_type: &str) -> bool {
    item_type.eq_ignore_ascii_case(HEAD_TYPE)
}

/// Returns the first Entity below `ports`, depth first, whose item type `is_type` accepts.
fn find_item(ports: &[PortNode], is_type: fn(&str) -> bool) -> Option<&EntityNode> {
    ports.iter().find_map(|port| {
        if let Attachment::Entity(node) = port.attachment() {
            if is_type(node.attach_type()) {
                return Some(&**node);
            }
            if let Some(found) = find_item(node.ports(), is_type) {
                return Some(found);
            }
        }
        find_item(port.nested(), is_type)
    })
}

/// Returns the first Entity of the item type `Char_Head` below `ports`, depth first.
pub(crate) fn find_head(ports: &[PortNode]) -> Option<&EntityNode> {
    find_item(ports, is_head_type)
}

/// Returns whether `item_type` is the item type of a body, `Char_Body`.
pub(crate) const fn is_body_type(item_type: &str) -> bool {
    item_type.eq_ignore_ascii_case(BODY_TYPE)
}

/// Returns the body the sub-type of the head Entity `head` names.
pub(crate) fn variant_of(head: &EntityNode) -> Option<BodyVariant> {
    BodyVariant::from_code(&head.attach_sub_type)
}
