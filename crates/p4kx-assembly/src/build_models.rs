// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use p4kx_model::{Lod, Model};
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};

use crate::build::{file_key, issue};
use crate::flatten::Draft;
use crate::geometry_choice::GeometryChoice;
use crate::{
    AssemblyError, AssemblyOptions, Issue, IssueKind, ModelFailure, ModelProvider,
    ProvidedAttachment, SkeletonProblem,
};

/// The most distinct model files read at once before the vertex budget is checked again; a wave
/// is twice the number of threads, at most this many.
const WAVE: usize = 256;

/// A model read for the Assembly and the file it came from.
pub(crate) struct Loaded {
    pub(crate) entry: String,
    pub(crate) model: Arc<Model>,
    pub(crate) left_out: Vec<String>,
    pub(crate) attachments: Vec<ProvidedAttachment>,
    pub(crate) skeleton: Option<SkeletonProblem>,
}

impl Loaded {
    /// Returns the vertices the part holds: its model's and those of its bone attachments.
    pub(crate) fn vertices(&self) -> usize {
        self.attachments
            .iter()
            .fold(vertices(&self.model), |sum, attached| {
                sum.saturating_add(vertices(&attached.model))
            })
    }
}

/// What reading one distinct geometry file came to.
pub(crate) enum Outcome {
    Loaded(Loaded),
    Failed { missing: bool, reason: String },
}

pub(crate) type Models = HashMap<String, Outcome>;

pub(crate) fn vertices(model: &Model) -> usize {
    model.lods.iter().map(Lod::vertex_count).max().unwrap_or(0)
}

/// Reads every distinct geometry file the parts name, in file order a wave of files at a time in
/// parallel, and stops after the wave in which the distinct models read hold
/// `options.max_vertices` vertices. The files come in the order of their first part, so a file left
/// unread belongs to a part that [`choose_models`] refuses anyway, whatever the size of the wave;
/// the budget is overshot by one wave at most.
pub(crate) fn read_models(
    drafts: &[Draft<'_>],
    provider: &dyn ModelProvider,
    options: &AssemblyOptions,
) -> Result<Models, AssemblyError> {
    let mut seen = HashSet::new();
    let files: Vec<(String, &str)> = drafts
        .iter()
        .filter(|draft| !draft.hidden)
        .filter_map(|draft| Some((draft.model_key()?, draft.geometry_path()?)))
        .filter(|(key, _)| seen.insert(key.clone()))
        .collect();
    let mut models = Models::new();
    read_files(&files, provider, options, &mut models, 0)?;
    Ok(models)
}

/// Gives a part whose chosen sub-geometry is not in the Archive the main geometry of its Entity
/// and notes the missing file, except for the own skin of a head, which the Archive need not
/// have; the main models are read once each, after the others.
pub(crate) fn fall_back<'t>(
    drafts: &mut [Draft<'t>],
    models: &mut Models,
    provider: &dyn ModelProvider,
    options: &AssemblyOptions,
    issues: &mut Vec<Issue>,
) -> Result<(), AssemblyError> {
    let mut seen = HashSet::new();
    let mut files: Vec<(String, &'t str)> = Vec::new();
    for draft in drafts.iter_mut() {
        let (Some(chosen), Some(main)) = (draft.geometry, draft.node.fallback.as_ref()) else {
            continue;
        };
        if draft.hidden {
            continue;
        }
        if !matches!(
            models.get(&file_key(chosen.path())),
            Some(Outcome::Failed { missing: true, .. })
        ) {
            continue;
        }
        let path = chosen.path().to_owned();
        draft.geometry = Some(main);
        if draft.node.choice != GeometryChoice::Bespoke {
            issues.push(issue(draft, IssueKind::SubGeometryMissing { path }));
        }
        let key = file_key(main.path());
        if !models.contains_key(&key) && seen.insert(key.clone()) {
            files.push((key, main.path()));
        }
    }
    let read = models
        .values()
        .map(|outcome| match outcome {
            Outcome::Loaded(loaded) => loaded.vertices(),
            Outcome::Failed { .. } => 0,
        })
        .fold(0_usize, usize::saturating_add);
    read_files(&files, provider, options, models, read)
}

fn read_files(
    files: &[(String, &str)],
    provider: &dyn ModelProvider,
    options: &AssemblyOptions,
    models: &mut Models,
    already_read: usize,
) -> Result<(), AssemblyError> {
    let lod = options.lod;
    let mut read_vertices = already_read;
    let wave_size = rayon::current_num_threads()
        .saturating_mul(2)
        .clamp(1, WAVE);
    for wave in files.chunks(wave_size) {
        if read_vertices >= options.max_vertices {
            break;
        }
        let read: Vec<(String, Result<Loaded, ModelFailure>)> = wave
            .par_iter()
            .map(|(key, path)| {
                let result = provider.model(path, lod).map(|provided| Loaded {
                    entry: provided.entry,
                    model: Arc::new(provided.model),
                    left_out: provided.left_out,
                    attachments: provided.attachments,
                    skeleton: provided.skeleton,
                });
                (key.clone(), result)
            })
            .collect();
        for (key, result) in read {
            if let Ok(loaded) = &result {
                read_vertices = read_vertices.saturating_add(loaded.vertices());
            }
            let outcome = match result {
                Ok(loaded) => Outcome::Loaded(loaded),
                Err(ModelFailure::Missing) => Outcome::Failed {
                    missing: true,
                    reason: String::new(),
                },
                Err(ModelFailure::Fatal(source)) => return Err(AssemblyError::Provider(source)),
                Err(other) => Outcome::Failed {
                    missing: false,
                    reason: other.to_string(),
                },
            };
            models.insert(key, outcome);
        }
    }
    Ok(())
}

/// Picks the model of each part in order, within the vertex budget, and records why a part has
/// none.
pub(crate) fn choose_models<'m>(
    drafts: &[Draft<'_>],
    models: &'m Models,
    max_vertices: usize,
    issues: &mut Vec<Issue>,
) -> Vec<Option<&'m Loaded>> {
    let mut total = 0_usize;
    drafts
        .iter()
        .map(|draft| {
            if draft.hidden {
                return None;
            }
            let Some(path) = draft.geometry_path() else {
                let kind = match (draft.node.choice, draft.node.fallback.as_ref()) {
                    (GeometryChoice::WorldProp, Some(prop)) => IssueKind::WorldPropNotWorn {
                        path: prop.path().to_owned(),
                    },
                    _ => IssueKind::NoGeometry,
                };
                issues.push(issue(draft, kind));
                return None;
            };
            let path = path.to_owned();
            match draft.model_key().and_then(|key| models.get(&key)) {
                Some(Outcome::Loaded(loaded)) => {
                    let count = loaded.vertices();
                    if total.saturating_add(count) > max_vertices {
                        let limit = max_vertices;
                        issues.push(issue(draft, IssueKind::TooManyVertices { limit }));
                        None
                    } else {
                        total += count;
                        Some(loaded)
                    }
                }
                Some(Outcome::Failed { missing: true, .. }) => {
                    let kind = match draft.node.choice {
                        GeometryChoice::Body(body) => IssueKind::BodyGeometryMissing {
                            body: body.code().to_owned(),
                            path,
                        },
                        _ => IssueKind::GeometryMissing { path },
                    };
                    issues.push(issue(draft, kind));
                    None
                }
                Some(Outcome::Failed { reason, .. }) => {
                    let reason = reason.clone();
                    issues.push(issue(draft, IssueKind::GeometryUnreadable { path, reason }));
                    None
                }
                None => {
                    let limit = max_vertices;
                    issues.push(issue(draft, IssueKind::TooManyVertices { limit }));
                    None
                }
            }
        })
        .collect()
}
