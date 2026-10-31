// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The face of a story character's own head: the joint deltas of its `.RigLogic` file and the
//! blendshapes of its skin geometry, for a head that ships no DNA rig.

use std::collections::HashMap;

use p4kx_character::{RigLogic, SkinBlendshapes};
use p4kx_chunks::{ChunkFile, ivo};
use p4kx_model::{Bone, Model};

use super::expressions::{Expressions, Morphs, targets_of};
use super::facial::{bones_of, own_facial};
use super::lod_morphs::from_level_zero;
use super::shape::{Outcome, outcome};
use crate::{IssueKind, ModelFailure, ModelProvider};

/// The folder below `Data` that holds the `.RigLogic` file of every head rig.
const LOGIC_FOLDER: &str = "Objects/Characters/RigLogic";
const HEAD_SUFFIX: &str = "_head.skin";
/// The largest geometry file read for the blendshapes of a head's own rig, 64 MiB.
const MAX_OWN_RIG_GEOMETRY_LEN: u64 = 64 << 20;
/// The largest `.RigLogic` file that is read, 4 MiB; the largest in LIVE has 0.5 MB.
const MAX_LOGIC_LEN: u64 = 4 << 20;
/// The prefix of the names of the facial bones, in lower case.
const FACIAL_PREFIX: &str = "facial_";

/// Returns the path of the `.RigLogic` file of the head whose skin is at `skin`: the file named
/// like the skin without `_head.skin` in [`LOGIC_FOLDER`]; `None` for any other skin.
pub(super) fn logic_path(skin: &str) -> Option<String> {
    let name = skin.rsplit(['/', '\\']).next()?;
    let cut = name.len().checked_sub(HEAD_SUFFIX.len())?;
    let (stem, tail) = (name.get(..cut)?, name.get(cut..)?);
    (!stem.is_empty() && tail.eq_ignore_ascii_case(HEAD_SUFFIX))
        .then(|| format!("{LOGIC_FOLDER}/{stem}.RigLogic"))
}

/// Reads the `.RigLogic` file at `path`; `None` when the Archive has no such file.
fn read_logic(path: &str, models: &dyn ModelProvider) -> Result<Option<RigLogic>, String> {
    let bytes = match models.file(path, MAX_LOGIC_LEN) {
        Ok(bytes) => bytes,
        Err(ModelFailure::Missing) => return Ok(None),
        Err(other) => return Err(format!("{path} cannot be read: {other}")),
    };
    let logic = RigLogic::read(&bytes).map_err(|error| format!("{path}: {error}"))?;
    if logic.has_joint_deltas() {
        Ok(Some(logic))
    } else {
        Err(format!("{path} carries no joint delta records"))
    }
}

fn parent_name(bones: &[Bone], bone: &Bone) -> Option<String> {
    bone.parent
        .and_then(|parent| bones.get(parent))
        .map(|parent| parent.name.to_ascii_lowercase())
}

/// Returns the bones `skin` with the bind poses of the same-named bones of `file`, the skeleton
/// file at `path`, after checking that every facial bone of `skin` is in `file` with the same
/// parent and controller ID.
fn posed(skin: &[Bone], file: &[Bone], path: &str) -> Result<Vec<Bone>, String> {
    let own: HashMap<String, &Bone> = file
        .iter()
        .map(|bone| (bone.name.to_ascii_lowercase(), bone))
        .collect();
    for bone in skin {
        let name = bone.name.to_ascii_lowercase();
        if !name.starts_with(FACIAL_PREFIX) {
            continue;
        }
        let other = own
            .get(&name)
            .ok_or_else(|| format!("{path} lacks the facial bone {}", bone.name))?;
        if parent_name(skin, bone) != parent_name(file, other) {
            return Err(format!(
                "{path}: the facial bone {} has another parent than in the skin",
                bone.name
            ));
        }
        if bone.controller_id != other.controller_id {
            return Err(format!(
                "{path}: the facial bone {} has another controller ID than in the skin",
                bone.name
            ));
        }
    }
    Ok(skin
        .iter()
        .map(|bone| {
            let mut bone = bone.clone();
            if let Some(found) = own.get(&bone.name.to_ascii_lowercase()) {
                bone.relative = found.relative;
            }
            bone
        })
        .collect())
}

/// Returns the bones of the head `model` with the bind poses of the skeleton file
/// `<skin>_skel.chr` beside its skin at `skin`; `None` when the Archive has no such file.
///
/// The skin's own copy of the facial bones is the stock head's, symmetric; the skeleton file has
/// the joints where the character's face puts them.
fn posed_bones(
    model: &Model,
    skin: &str,
    models: &dyn ModelProvider,
) -> Result<Option<Vec<Bone>>, String> {
    let Some((stem, _)) = skin.rsplit_once('.') else {
        return Ok(None);
    };
    let path = format!("{stem}_skel.chr");
    let file = match models.model(&path, 0) {
        Ok(provided) => provided.model,
        Err(ModelFailure::Missing) => return Ok(None),
        Err(other) => return Err(format!("{path} cannot be read: {other}")),
    };
    posed(bones_of(model), bones_of(&file), &path).map(Some)
}

/// Reads the blendshapes of the head whose skin is at `skin` from the `BShapesGpu` chunk of its
/// geometry file and makes the morph targets `expressions` asks for of its `mesh`.
fn morphs(
    model: &Model,
    skin: &str,
    (logic, expressions): (&RigLogic, &Expressions),
    models: &dyn ModelProvider,
) -> Result<Morphs, String> {
    let [mesh] = model
        .lods
        .first()
        .map_or(&[][..], |lod| lod.meshes.as_slice())
    else {
        return Err("a head has exactly one mesh".to_owned());
    };
    let geometry = format!("{skin}m");
    let bytes = models
        .file(&geometry, MAX_OWN_RIG_GEOMETRY_LEN)
        .map_err(|failure| format!("{geometry} cannot be read: {failure}"))?;
    let chunks =
        ChunkFile::parse(bytes.as_slice()).map_err(|error| format!("{geometry}: {error}"))?;
    let (index, _) = chunks
        .chunks_of(ivo::BSHAPES_GPU)
        .next()
        .ok_or_else(|| format!("{geometry} has no BShapesGpu chunk"))?;
    let data = chunks
        .chunk_data(index)
        .ok_or_else(|| format!("{geometry}: the BShapesGpu chunk lies outside the file"))?;
    let shapes = SkinBlendshapes::read(data).map_err(|error| format!("{geometry}: {error}"))?;
    if shapes.vertices() != mesh.positions.len() {
        return Err(format!(
            "{geometry} covers {} vertices, the skin has {}",
            shapes.vertices(),
            mesh.positions.len()
        ));
    }
    if shapes.blendshapes() != logic.blendshapes() {
        return Err(format!(
            "{geometry} has {} blendshapes, the rig logic {}",
            shapes.blendshapes(),
            logic.blendshapes()
        ));
    }
    Ok(targets_of(&shapes, logic, mesh, expressions))
}

/// Makes the morph targets of the head `model`, a level below 0, by carrying those of level 0.
fn lower_morphs(
    model: &Model,
    skin: &str,
    (logic, expressions): (&RigLogic, &Expressions),
    models: &dyn ModelProvider,
) -> Result<Morphs, String> {
    let [mesh] = model
        .lods
        .first()
        .map_or(&[][..], |lod| lod.meshes.as_slice())
    else {
        return Err("a head has exactly one mesh".to_owned());
    };
    from_level_zero(skin, models, (mesh, expressions), |base| {
        let made = morphs(&base, skin, (logic, expressions), models)?;
        Ok((base, made))
    })
}

/// Returns what the own rig of the head `model`, whose skin is at `skin`, contributes.
///
/// That is the joints of its face from the `.RigLogic` file, posed by the skeleton file beside the
/// skin, and, for `expressions`, the blendshapes of its geometry as morph targets. A skeleton file
/// that does not match the skin leaves the skin's poses and is listed as an issue. `None` for a
/// head without a `.RigLogic` file. A level of detail below 0 has no joints of its face, and its
/// morph targets are those of level 0 carried over, see [`from_level_zero`].
pub(super) fn own_head(
    model: &Model,
    skin: &str,
    expressions: &Expressions,
    models: &dyn ModelProvider,
) -> Option<Outcome> {
    let lower = model.lods.first()?.level != 0;
    if lower && *expressions == Expressions::None {
        return None;
    }
    let path = logic_path(skin)?;
    let logic = match read_logic(&path, models) {
        Ok(found) => found?,
        Err(reason) => return Some(outcome(None, Some(Err(reason)))),
    };
    if lower {
        return Some(outcome(
            Some(lower_morphs(model, skin, (&logic, expressions), models)),
            None,
        ));
    }
    let made = (*expressions != Expressions::None)
        .then(|| morphs(model, skin, (&logic, expressions), models));
    let (posed, mismatch) = match posed_bones(model, skin, models) {
        Ok(posed) => (posed, None),
        Err(reason) => (None, Some(IssueKind::FaceJointsFailed { reason })),
    };
    let face = (own_facial(model, logic, posed), path);
    let mut made = outcome(made, Some(Ok(face)));
    made.kinds.extend(mismatch);
    Some(made)
}
