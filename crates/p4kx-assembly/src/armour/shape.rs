// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_character::{Dna, DnaRig, RigLogic};
use p4kx_gltf::MorphTarget;
use p4kx_model::{Mesh, Model};

use super::expressions::{Expressions, Morphs, logic_of, morphs};
use super::facial::{Facial, facial};
use super::lod_morphs::from_level_zero;
use super::normals::carry_normals;
use crate::{IssueKind, ModelFailure, ModelProvider};

/// The largest DNA rig that is read, 256 MiB; the rig of a human head is about 76 MB.
pub const MAX_DNA_RIG_LEN: u64 = 256 << 20;

/// Returns the path of the DNA rig of the skin at `path` for the level of detail `level`: the
/// file beside the skin with the extension `.dna`, and `_lod<level>` before it from level 1 on.
fn rig_path(path: &str, level: u32) -> Option<String> {
    let (stem, extension) = path.rsplit_once('.')?;
    if !extension.eq_ignore_ascii_case("skin") {
        return None;
    }
    Some(if level == 0 {
        format!("{stem}.dna")
    } else {
        format!("{stem}_lod{level}.dna")
    })
}

/// A skin shaped by a Character's DNA, with the expression morphs `expressions` asked for.
pub(super) struct Shaped {
    pub(super) model: Model,
    pub(super) morphs: Option<Result<Morphs, String>>,
    pub(super) face: Option<Result<(Facial, String), String>>,
}

/// What a shaped head contributes besides its model and morph targets: the joints of its face and
/// the blendshape number of each morph target.
pub(super) struct Worn {
    pub(super) model: Model,
    pub(super) targets: Vec<MorphTarget>,
    pub(super) kinds: Vec<IssueKind>,
    pub(super) face: Option<(Facial, String)>,
    pub(super) numbers: Vec<usize>,
}

/// What a head contributes besides its model: its morph targets, the blendshape number of each,
/// the joints of its face, and what could not be made as issues.
pub(super) struct Outcome {
    pub(super) targets: Vec<MorphTarget>,
    pub(super) kinds: Vec<IssueKind>,
    pub(super) face: Option<(Facial, String)>,
    pub(super) numbers: Vec<usize>,
}

/// Returns the outcome of the morph targets and the face joints of a head, as they were made.
pub(super) fn outcome(
    morphs: Option<Result<Morphs, String>>,
    face: Option<Result<(Facial, String), String>>,
) -> Outcome {
    let mut numbers = Vec::new();
    let (targets, mut kinds) = match morphs {
        Some(Ok(morphs)) => {
            let unknown = morphs
                .unknown
                .into_iter()
                .map(|name| IssueKind::UnknownExpression { name });
            let empty = morphs
                .empty
                .into_iter()
                .map(|name| IssueKind::EmptyExpression { name });
            numbers = morphs.numbers;
            (morphs.targets, unknown.chain(empty).collect())
        }
        Some(Err(reason)) => (Vec::new(), vec![IssueKind::MorphsFailed { reason }]),
        None => (Vec::new(), Vec::new()),
    };
    let face = match face {
        Some(Ok(face)) => Some(face),
        Some(Err(reason)) => {
            kinds.push(IssueKind::FaceJointsFailed { reason });
            None
        }
        None => None,
    };
    Outcome {
        targets,
        kinds,
        face,
        numbers,
    }
}

impl Shaped {
    /// Returns the model, its morph targets and what could not be made of them as issues.
    pub(super) fn into_parts(self) -> Worn {
        let made = outcome(self.morphs, self.face);
        Worn {
            model: self.model,
            targets: made.targets,
            kinds: made.kinds,
            face: made.face,
            numbers: made.numbers,
        }
    }
}

/// Returns `model`, the skin at `path`, with its vertices moved to the shape `dna` describes.
///
/// The moves are the neutral morphs of the DNA rig beside the skin, mixed by the DNA's weights;
/// the normals follow the new surface. The morph targets of `expressions` are made from the same
/// rig; their failure does not fail the shape.
pub(super) fn shaped(
    model: &Model,
    path: &str,
    dna: &Dna,
    expressions: &Expressions,
    models: &dyn ModelProvider,
    face: bool,
) -> Result<Shaped, String> {
    let level = model
        .lods
        .first()
        .map(|lod| lod.level)
        .ok_or("the model has no level of detail")?;
    let rig_path = rig_path(path, level).ok_or("the model is no skin, it has no DNA rig")?;
    let bytes = models
        .file(&rig_path, MAX_DNA_RIG_LEN)
        .map_err(|failure| match failure {
            ModelFailure::Missing => format!("the DNA rig {rig_path} does not exist"),
            other => format!("the DNA rig {rig_path} cannot be read: {other}"),
        })?;
    let rig = DnaRig::read(&bytes).map_err(|error| format!("{rig_path}: {error}"))?;
    if rig.header().rig_hash != dna.gender_hash {
        return Err(format!(
            "the DNA is for the rig {:#010x}, {rig_path} is the rig {:#010x}",
            dna.gender_hash,
            rig.header().rig_hash
        ));
    }
    let failed = |error| format!("{rig_path}: {error}");
    let matrix = rig.matrix(dna).map_err(failed)?;
    let moves = rig
        .neutral_morphs()
        .and_then(|morphs| morphs.decode(&matrix))
        .map_err(failed)?;
    let reshaped = moved(model, &moves).map_err(|problem| format!("{rig_path}: {problem}"))?;
    let [mesh] = reshaped
        .lods
        .first()
        .map_or(&[][..], |lod| lod.meshes.as_slice())
    else {
        return Err("the shaped model lost its mesh".to_owned());
    };
    let wants_morphs = *expressions != Expressions::None;
    let wants_face = face && level == 0;
    let logic = (level == 0 && (wants_morphs || wants_face)).then(|| logic_of(model, path));
    let morphs = if level == 0 {
        logic.as_ref().filter(|_| wants_morphs).map(|logic| {
            with_logic(logic, |logic| {
                morphs(&rig, &matrix, mesh, logic, expressions)
            })
        })
    } else {
        wants_morphs.then(|| lower_morphs(path, dna, (mesh, expressions), models))
    };
    let face = logic.as_ref().filter(|_| wants_face).map(|logic| {
        with_logic(logic, |logic| {
            facial(model, logic, &rig, &matrix).map(|found| (found, rig_path.clone()))
        })
    });
    Ok(Shaped {
        model: reshaped,
        morphs,
        face,
    })
}

/// Makes the morph targets of a head at a level below 0, whose rig has no expression data, by
/// carrying those of the shaped head at level 0 to `mesh`.
fn lower_morphs(
    path: &str,
    dna: &Dna,
    (mesh, expressions): (&Mesh, &Expressions),
    models: &dyn ModelProvider,
) -> Result<Morphs, String> {
    from_level_zero(path, models, (mesh, expressions), |base| {
        let made = shaped(&base, path, dna, expressions, models, false)?;
        let morphs = made
            .morphs
            .ok_or_else(|| "the head at level 0 made no morph targets".to_owned())??;
        Ok((made.model, morphs))
    })
}

/// Applies `make` to the rig logic read for the skin, or returns the failure to read it.
fn with_logic<T>(
    logic: &Result<RigLogic, String>,
    make: impl FnOnce(&RigLogic) -> Result<T, String>,
) -> Result<T, String> {
    logic.as_ref().map_err(Clone::clone).and_then(make)
}

/// Returns a copy of the skin `model`, a head with exactly one mesh, with its vertices moved by
/// `moves` and without its `RigLogic` chunk, which shaping has parsed.
fn moved(model: &Model, moves: &[[f32; 3]]) -> Result<Model, String> {
    let mut reshaped = model.clone();
    reshaped.rig_logic = None;
    let lod = reshaped
        .lods
        .first_mut()
        .ok_or("the model has no level of detail")?;
    let [mesh] = lod.meshes.as_mut_slice() else {
        return Err(format!(
            "the skin has {} meshes, a head has exactly one",
            lod.meshes.len()
        ));
    };
    apply(mesh, moves)?;
    Ok(reshaped)
}

/// Moves the vertices of `mesh` by the moves of their stream vertices.
///
/// The moves must cover exactly the vertices of the mesh, which holds every vertex of the
/// stream, and every move and moved position must be finite.
fn apply(mesh: &mut Mesh, moves: &[[f32; 3]]) -> Result<(), String> {
    if moves.len() != mesh.positions.len() {
        return Err(format!(
            "the rig covers {} vertices, the skin has {}",
            moves.len(),
            mesh.positions.len()
        ));
    }
    if moves.iter().flatten().any(|value| !value.is_finite()) {
        return Err("the rig gives a move that is not finite".to_owned());
    }
    let old = mesh.positions.clone();
    for (position, &source) in mesh.positions.iter_mut().zip(&mesh.source) {
        let by = usize::try_from(source)
            .ok()
            .and_then(|at| moves.get(at))
            .ok_or_else(|| {
                format!("the skin uses the vertex {source}, beyond the rig's vertices")
            })?;
        for (value, delta) in position.iter_mut().zip(by) {
            *value += delta;
        }
    }
    if mesh
        .positions
        .iter()
        .flatten()
        .any(|value| !value.is_finite())
    {
        return Err("a moved vertex is not finite".to_owned());
    }
    carry_normals(mesh, &old);
    Ok(())
}
