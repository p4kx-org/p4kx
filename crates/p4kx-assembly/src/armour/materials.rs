// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The material libraries the pieces of an Armour are drawn with.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use p4kx_extraction::{ArchiveMaterialRead, MaterialArchiveOrigins};
use p4kx_model::MaterialStatus;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};

use super::Look;
use super::jobs::{Job, issue};
use super::read_models::Loaded;
use crate::build::library_key_of;
use crate::build_palettes::Palettes;
use crate::{Issue, IssueKind, ModelProvider};

#[derive(Clone)]
pub(super) struct Library {
    pub(super) status: Arc<MaterialStatus>,
    pub(super) origins: Arc<MaterialArchiveOrigins>,
}

impl From<ArchiveMaterialRead> for Library {
    fn from(read: ArchiveMaterialRead) -> Self {
        Self {
            status: Arc::new(read.status),
            origins: Arc::new(read.origins),
        }
    }
}

/// The material libraries read for the jobs, by the key they are cached under.
pub(super) type Libraries = HashMap<String, Library>;

/// Returns the materials a job is drawn with: its library, with the choices of its Character
/// applied and the tint palette of its geometry. A library that cannot be read is listed once.
pub(super) fn drawn_with(
    job: &Job,
    found: &Loaded,
    (libraries, unnamed): (&Libraries, &Library),
    (reported, tints): (&mut HashSet<String>, &mut Palettes<'_>),
    issues: &mut Vec<Issue>,
) -> Library {
    let key = library_key(job, found);
    let mut library = key
        .as_ref()
        .and_then(|key| libraries.get(key))
        .unwrap_or(unnamed)
        .clone();
    if let (Some(key), Some(reason)) = (
        key,
        library
            .status
            .problem()
            .or_else(|| library.status.layer_problem()),
    ) && reported.insert(key)
    {
        issues.push(issue(job, IssueKind::MaterialLibrary { reason }));
    }
    let painted = painted(tints, library.status, job, issues);
    let natural = Look::natural();
    library.status = job
        .look
        .as_ref()
        .unwrap_or(&natural)
        .apply(&painted)
        .map_or(painted, Arc::new);
    library
}

/// Returns `materials` drawn with the tint palette of the job's geometry, else the default
/// palette of their library, so that layers with a `PaletteTint` take the palette's entries;
/// unchanged, and no palette resolved, when no material takes one.
fn painted(
    tints: &mut Palettes<'_>,
    materials: Arc<MaterialStatus>,
    job: &Job,
    issues: &mut Vec<Issue>,
) -> Arc<MaterialStatus> {
    let Some(library) = materials
        .library()
        .filter(|library| library.materials.iter().any(|m| m.palette.is_some()))
    else {
        return materials;
    };
    let (palette, unresolved) = tints.of_named(job.palette.as_ref(), &materials);
    issues.extend(unresolved.into_iter().map(|kind| issue(job, kind)));
    let Some(palette) = palette else {
        return materials;
    };
    let mut library = library.clone();
    for material in &mut library.materials {
        if let Some(tinted) = palette.apply(material) {
            *material = tinted;
        }
    }
    Arc::new(MaterialStatus::Loaded(library))
}

/// Returns the key under which the library of a job is cached.
fn library_key(job: &Job, loaded: &Loaded) -> Option<String> {
    Some(library_key_of(library_name(job, loaded)?, &loaded.entry))
}

fn library_name<'a>(job: &'a Job, loaded: &'a Loaded) -> Option<&'a str> {
    job.material
        .as_deref()
        .or(loaded.model.material_library.as_deref())
}

/// Reads every distinct material library the jobs draw with once, in parallel.
pub(super) fn read_libraries(
    jobs: &[Job],
    chosen: &[Option<&Loaded>],
    provider: &dyn ModelProvider,
) -> Libraries {
    let mut seen = HashSet::new();
    let wanted: Vec<(String, &str, &str)> = jobs
        .iter()
        .zip(chosen)
        .filter_map(|(job, loaded)| {
            let loaded = (*loaded)?;
            let name = library_name(job, loaded)?;
            let key = library_key(job, loaded)?;
            seen.insert(key.clone())
                .then_some((key, loaded.entry.as_str(), name))
        })
        .collect();
    wanted
        .par_iter()
        .map(|(key, entry, name)| {
            (
                key.clone(),
                Library::from(provider.library_observed(entry, name)),
            )
        })
        .collect()
}
