// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_character::{Character, Dna, Guid as ChfGuid, ItemPort, lowered_hash, skin_attachment};
use p4kx_dataforge::Guid;
use p4kx_extraction::SourceLocation;

use super::piece::Note;
use super::{Head, HeadError, Look, Piece, Wearable};

const HEAD_PORT: &str = "head_itemport";
const BODY_PORT: &str = "body_itemport";
const EYES_PORT: &str = "eyes_itemport";
const HAIR_PORT: &str = "hair_itemport";
const SCALP_PORT: &str = "universal_scalp_itemport";
const TEETH: &str = "teeth";

/// The names of the item ports of a head, which a CHF names by their hashes.
const PORTS: [&str; 16] = [
    EYES_PORT,
    HAIR_PORT,
    "hat_itemport",
    "teeth_itemport",
    "eye_accessories_itemport",
    "eyebrow_itemport",
    "eyelashes_itemport",
    "eyedetail_itemport",
    "beard_itemport",
    "stubble_itemport",
    "skin_variant",
    "piercings_itemport",
    "piercings_nose_itemport",
    "piercings_eyebrows_itemport",
    "piercings_mouth_itemport",
    SCALP_PORT,
];

/// A material library selected by a Character's base-material GUID.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct HeadMaterial {
    /// The chosen library path below Data.
    pub path: String,
    /// The original DataForge field that supplied the path, when observed.
    pub source: Option<SourceLocation>,
}

impl HeadMaterial {
    /// Returns a selected path with optional original DataForge evidence.
    #[must_use]
    pub const fn new(path: String, source: Option<SourceLocation>) -> Self {
        Self { path, source }
    }
}

/// Supplies what the items of a Character's head are made of.
pub trait HeadItems {
    /// Reads the Entity `item` as something to wear.
    ///
    /// # Errors
    ///
    /// Returns the reason when there is no such Entity or its Record cannot be read.
    fn wearable(&mut self, item: Guid) -> Result<Wearable, String>;

    /// Returns the material library, a path below `Data`, that the material `material` of a
    /// Character stands for, or `None` when it is in no lookup table.
    fn material_path(&self, material: Guid) -> Option<String>;

    /// Returns the chosen library path with its original source field when observed.
    fn material_observed(&self, material: Guid) -> Option<HeadMaterial> {
        self.material_path(material)
            .map(|path| HeadMaterial::new(path, None))
    }
}

const fn forge_guid(guid: ChfGuid) -> Guid {
    Guid::from_stored(guid.stored())
}

/// Returns the name of the item port with the hash `hash`: one of the known ports, else
/// `port_<hash>`.
fn port_name(hash: u32) -> String {
    PORTS
        .iter()
        .find(|name| lowered_hash(name) == hash)
        .map_or_else(|| format!("port_{hash:08x}"), |name| (*name).to_owned())
}

/// Returns the first port with the hash `hash` in the tree below `port`, in depth-first order.
fn find(port: &ItemPort, hash: u32) -> Option<&ItemPort> {
    if port.port == hash {
        return Some(port);
    }
    port.children.iter().find_map(|child| find(child, hash))
}

/// What an item of the head needs to be worn as the Character chose it.
struct Dressing<'a> {
    character: &'a Character,
    dna: &'a Dna,
}

impl Dressing<'_> {
    /// Returns the material choice for the item port `port`; the scalp takes the hair's.
    fn choice_for(&self, port: &str) -> Option<&p4kx_character::Material> {
        let wanted = if port == SCALP_PORT { HAIR_PORT } else { port };
        let hash = skin_attachment(wanted);
        self.character
            .materials
            .iter()
            .find(|material| material.attachment == hash)
    }

    /// Draws `piece` as the Character chose its materials for `port`.
    fn styled(&self, mut piece: Piece, port: &str, items: &dyn HeadItems) -> Piece {
        let Some(choice) = self.choice_for(port) else {
            return piece;
        };
        if choice.base_material != ChfGuid::default() {
            let base = forge_guid(choice.base_material);
            piece = if let Some(selected) = items.material_observed(base) {
                piece.with_material_source(selected)
            } else {
                piece.noting(Note::UnknownMaterial(base.to_string()))
            };
        }
        let look = Look::new(choice);
        let textures = look.texture_choices();
        if textures > 0 {
            let what = format!("the {textures} texture choices (blemishes, makeup, tattoos)");
            piece = piece.noting(Note::LookIgnored(what));
        }
        piece.looking(look)
    }

    /// Returns the piece for the item of the port `item`, which sits at the port named `name`.
    fn piece(&self, item: &ItemPort, items: &mut dyn HeadItems) -> Option<Piece> {
        let name = port_name(item.port);
        let wearable = match items.wearable(forge_guid(item.item)) {
            Ok(wearable) => wearable,
            Err(reason) => return Some(Piece::failed(name.clone(), reason).at_port(name)),
        };
        if wearable.geometry().is_none() && wearable.variants().is_empty() {
            return None;
        }
        let mut piece = Piece::wearable(wearable)
            .named(name.clone())
            .at_port(name.clone());
        piece = if name == EYES_PORT {
            piece.shaped(self.dna.clone())
        } else {
            piece.following_head()
        };
        Some(self.styled(piece, &name, items))
    }
}

impl Head {
    /// Makes the head of `character` with what the Character wears on it: the head, the teeth, and
    /// each item of its head port (eyes, hair, brows, lashes, beard) that has geometry.
    ///
    /// The head, the teeth and the eyes are shaped by the Character's DNA, the other items follow
    /// the shaped head. The Character's material choices are applied to the pieces: its base
    /// material replaces the library of the head, and its colours and numbers set the skin tone,
    /// the iris and the hair, see [`Look`]. The choice the Character made for its body is given by
    /// a last piece, [`Piece::body`], that styles the default body the same way, so that the body
    /// and the head have one skin tone. An item that `items` cannot supply is kept as a failed
    /// piece, and a base material that is in no lookup table is listed, so that the Armour's
    /// report names both.
    ///
    /// # Errors
    ///
    /// As [`Head::from_character`].
    pub fn dressed(character: &Character, items: &mut dyn HeadItems) -> Result<Self, HeadError> {
        let mut head = Self::from_character(character)?;
        let dna = character.dna.as_ref().ok_or(HeadError::NoFace)?;
        let dressing = Dressing { character, dna };
        head.pieces = std::mem::take(&mut head.pieces)
            .into_iter()
            .map(|skin| dressing.styled(skin, HEAD_PORT, items))
            .collect();
        head.pieces.push(
            Piece::file(
                TEETH,
                format!("{}/{}_teeth.skin", head.folder(), head.rig),
                None,
            )
            .shaped(dna.clone()),
        );
        let top = character
            .item_ports
            .as_ref()
            .and_then(|root| find(root, lowered_hash(HEAD_PORT)));
        for item in top.iter().flat_map(|port| &port.children) {
            if let Some(piece) = dressing.piece(item, items) {
                head.pieces.push(piece);
            }
        }
        if dressing.choice_for(BODY_PORT).is_some() {
            head.pieces
                .push(dressing.styled(Piece::body(), BODY_PORT, items));
        }
        Ok(head)
    }
}
