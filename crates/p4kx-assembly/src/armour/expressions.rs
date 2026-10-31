// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_character::{
    DnaMatrix, DnaRig, ExpressionBlend, ExpressionTarget, RigLogic, SkinBlendshapes,
};
use p4kx_gltf::MorphTarget;
use p4kx_model::{Mesh, Model, RigLogicBytes};
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};

const NO_VERTEX: u32 = u32::MAX;
const CONTROL_PREFIX: &str = "ctl_";

/// Which expression blendshapes of a head are written as morph targets.
///
/// A blendshape is named like the control of the head rig it stands for, e.g. `CTL_jaw_open`, or,
/// for a corrective, like the controls it combines joined by `*`; a name matches without regard to
/// case and to the prefix `CTL_`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum Expressions {
    /// No morph targets: the neutral face only.
    #[default]
    None,
    /// Every blendshape the Character's head moves.
    All,
    /// The blendshapes with these names; a name no blendshape has is listed as an issue.
    Only(Vec<String>),
}

/// The morph targets of a head and the requested names no blendshape has.
pub(super) struct Morphs {
    pub(super) targets: Vec<MorphTarget>,
    pub(super) numbers: Vec<usize>,
    pub(super) unknown: Vec<String>,
    pub(super) empty: Vec<String>,
}

fn normal(name: &str) -> String {
    name.to_lowercase()
        .split('*')
        .map(|part| part.strip_prefix(CONTROL_PREFIX).unwrap_or(part))
        .collect::<Vec<_>>()
        .join("*")
}

/// Parses the `RigLogic` chunk the provider handed along with the head skin `model` read from
/// `path`.
pub(super) fn logic_of(model: &Model, path: &str) -> Result<RigLogic, String> {
    let data = model
        .rig_logic
        .as_ref()
        .map(RigLogicBytes::as_slice)
        .ok_or_else(|| format!("{path} has no RigLogic chunk"))?;
    RigLogic::read(data).map_err(|error| format!("{path}: {error}"))
}

/// Returns the blendshapes `selection` asks for as `(blendshape, name)` and the names it asks for
/// that the rig lacks.
fn select(logic: &RigLogic, selection: &Expressions) -> (Vec<(usize, String)>, Vec<String>) {
    let all = (0..logic.blendshapes()).filter_map(|number| Some((number, logic.name(number)?)));
    match selection {
        Expressions::None => (Vec::new(), Vec::new()),
        Expressions::All => (all.collect(), Vec::new()),
        Expressions::Only(wanted) => {
            let known: Vec<(usize, String)> = all.collect();
            let mut chosen = Vec::new();
            let mut unknown = Vec::new();
            for name in wanted {
                let key = normal(name);
                let found: Vec<&(usize, String)> = known
                    .iter()
                    .filter(|(_, candidate)| normal(candidate) == key)
                    .collect();
                if found.is_empty() {
                    unknown.push(name.clone());
                }
                for (number, label) in found {
                    if !chosen.iter().any(|(taken, _)| taken == number) {
                        chosen.push((*number, label.clone()));
                    }
                }
            }
            chosen.sort_unstable_by_key(|(number, _)| *number);
            (chosen, unknown)
        }
    }
}

/// Returns the vertices of `mesh` by the stream vertex they were made from: the first mesh vertex
/// of each and the next mesh vertex of the same stream vertex.
fn by_stream_vertex(mesh: &Mesh, stream: usize) -> (Vec<u32>, Vec<u32>) {
    let mut first = vec![NO_VERTEX; stream];
    let mut next = vec![NO_VERTEX; mesh.source.len()];
    for (vertex, &source) in mesh.source.iter().enumerate().rev() {
        let Ok(vertex_id) = u32::try_from(vertex) else {
            continue;
        };
        let slot = usize::try_from(source)
            .ok()
            .and_then(|source| first.get_mut(source));
        if let (Some(slot), Some(link)) = (slot, next.get_mut(vertex)) {
            *link = *slot;
            *slot = vertex_id;
        }
    }
    (first, next)
}

/// Turns a target on stream vertices into one on the vertices of the mesh.
fn on_mesh(target: &ExpressionTarget, first: &[u32], next: &[u32]) -> (Vec<u32>, Vec<[f32; 3]>) {
    let mut moves: Vec<(u32, [f32; 3])> = Vec::with_capacity(target.vertices.len());
    for (&stream, &delta) in target.vertices.iter().zip(&target.deltas) {
        let mut vertex = usize::try_from(stream)
            .ok()
            .and_then(|stream| first.get(stream))
            .copied()
            .unwrap_or(NO_VERTEX);
        while vertex != NO_VERTEX {
            moves.push((vertex, delta));
            vertex = usize::try_from(vertex)
                .ok()
                .and_then(|vertex| next.get(vertex))
                .copied()
                .unwrap_or(NO_VERTEX);
        }
    }
    moves.sort_unstable_by_key(|&(vertex, _)| vertex);
    moves.into_iter().unzip()
}

/// Makes the morph targets `selection` asks for of the head `mesh` from the expression morphs of
/// `rig` mixed by `matrix`; the targets are named by `logic`, the skin's `RigLogic` chunk, ascend
/// by blendshape and leave out the blendshapes that move no vertex.
pub(super) fn morphs(
    rig: &DnaRig<'_>,
    matrix: &DnaMatrix,
    mesh: &Mesh,
    logic: &RigLogic,
    selection: &Expressions,
) -> Result<Morphs, String> {
    let covered = rig.vertex_count().map_err(|error| error.to_string())?;
    if covered != mesh.positions.len() {
        return Err(format!(
            "the rig covers {covered} vertices, the skin has {}",
            mesh.positions.len()
        ));
    }
    let blend = rig
        .expression_blend(matrix)
        .map_err(|error| error.to_string())?;
    if logic.blendshapes() != blend.blendshapes() {
        return Err(format!(
            "the RigLogic chunk names {} blendshapes, the DNA rig has {}",
            logic.blendshapes(),
            blend.blendshapes()
        ));
    }
    Ok(targets_of(&blend, logic, mesh, selection))
}

/// The blendshapes of a head on the vertices of its geometry stream.
pub(super) trait BlendshapeSource: Sync {
    fn vertices(&self) -> usize;

    /// Returns one blendshape; the cost is a pass over all vertices of the stream, so callers run
    /// it per chosen blendshape, in parallel.
    fn target(&self, blendshape: usize) -> ExpressionTarget;
}

impl BlendshapeSource for ExpressionBlend<'_> {
    fn vertices(&self) -> usize {
        Self::vertices(self)
    }

    /// Costs a dense array of the stream's vertices and a pass over the cells of the heads.
    fn target(&self, blendshape: usize) -> ExpressionTarget {
        Self::target(self, blendshape)
    }
}

impl BlendshapeSource for SkinBlendshapes<'_> {
    fn vertices(&self) -> usize {
        Self::vertices(self)
    }

    /// Costs one pass over the bitmaps of all vertices of the stream.
    fn target(&self, blendshape: usize) -> ExpressionTarget {
        Self::target(self, blendshape)
    }
}

/// Makes the morph targets `selection` asks for of the head `mesh` from `source`, named by
/// `logic`; the targets ascend by blendshape and leave out the blendshapes that move no vertex.
pub(super) fn targets_of(
    source: &impl BlendshapeSource,
    logic: &RigLogic,
    mesh: &Mesh,
    selection: &Expressions,
) -> Morphs {
    let (chosen, unknown) = select(logic, selection);
    let (first, next) = by_stream_vertex(mesh, source.vertices());
    let made = make(source, &chosen, &first, &next);
    let empty = match selection {
        Expressions::Only(_) => made
            .iter()
            .zip(&chosen)
            .filter(|(target, _)| target.is_none())
            .map(|(_, (_, name))| name.clone())
            .collect(),
        _ => Vec::new(),
    };
    let numbers: Vec<usize> = made
        .iter()
        .zip(&chosen)
        .filter(|(target, _)| target.is_some())
        .map(|(_, (number, _))| *number)
        .collect();
    let mut targets: Vec<MorphTarget> = made.into_iter().flatten().collect();
    let mut seen = std::collections::HashMap::<String, usize>::new();
    for target in &mut targets {
        let count = seen.entry(target.name.clone()).or_insert(0);
        *count += 1;
        if *count > 1 {
            target.name = format!("{}#{count}", target.name);
        }
    }
    Morphs {
        targets,
        numbers,
        unknown,
        empty,
    }
}

fn make(
    blend: &impl BlendshapeSource,
    chosen: &[(usize, String)],
    first: &[u32],
    next: &[u32],
) -> Vec<Option<MorphTarget>> {
    chosen
        .par_iter()
        .map(|(number, name)| {
            let (vertices, deltas) = on_mesh(&blend.target(*number), first, next);
            (!vertices.is_empty()).then(|| MorphTarget {
                name: name.clone(),
                vertices,
                deltas,
            })
        })
        .collect()
}
