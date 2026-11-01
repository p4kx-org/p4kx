// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::{BodyVariant, Expressions, Piece, Wearable};
use crate::bespoke::Bespoke;
use crate::node_geometry::port_words;
use crate::resolver::LoadoutResolver;
use crate::{Attachment, EntityNode, PortNode};

/// The head of a character as its Loadout gives it: the head Entity, when it names a model, and
/// what sits on its ports, as pieces to wear on the body.
///
/// A story character's head and the generic head of other NPCs both qualify.
///
/// A head whose material library lives in a folder of its own has a bespoke skin there, see
/// [`Piece::bespoke_in`]; [`NpcHead::folder`] names that folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NpcHead {
    class_name: String,
    variant: BodyVariant,
    folder: Option<String>,
    pieces: Vec<Piece>,
}

impl NpcHead {
    /// Returns the class name of the head Entity, e.g. `eckhart_t2_head`.
    #[must_use]
    pub fn class_name(&self) -> &str {
        &self.class_name
    }

    /// Returns the body variant of the character, `Male` or `Female`: the one its head names,
    /// else the one its `Char_Body` Entity names.
    #[must_use]
    pub const fn variant(&self) -> BodyVariant {
        self.variant
    }

    /// Returns the folder below `Data` that holds the character's own head skins, the folder of
    /// the head's material library, when the head has one of its own.
    #[must_use]
    pub fn folder(&self) -> Option<&str> {
        self.folder.as_deref()
    }

    /// Returns the pieces to wear: the head, then the Entities on its ports that have geometry.
    #[must_use]
    pub fn pieces(&self) -> &[Piece] {
        &self.pieces
    }

    /// Returns the head with the expression blendshapes `expressions` asks for written as morph
    /// targets of the piece named `head`, see [`Piece::with_expressions`]. A head without a
    /// `.RigLogic` file of its own has none.
    #[must_use]
    pub fn with_expressions(mut self, expressions: &Expressions) -> Self {
        self.pieces = self
            .pieces
            .into_iter()
            .map(|piece| {
                if piece.name() == "head" {
                    piece.with_expressions(expressions.clone())
                } else {
                    piece
                }
            })
            .collect();
        self
    }
}

/// Returns whether the Entity names a model, main or per body.
fn has_geometry(wearable: &Wearable) -> bool {
    wearable.geometry().is_some() || !wearable.variants().is_empty()
}

/// Returns the flags of `port` followed by the `shared` flags of its container.
fn flags_of(port: &PortNode, shared: &[String]) -> Vec<String> {
    let own = port.definition().map(crate::PortDefinition::flags);
    port_words(own.unwrap_or_default(), shared)
        .map(str::to_owned)
        .collect()
}

impl LoadoutResolver<'_> {
    /// Returns the head a resolved Loadout tree gives its character: the Entity of the item type
    /// `Char_Head` wherever it hangs in the tree, and the Entities on its ports that have geometry.
    ///
    /// The sub-geometry an item takes is the one tagged with a flag of the head's port it sits
    /// at; the material library of an item is chosen by the tags of the Entities on its own ports,
    /// e.g. a hair colour. `None` when the tree has no head or neither the head nor the
    /// `Char_Body` Entity names a body.
    pub fn npc_head(&mut self, tree: &EntityNode) -> Option<NpcHead> {
        let body = tree.character_body()?;
        let (head, variant) = (body.head(), body.variant());
        let spec = self.spec(head.guid);
        let spec = spec.as_ref().as_ref().ok()?;
        let wearable = Wearable::from_spec(spec);
        let library = wearable
            .geometry()
            .and_then(|geometry| geometry.material())
            .map(str::to_owned);
        let class_name = wearable.class_name().to_owned();
        let own = Bespoke::of_head(&class_name, library.as_deref());
        let mut pieces = Vec::new();
        if has_geometry(&wearable) {
            pieces.push(bespoke(
                Piece::wearable(wearable).named("head"),
                own.as_ref(),
            ));
        }
        for port in head.ports() {
            if let Some(piece) = self.item_piece(port, &spec.port_flags) {
                pieces.push(bespoke(piece, own.as_ref()));
            }
        }
        Some(NpcHead {
            class_name,
            variant,
            folder: own.map(|own| own.folder),
            pieces,
        })
    }

    /// Returns the piece for the Entity on a port of the head, `None` for an empty port or an
    /// Entity without geometry. The flags the head's port container gives all its ports, `shared`,
    /// select the sub-geometry beside the flags of the port.
    fn item_piece(&mut self, port: &PortNode, shared: &[String]) -> Option<Piece> {
        let Attachment::Entity(item) = port.attachment() else {
            return None;
        };
        let spec = self.spec(item.guid);
        let spec = match spec.as_ref() {
            Ok(spec) => spec,
            Err(error) => {
                return Some(Piece::failed(port.name(), error.to_string()).at_port(port.name()));
            }
        };
        let wearable = Wearable::from_spec(spec);
        if !has_geometry(&wearable) {
            return None;
        }
        let tags = item
            .ports()
            .iter()
            .filter_map(|nested| match nested.attachment() {
                Attachment::Entity(variant) => Some(variant.guid),
                _ => None,
            })
            .filter_map(|guid| {
                self.spec(guid)
                    .as_ref()
                    .as_ref()
                    .ok()
                    .map(|spec| spec.attach_tags.clone())
            })
            .flatten()
            .collect();
        Some(
            Piece::wearable(wearable)
                .named(port.name())
                .at_port(port.name())
                .selected_by(flags_of(port, shared))
                .with_material_tags(tags),
        )
    }
}

fn bespoke(piece: Piece, own: Option<&Bespoke>) -> Piece {
    let Some(own) = own else {
        return piece;
    };
    piece.bespoke_in(own.folder.clone(), own.prefix.clone(), own.library.clone())
}
