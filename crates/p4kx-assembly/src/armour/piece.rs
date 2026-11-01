// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_character::Dna;

use super::{Expressions, Look, Wearable};
use crate::PortTransform;
use crate::bespoke::Bespoke;

/// The port an unskinned piece hangs on: the helper of the holder it is placed at, the offset from
/// it and the helper of the piece that goes there. The offset is kept as the bits of its numbers
/// so that equal pieces compare equal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Mount {
    /// The name of the piece whose model holds the helper; `None` for the body.
    pub(crate) holder: Option<String>,
    pub(crate) helper: String,
    pub(crate) offset_helper: String,
    position: [u32; 3],
    rotation: [u32; 3],
}

impl Mount {
    /// Returns the mount of an item at the port `port` of `holder`.
    pub(crate) fn new(holder: Option<String>, port: &PortTransform) -> Self {
        Self {
            holder,
            helper: port.helper().to_owned(),
            offset_helper: port.offset_helper().to_owned(),
            position: port.position().map(f32::to_bits),
            rotation: port.rotation().map(f32::to_bits),
        }
    }

    pub(crate) fn position(&self) -> [f32; 3] {
        self.position.map(f32::from_bits)
    }

    pub(crate) fn rotation(&self) -> [f32; 3] {
        self.rotation.map(f32::from_bits)
    }
}

/// Where the geometry of a [`Piece`] comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Source {
    /// An Entity; its geometry is chosen by the body variant.
    Wearable(Box<Wearable>),
    /// A skin file, wherever it comes from.
    File {
        path: String,
        material: Option<String>,
    },
    /// The default body, which the piece restyles and adds no skin to.
    Body,
    /// Something that could not be turned into a piece, with the reason.
    Failed(String),
}

/// Something to list about a piece that does not keep it from being worn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Note {
    /// The Character's base material is in no lookup table.
    UnknownMaterial(String),
    /// Parts of the Character's material choice that are not drawn.
    LookIgnored(String),
    /// The Character's body type is not the one of the head rig its DNA names.
    BodyTypeMismatch {
        body_type: String,
        rig: &'static str,
    },
    /// The Loadout of the piece's Entity could not be read in full.
    LoadoutUnreadable(String),
}

/// One piece worn on a body: a piece of armour or clothing, or any skin that belongs on the
/// skeleton.
///
/// A piece comes from an Entity ([`Piece::wearable`]) or from a file ([`Piece::file`]), so that
/// pieces from a Loadout or a head from a character file are built the same way. One that could
/// not be found is kept as [`Piece::failed`], so that the report lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Piece {
    pub(crate) name: String,
    pub(crate) source: Source,
    pub(crate) shape: Option<Dna>,
    pub(crate) expressions: Expressions,
    pub(crate) look: Option<Look>,
    pub(crate) material: Option<String>,
    pub(crate) follows_head: bool,
    pub(crate) notes: Vec<Note>,
    pub(crate) selecting: Vec<String>,
    pub(crate) variant_tags: Vec<String>,
    pub(crate) bespoke: Option<Bespoke>,
    pub(crate) mount: Option<Mount>,
    pub(crate) port: Option<String>,
}

impl Piece {
    const fn from_source(name: String, source: Source) -> Self {
        Self {
            name,
            source,
            shape: None,
            expressions: Expressions::None,
            look: None,
            material: None,
            follows_head: false,
            notes: Vec::new(),
            selecting: Vec::new(),
            variant_tags: Vec::new(),
            bespoke: None,
            mount: None,
            port: None,
        }
    }

    /// Says the piece sits at the Loadout port `port`, so that a worn piece whose Record hides
    /// the port, see [`Wearable::hidden_ports`], leaves it out of the [`Armour`](super::Armour).
    #[must_use]
    pub fn at_port(mut self, port: impl Into<String>) -> Self {
        self.port = Some(port.into());
        self
    }

    /// Returns the name of the port the piece sits at, if it was given.
    #[must_use]
    pub fn port(&self) -> Option<&str> {
        self.port.as_deref()
    }

    /// Returns the ports whose items the piece hides when it is worn, see
    /// [`Wearable::hidden_ports`]; none for a piece that is not an Entity.
    #[must_use]
    pub fn hidden_ports(&self) -> &[String] {
        match &self.source {
            Source::Wearable(wearable) => wearable.hidden_ports(),
            _ => &[],
        }
    }

    pub(crate) fn mounted(mut self, mount: Mount) -> Self {
        self.mount = Some(mount);
        self
    }

    /// Takes the sub-geometry of the Entity that is tagged with one of `flags`, the flags of the
    /// port the Entity sits at, before the sub-geometry of the body.
    #[must_use]
    pub fn selected_by(mut self, flags: Vec<String>) -> Self {
        self.selecting = flags;
        self
    }

    /// Chooses the Entity's material library by `tags`, see [`MaterialVariant::resolve`]; the
    /// body variant's tag is always among them.
    #[must_use]
    pub fn with_material_tags(mut self, tags: Vec<String>) -> Self {
        self.variant_tags = tags;
        self
    }

    /// Wears the skin a story character's own head has for this piece instead of the generic
    /// Protos skin its Record names, see [`Bespoke::skin_for`]. A skin the Archive lacks leaves
    /// the Record's. A material library of the Record that the Archive lacks is replaced by the
    /// head's own for such a skin.
    #[must_use]
    pub(crate) fn with_bespoke(mut self, own: Bespoke) -> Self {
        self.bespoke = Some(own);
        self
    }

    /// Wears the Entity `wearable`; the piece is named after its class name.
    #[must_use]
    pub fn wearable(wearable: Wearable) -> Self {
        Self::from_source(
            wearable.class_name().to_owned(),
            Source::Wearable(Box::new(wearable)),
        )
    }

    /// Wears the skin at `path`, a path below `Data`, in the node `name`, drawn with the
    /// material library `material` when given.
    #[must_use]
    pub fn file(
        name: impl Into<String>,
        path: impl Into<String>,
        material: Option<String>,
    ) -> Self {
        Self::from_source(
            name.into(),
            Source::File {
                path: path.into(),
                material,
            },
        )
    }

    /// Stands for the default body of the [`Armour`](super::Armour), so that the material library
    /// and the [`Look`] given to it, see [`Piece::with_material`] and [`Piece::looking`], style
    /// the body skin. It is no skin of its own and makes no part; when several are worn, only the
    /// first restyles the body and the others are ignored.
    #[must_use]
    pub fn body() -> Self {
        Self::from_source("body".to_owned(), Source::Body)
    }

    /// Stands for a piece `name` that could not be found or read, because of `reason`.
    #[must_use]
    pub fn failed(name: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::from_source(name.into(), Source::Failed(reason.into()))
    }

    /// Shapes the piece by `dna`, the face DNA of a Character: the vertices of the piece's model
    /// are moved by the neutral morphs of the DNA rig that sits beside the model, see
    /// [`DnaRig`](p4kx_character::DnaRig). A piece whose rig is missing or does not fit its model
    /// is left out and listed as [`IssueKind::ShapeFailed`](crate::IssueKind::ShapeFailed).
    #[must_use]
    pub fn shaped(mut self, dna: Dna) -> Self {
        self.shape = Some(dna);
        self
    }

    /// Draws the piece with the material library at `path`, a path below `Data`, instead of the
    /// one its Record or its model names.
    #[must_use]
    pub fn with_material(mut self, path: impl Into<String>) -> Self {
        self.material = Some(path.into());
        self
    }

    /// Applies the material choices of a Character to the piece's materials, see [`Look`].
    #[must_use]
    pub fn looking(mut self, look: Look) -> Self {
        self.look = Some(look);
        self
    }

    /// Makes the piece follow the shape of the head piece named `head`: its vertices move as the
    /// vertices of the shaped head around them did, and the first mesh takes a morph target for
    /// each expression target of the head that moves it. For hair, brows, lashes and beards.
    #[must_use]
    pub const fn following_head(mut self) -> Self {
        self.follows_head = true;
        self
    }

    pub(crate) fn noting(mut self, note: Note) -> Self {
        self.notes.push(note);
        self
    }

    /// Writes the expression blendshapes `expressions` names as morph targets of the piece's mesh,
    /// mixed by the DNA the piece is shaped by, see [`Piece::shaped`], so that they move the
    /// Character's own neutral face. A piece that is not shaped has none. The targets are named
    /// by the `RigLogic` chunk of the skin; if they cannot be made the piece is kept without them
    /// and listed as [`IssueKind::MorphsFailed`](crate::IssueKind::MorphsFailed), and a name no
    /// blendshape has as [`IssueKind::UnknownExpression`](crate::IssueKind::UnknownExpression).
    #[must_use]
    pub fn with_expressions(mut self, expressions: Expressions) -> Self {
        self.expressions = expressions;
        self
    }

    /// Renames the piece's node, e.g. to `<port>/<entity>`.
    #[must_use]
    pub fn named(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    /// Returns the name of the piece's node.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }
}
