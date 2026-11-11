// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The material libraries the pieces of an Armour are drawn with.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;

use p4kx_extraction::{ArchiveMaterialRead, MaterialArchiveOrigins, SourceLocation};
use p4kx_model::MaterialStatus;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};

use super::Look;
use super::jobs::{Job, issue};
use super::read_models::Loaded;
use crate::build::library_key_of;
use crate::build_palettes::Palettes;
use crate::{Issue, IssueKind, ModelProvider, PaletteRecords};

#[derive(Clone)]
pub(super) struct Library {
    pub(super) status: Arc<MaterialStatus>,
    pub(super) origins: Arc<MaterialArchiveOrigins>,
    pub(super) palette: Option<PaletteApplication>,
    pub(super) look_sources: BTreeMap<usize, Vec<SourceLocation>>,
}

impl From<ArchiveMaterialRead> for Library {
    fn from(read: ArchiveMaterialRead) -> Self {
        Self {
            status: Arc::new(read.status),
            origins: Arc::new(read.origins),
            palette: None,
            look_sources: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct PaletteApplication {
    records: PaletteRecords,
    slots: Vec<usize>,
}

impl PaletteApplication {
    pub(super) fn records_for(&self, slot: usize) -> Option<PaletteRecords> {
        self.slots
            .binary_search(&slot)
            .is_ok()
            .then_some(self.records)
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
    let (painted, palette) = painted(tints, library.status, job, issues);
    library.palette = palette;
    let natural = Look::natural();
    if let Some(applied) = job
        .look
        .as_ref()
        .unwrap_or(&natural)
        .apply_observed(&painted)
    {
        library.status = Arc::new(applied.status);
        library.look_sources = applied.fields;
    } else {
        library.status = painted;
    }
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
) -> (Arc<MaterialStatus>, Option<PaletteApplication>) {
    let Some(library) = materials
        .library()
        .filter(|library| library.materials.iter().any(|m| m.palette.is_some()))
    else {
        return (materials, None);
    };
    let (palette, unresolved) = tints.of_named_observed(job.palette.as_ref(), &materials);
    issues.extend(unresolved.into_iter().map(|kind| issue(job, kind)));
    let Some(palette) = palette else {
        return (materials, None);
    };
    let mut library = library.clone();
    let mut slots = Vec::new();
    for (slot, material) in library.materials.iter_mut().enumerate() {
        if let Some(tinted) = palette.palette.apply(material) {
            slots.push(slot);
            *material = tinted;
        }
    }
    let applied = (!slots.is_empty()).then_some(PaletteApplication {
        records: palette.records,
        slots,
    });
    (Arc::new(MaterialStatus::Loaded(library)), applied)
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
