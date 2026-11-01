// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_character::{Character, Guid};
use thiserror::Error;

use super::piece::Note;
use super::{BodyVariant, Expressions, Piece};

/// The rigs of the heads: the body variant, the CRC-32C of the rig's name, which is the
/// `gender_hash` of the DNA of the Characters made for it, the GUID of the body type the CHFs of
/// that rig hold, and the name.
const RIGS: [(BodyVariant, u32, Guid, &str); 2] = [
    (
        BodyVariant::Male,
        0xdd6c_67f6,
        Guid::from_stored([
            0x61, 0x4a, 0x6b, 0x14, 0xd5, 0x39, 0xf4, 0x25, 0x49, 0x8a, 0xb6, 0xdf, 0x86, 0xa4,
            0x99, 0xa9,
        ]),
        "protos_human_male_face_t1_pu",
    ),
    (
        BodyVariant::Female,
        0x9ef4_eb54,
        Guid::from_stored([
            0xad, 0x4c, 0xb0, 0xef, 0x94, 0x4a, 0x79, 0xd0, 0x53, 0xc2, 0xd3, 0xb4, 0x58, 0x25,
            0x38, 0xad,
        ]),
        "protos_human_female_face_t1_pu",
    ),
];

/// Returns what the file name `stem` adds to the name of a stock head rig, e.g. `eyes` for
/// `protos_human_male_face_t1_pu_eyes`, ignoring ASCII case; `None` for any other name.
pub(crate) fn rig_part(stem: &str) -> Option<&str> {
    RIGS.iter().find_map(|(_, _, _, rig)| {
        let rest = stem.get(rig.len()..)?;
        let head = stem.get(..rig.len())?;
        if !head.eq_ignore_ascii_case(rig) {
            return None;
        }
        rest.strip_prefix('_').filter(|part| !part.is_empty())
    })
}

/// Why the head of a Character cannot be made.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum HeadError {
    /// The Character holds no face DNA, so it has no head to shape.
    #[error("the character holds no face DNA")]
    NoFace,
    /// The DNA is made for a head rig this tool does not know.
    #[error("the DNA belongs to the unknown head rig {gender_hash:#010x}")]
    UnknownRig {
        /// The `gender_hash` of the DNA.
        gender_hash: u32,
    },
}

/// The head of a Character: the skin of the head rig that fits the Character's body, shaped by the
/// Character's face DNA.
///
/// Wear it with [`Armour::build`](super::Armour::build) on the [`BodyVariant`] it names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Head {
    pub(super) variant: BodyVariant,
    pub(super) rig: &'static str,
    pub(super) pieces: Vec<Piece>,
}

impl Head {
    /// Makes the head of `character`: the body variant and the skin, shaped by the Character's
    /// DNA, of the head rig the DNA's `gender_hash` names.
    ///
    /// A Character whose body type is not the one of that rig is still made; the head piece
    /// carries [`IssueKind::BodyTypeMismatch`](crate::IssueKind::BodyTypeMismatch), which the
    /// report of [`Armour::build`](super::Armour::build) lists. The DNA's `variant_hash` is not
    /// checked.
    ///
    /// # Errors
    ///
    /// Returns [`HeadError::NoFace`] for a Character without DNA and [`HeadError::UnknownRig`]
    /// when the DNA names a rig that is neither the male nor the female head.
    pub fn from_character(character: &Character) -> Result<Self, HeadError> {
        let dna = character.dna.as_ref().ok_or(HeadError::NoFace)?;
        let (variant, _, body_type, rig) = RIGS
            .iter()
            .find(|(_, hash, _, _)| *hash == dna.gender_hash)
            .ok_or(HeadError::UnknownRig {
                gender_hash: dna.gender_hash,
            })?;
        let folder = format!("Objects/Characters/Human/heads/{}/pu/{rig}", variant.code());
        let mut head =
            Piece::file("head", format!("{folder}/{rig}_head.skin"), None).shaped(dna.clone());
        if character.body_type != *body_type {
            head = head.noting(Note::BodyTypeMismatch {
                body_type: character.body_type.to_string(),
                rig,
            });
        }
        Ok(Self {
            variant: *variant,
            rig,
            pieces: vec![head],
        })
    }

    /// Returns the folder below `Data` that holds the skins and DNA rigs of the head rig.
    pub(super) fn folder(&self) -> String {
        format!(
            "Objects/Characters/Human/heads/{}/pu/{}",
            self.variant.code(),
            self.rig
        )
    }

    /// Returns the head with the expression blendshapes `expressions` asks for written as morph
    /// targets of the mesh of the piece named `head`, see [`Piece::with_expressions`]. The teeth
    /// and eyes get none: only the head skin carries the expression blendshapes. The hair, brows,
    /// lashes and beard follow the head's targets with morph targets of their own.
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

    /// Returns the body variant the head belongs on.
    #[must_use]
    pub const fn variant(&self) -> BodyVariant {
        self.variant
    }

    /// Returns the name of the head rig, e.g. `protos_human_male_face_t1_pu`.
    #[must_use]
    pub const fn rig(&self) -> &'static str {
        self.rig
    }

    /// Returns the pieces to wear on the body: the shaped head, and with [`Head::dressed`] its
    /// teeth, the items of the Character's head port and, if the Character chose one, the
    /// [`Piece::body`] (adds no skin) that styles its body.
    #[must_use]
    pub fn pieces(&self) -> &[Piece] {
        &self.pieces
    }
}
