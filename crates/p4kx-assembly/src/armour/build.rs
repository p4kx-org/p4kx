// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;
use std::sync::Arc;

use p4kx_gltf::MorphTarget;
use p4kx_model::{MaterialStatus, Model};

use super::face_build::{Face, Found, settled};
use super::head_move::{HeadMove, Worn, check_follows, first_mesh_note, followed};
use super::jobs::{Job, issue, planned};
use super::materials::{drawn_with, read_libraries};
use super::own_face::own_head;
use super::read_models::{Loaded, choose, read_models};
use super::shape::shaped;
use super::zones::{Coverage, without_covered};
use super::{Armour, ArmourPart, BodyVariant, Piece};
use crate::build_palettes::Palettes;
use crate::{
    AssemblyError, AssemblyOptions, Issue, IssueKind, ModelFailure, ModelProvider, PaletteProvider,
    ProvidedModel,
};

/// The name of the piece the pieces that follow the head follow.
const HEAD: &str = "head";

struct Body {
    skeleton: Model,
    skeleton_file: String,
}

impl Armour {
    /// Builds the Armour of `pieces` worn on the default body of `variant`.
    ///
    /// The body is the skeleton of the variant and its default skin, see [`BodyVariant`]. Each
    /// piece contributes the geometry its Record names for the variant's tag, see
    /// [`Wearable`](crate::Wearable). The models are read at the level of detail in `options`,
    /// each distinct file once and in parallel, and the material library the Record names
    /// replaces the one a model names. A piece that cannot be fitted to the variant, or whose
    /// model cannot be read, is left out and the [`Issue`] says why; the rest is still built.
    /// `options.max_parts` and `options.max_vertices` limit the parts and their vertices.
    ///
    /// # Errors
    ///
    /// Returns [`AssemblyError::Body`] when the skeleton of the variant cannot be loaded and
    /// [`AssemblyError::Provider`] when `models` fails for every file, e.g. because an Archive key
    /// is missing.
    pub fn build(
        variant: BodyVariant,
        pieces: &[Piece],
        models: &dyn ModelProvider,
        options: &AssemblyOptions,
    ) -> Result<Self, AssemblyError> {
        Self::build_painted(variant, pieces, models, None, options)
    }

    /// Builds the Armour as [`Self::build`] does, with the materials of each piece drawn with the
    /// tint palette its worn geometry names, else the default palette of its material library,
    /// which `palettes` reads. A piece keeps its own palette and takes none from elsewhere. A
    /// palette that cannot be resolved is listed as an [`Issue`] and the piece is drawn without it.
    ///
    /// # Errors
    ///
    /// As [`Self::build`].
    pub fn build_with_palettes(
        variant: BodyVariant,
        pieces: &[Piece],
        models: &dyn ModelProvider,
        palettes: &dyn PaletteProvider,
        options: &AssemblyOptions,
    ) -> Result<Self, AssemblyError> {
        Self::build_painted(variant, pieces, models, Some(palettes), options)
    }

    fn build_painted(
        variant: BodyVariant,
        pieces: &[Piece],
        models: &dyn ModelProvider,
        palettes: Option<&dyn PaletteProvider>,
        options: &AssemblyOptions,
    ) -> Result<Self, AssemblyError> {
        let body = load_body(variant, models, options.lod)?;
        let mut issues = Vec::new();
        let jobs = planned(variant, pieces, models, options.max_parts, &mut issues);
        let loaded = read_models(&jobs, models, options)?;
        let chosen = choose(&jobs, &loaded, options.max_vertices, &mut issues);
        let libraries = read_libraries(&jobs, &chosen, models);
        let unnamed = Arc::new(MaterialStatus::Unnamed);
        let covering = covering(&jobs, &chosen, variant);
        let (mut reported, mut tints) = (HashSet::new(), Palettes::new(palettes));
        let mut parts = Vec::with_capacity(chosen.len());
        let mut head: Option<HeadMove> = None;
        let mut heads = Vec::new();
        let mut part_jobs = Vec::new();
        for (job, found) in jobs.iter().zip(&chosen) {
            let Some(found) = found else { continue };
            let Some(mut dressed) = worn(job, found, models, &mut head, &mut issues) else {
                continue;
            };
            let model = without_covered(job, dressed.model, variant, &covering, &mut issues);
            let face = dressed.face.take();
            if job.bespoke {
                heads.push(Found::bespoke(parts.len(), face));
            } else {
                heads.extend(Found::of(parts.len(), job.shape.as_ref(), face));
            }
            part_jobs.push(job);
            let materials = drawn_with(
                job,
                found,
                (&libraries, &unnamed),
                (&mut reported, &mut tints),
                &mut issues,
            );
            parts.push(part_of(
                job,
                (model, materials),
                dressed.morphs,
                dressed.follows,
            ));
        }
        let skeleton = settled(
            body.skeleton,
            &mut parts,
            heads,
            &part_jobs,
            options.lod,
            &mut issues,
        );
        check_follows(&mut parts, &mut issues);
        Ok(Self {
            variant,
            skeleton: Arc::new(skeleton),
            skeleton_file: body.skeleton_file,
            parts,
            issues,
            lod: options.lod,
        })
    }
}

/// Returns the part a job is worn as, with its model and materials, the morph targets it was built
/// with and the vertices that follow the head.
fn part_of(
    job: &Job,
    (model, materials): (Arc<Model>, Arc<MaterialStatus>),
    morphs: Vec<MorphTarget>,
    follows: Vec<usize>,
) -> ArmourPart {
    ArmourPart {
        name: job.part.clone(),
        origin: job.origin,
        file: job.path.clone(),
        model,
        materials,
        morphs,
        follows,
        face: None,
        mount: None,
    }
}

/// Returns the body zones the worn clothing of the jobs that have a model covers.
fn covering<'j>(jobs: &'j [Job], chosen: &[Option<&Loaded>], variant: BodyVariant) -> Coverage<'j> {
    Coverage::of(
        jobs.iter()
            .zip(chosen)
            .filter(|(_, found)| found.is_some())
            .flat_map(|(job, _)| job.zones.iter()),
        variant,
    )
}

/// Returns the model of a job as it is worn: shaped by the job's DNA, if it has one, and following
/// the shaped head, if it follows it, with its morph targets; what follows the head has the
/// targets of the head moved to its own vertices. The first shaped head is remembered in `head`,
/// as it is shaped and with its morph targets.
fn worn(
    job: &Job,
    found: &Loaded,
    models: &dyn ModelProvider,
    head: &mut Option<HeadMove>,
    issues: &mut Vec<Issue>,
) -> Option<Worn> {
    let (model, morphs, face) = fitted(job, found, models, issues)?;
    if job.shape.is_some() && job.part == HEAD && head.is_none() {
        *head = Some(HeadMove {
            before: Arc::clone(&found.model),
            after: Arc::clone(&model),
            morphs: morphs.clone(),
        });
    }
    Some(match head {
        Some(moved) if job.follows_head => {
            let mut dressed = followed(&model, moved);
            issues.extend(dressed.kinds.drain(..).map(|kind| issue(job, kind)));
            if let Some(kind) = first_mesh_note(&dressed) {
                issues.push(issue(job, kind));
            }
            dressed
        }
        _ => Worn {
            model,
            morphs,
            face,
            follows: Vec::new(),
            kinds: Vec::new(),
        },
    })
}

/// Returns the model of a job as it is worn, shaped by the job's DNA if it has one, and its morph
/// targets; a model that cannot be shaped is listed and left out, morph targets that cannot be
/// made are listed.
fn fitted(
    job: &Job,
    found: &Loaded,
    models: &dyn ModelProvider,
    issues: &mut Vec<Issue>,
) -> Option<(Arc<Model>, Vec<MorphTarget>, Option<Face>)> {
    let Some(dna) = &job.shape else {
        return Some(unshaped(job, found, models, issues));
    };
    let head = job.part == HEAD;
    let shaped = match shaped(&found.model, &job.path, dna, &job.expressions, models, head) {
        Ok(shaped) => shaped,
        Err(reason) => {
            issues.push(issue(job, IssueKind::ShapeFailed { reason }));
            return None;
        }
    };
    let worn = shaped.into_parts();
    issues.extend(worn.kinds.into_iter().map(|kind| issue(job, kind)));
    let face = head.then(|| Face::made(worn.face, worn.numbers));
    Some((Arc::new(worn.model), worn.targets, face))
}

/// Returns the model of a job without a DNA as it is worn: unchanged. The head of a story
/// character with a `.RigLogic` file of its own also has the face of that rig: its joints and
/// the morph targets the job asks for.
fn unshaped(
    job: &Job,
    found: &Loaded,
    models: &dyn ModelProvider,
    issues: &mut Vec<Issue>,
) -> (Arc<Model>, Vec<MorphTarget>, Option<Face>) {
    let own = (job.bespoke && job.part == HEAD)
        .then(|| own_head(&found.model, &job.path, &job.expressions, models))
        .flatten();
    let Some(made) = own else {
        return (Arc::clone(&found.model), Vec::new(), None);
    };
    issues.extend(made.kinds.into_iter().map(|kind| issue(job, kind)));
    (
        Arc::clone(&found.model),
        made.targets,
        Some(Face::made(made.face, made.numbers)),
    )
}

fn provider_failure(path: &str, variant: BodyVariant, failure: ModelFailure) -> AssemblyError {
    match failure {
        ModelFailure::Fatal(source) => AssemblyError::Provider(source),
        other => AssemblyError::Body {
            variant: variant.code().to_owned(),
            path: path.to_owned(),
            reason: other.to_string(),
        },
    }
}

fn load_body(
    variant: BodyVariant,
    models: &dyn ModelProvider,
    lod: u32,
) -> Result<Body, AssemblyError> {
    let skeleton_file = variant.skeleton();
    let ProvidedModel { model, .. } = models
        .model(skeleton_file, lod)
        .map_err(|failure| provider_failure(skeleton_file, variant, failure))?;
    Ok(Body {
        skeleton: model,
        skeleton_file: skeleton_file.to_owned(),
    })
}
