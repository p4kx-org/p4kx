// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use p4kx_extraction::{EntryOrigin, ModelArchiveSource};
use p4kx_model::{Lod, Model};
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};

use super::jobs::{Job, issue};
use crate::build::file_key;
use crate::{AssemblyError, AssemblyOptions, Issue, IssueKind, ModelFailure, ModelProvider};

/// The most distinct model files read at once before the vertex budget is checked again.
const WAVE: usize = 64;

/// A model read for a piece and the Entry it came from.
pub(super) struct Loaded {
    pub(super) entry: String,
    pub(super) model: Arc<Model>,
    pub(super) archive_sources: Arc<[ModelArchiveSource]>,
    pub(super) definition_origin: Option<EntryOrigin>,
}

pub(super) type Models = HashMap<String, Result<Loaded, ModelFailure>>;

fn vertices(model: &Model) -> usize {
    model.lods.iter().map(Lod::vertex_count).max().unwrap_or(0)
}

/// Reads every distinct file the jobs name, a wave of files at a time in parallel, and stops
/// when the files read so far already hold `options.max_vertices` vertices.
pub(super) fn read_models(
    jobs: &[Job],
    provider: &dyn ModelProvider,
    options: &AssemblyOptions,
) -> Result<Models, AssemblyError> {
    let mut seen = HashSet::new();
    let files: Vec<(String, &str)> = jobs
        .iter()
        .filter(|job| seen.insert(file_key(&job.path)))
        .map(|job| (file_key(&job.path), job.path.as_str()))
        .collect();
    let mut models = Models::new();
    let mut total = 0_usize;
    for wave in files.chunks(WAVE) {
        if total >= options.max_vertices {
            break;
        }
        let read: Vec<(String, Result<Loaded, ModelFailure>)> = wave
            .par_iter()
            .map(|(key, path)| {
                let result = provider.model(path, options.lod).map(|provided| Loaded {
                    entry: provided.entry,
                    model: Arc::new(provided.model),
                    archive_sources: provided.archive_sources,
                    definition_origin: provided.definition_origin,
                });
                (key.clone(), result)
            })
            .collect();
        for (key, result) in read {
            match result {
                Err(ModelFailure::Fatal(source)) => return Err(AssemblyError::Provider(source)),
                Ok(loaded) => {
                    total = total.saturating_add(vertices(&loaded.model));
                    models.insert(key, Ok(loaded));
                }
                Err(other) => {
                    models.insert(key, Err(other));
                }
            }
        }
    }
    Ok(models)
}

/// Picks the model of each job in order, within the vertex budget, and records why a job has none.
pub(super) fn choose<'m>(
    jobs: &[Job],
    models: &'m Models,
    max_vertices: usize,
    issues: &mut Vec<Issue>,
) -> Vec<Option<&'m Loaded>> {
    let mut total = 0_usize;
    jobs.iter()
        .map(|job| {
            let path = job.path.clone();
            let kind = match models.get(&file_key(&job.path)) {
                Some(Ok(loaded)) => {
                    let count = vertices(&loaded.model);
                    if total.saturating_add(count) <= max_vertices {
                        total += count;
                        return Some(loaded);
                    }
                    IssueKind::TooManyVertices {
                        limit: max_vertices,
                    }
                }
                Some(Err(ModelFailure::Missing)) => IssueKind::GeometryMissing { path },
                Some(Err(other)) => IssueKind::GeometryUnreadable {
                    path,
                    reason: other.to_string(),
                },
                None => IssueKind::TooManyVertices {
                    limit: max_vertices,
                },
            };
            issues.push(issue(job, kind));
            None
        })
        .collect()
}
