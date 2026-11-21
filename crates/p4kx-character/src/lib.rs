// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Reads and writes the CHF character files of Star Citizen: the fixed-size container with its
//! magic, CRC-32C and zstd-compressed body, the body with its inner versions, body type, face DNA,
//! item port tree and material choices, and a JSON form that converts back byte-exactly.
//!
//! Reads the DNA rigs (`.dna` files) the face DNA of a Character is applied to: [`DnaRig`] gives
//! the names, joint offsets, neutral and expression morphs, joint and region weights and delta
//! records of the heads of a rig, and [`DnaMatrix`] the weights with which a Character's DNA
//! blends them. [`ExpressionBlend`] turns the expression morphs of the stock heads into the
//! blendshapes of a Character's head, and [`RigLogic`] names them.
//!
//! A head that ships no DNA rig, like a story character's own, has its joint deltas in its
//! `.RigLogic` file ([`FaceRig::from_logic`]) and its blendshapes in the `BShapesGpu` chunk of its
//! skin geometry ([`SkinBlendshapes`]).

mod body_reader;
mod body_writer;
mod character;
mod chf;
mod container;
mod diff;
mod diff_pair;
mod diff_source;
mod diff_tree;
mod dna;
mod error;
mod guid;
mod item_port;
mod json_tree;
mod json_values;
mod label;
mod material;
mod name_table;
mod named_json;
mod named_json_field;
mod names;
mod rig;
mod rig_blend;
mod rig_bytes;
mod rig_deltas;
mod rig_error;
mod rig_expression;
mod rig_logic;
mod rig_logic_deltas;
mod rig_matrix;
mod rig_neutral;
mod rig_pose;
mod rig_tables;
mod skin_blendshapes;

pub use character::{Character, FEMALE_VERSIONS, MALE_VERSIONS, Versions};
pub use chf::{Chf, ChfContainer};
pub use container::{CHF_MAGIC, CHF_SIZE, MAX_BODY_LEN};
pub use diff::{ChfChange, diff_chf, diff_chf_observed};
pub use diff_source::{ChfDiffLookup, ChfDiffMissing, ChfDiffSite, ChfObservedChange};
pub use dna::{DNA_MAGIC, Dna, DnaBlend, MAX_FACE_PARTS, face_part_name};
pub use error::ChfError;
pub use guid::{Guid, InvalidGuid};
pub use item_port::{ItemPort, MAX_PORT_DEPTH};
pub use label::Label;
pub use material::{ColorParam, FloatParam, Material, SubMaterial, Texture};
pub use name_table::{MAX_NAME_LEN, MAX_NAMES, NameTable};
pub use named_json::{LABEL_SUFFIX, NameSummary};
pub use named_json_field::{ChfJsonField, ChfJsonName, ChfJsonNameUse, ChfNameKind};
pub use names::{
    BODY_COLOR, EYE_COLOR, HAIR_DYE_COLOR, HAIR_TINT_COLOR, lowered_hash, name_hash,
    skin_attachment,
};
pub use rig::{DnaRig, RIG_MAGIC, RigHeader};
pub use rig_blend::{ExpressionBlend, ExpressionTarget};
pub use rig_deltas::{
    DeltaEntry, DeltaRecord, DeltaRecords, DeltaSet, VariantDeltas, VariantHeader,
};
pub use rig_error::RigError;
pub use rig_expression::{ExpressionCell, ExpressionMorphs};
pub use rig_logic::{RIG_LOGIC_MAGIC, RigLogic, RigLogicError};
pub use rig_matrix::{DnaMatrix, MAX_RIG_CELLS};
pub use rig_neutral::NeutralMorphs;
pub use rig_pose::{FacePose, FaceRig};
pub use rig_tables::{JointWeights, RegionMasks, VariantJoints};
pub use skin_blendshapes::{MAX_SKIN_BLENDSHAPES, SkinBlendshapes, SkinBlendshapesError};
