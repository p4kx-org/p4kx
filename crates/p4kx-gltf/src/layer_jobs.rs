// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;

use p4kx_model::Material;

use crate::layer_plan::{Layered, layered_of};
use crate::texture_assemble::Wish;
use crate::texture_pixels::Tints;
use crate::texture_plan::{Job, Key};
use crate::texture_report::TextureRole;
use crate::texture_roles::{Source, Sources};

type Numbered = (usize, Job);

/// Puts the normal images of layered materials after every other job, and after them the plain
/// normal images only a fallback uses, so that when the budget of images runs out, every
/// material has its colours before a layered one has its normals; returns the jobs with the
/// numbers in `wanted` following them.
pub(crate) fn layer_normals_last(jobs: Vec<Job>, wanted: &mut [Wish]) -> (Vec<Job>, Vec<usize>) {
    let primary: HashSet<usize> = wanted
        .iter()
        .flat_map(|wish| wish.jobs.into_iter().flatten())
        .collect();
    let fallbacks: HashSet<usize> = wanted
        .iter()
        .filter_map(|wish| wish.fallback)
        .filter(|job| !primary.contains(job))
        .collect();
    let rank = |at: usize, job: &Job| {
        if matches!(job.key, Key::LayerNormal { .. }) {
            1
        } else {
            u8::from(fallbacks.contains(&at)) * 2
        }
    };
    let mut numbered: Vec<(u8, Numbered)> = jobs
        .into_iter()
        .enumerate()
        .map(|(at, job)| (rank(at, &job), (at, job)))
        .collect();
    numbered.sort_by_key(|(rank, _)| *rank);
    let mut moved = vec![0; numbered.len()];
    for (new, (_, (old, _))) in numbered.iter().enumerate() {
        if let Some(target) = moved.get_mut(*old) {
            *target = new;
        }
    }
    for wish in wanted {
        for job in wish
            .jobs
            .iter_mut()
            .flatten()
            .chain(wish.fallback.iter_mut())
        {
            *job = moved.get(*job).copied().unwrap_or(*job);
        }
    }
    numbered
        .into_iter()
        .map(|(_, (old, job))| (job, old))
        .unzip()
}

/// Returns for each of `len` jobs the layered normal jobs of the materials that use it only as
/// their fallback; the job is unneeded once all of them made an image. Empty for a job that some
/// material draws as its own normal image.
pub(crate) fn fallback_guards(wanted: &[Wish], len: usize) -> Vec<Vec<usize>> {
    let primary: HashSet<usize> = wanted
        .iter()
        .flat_map(|wish| wish.jobs.into_iter().flatten())
        .collect();
    let mut guards = vec![Vec::new(); len];
    for wish in wanted {
        let [_, Some(layered), _] = wish.jobs else {
            continue;
        };
        if let Some(fallback) = wish.fallback.filter(|job| !primary.contains(job))
            && let Some(guard) = guards.get_mut(fallback)
            && !guard.contains(&layered)
        {
            guard.push(layered);
        }
    }
    guards
}

/// Returns the job of the base colour image of `layered`, or when `normal` is set its normal
/// image, to which `own`, the material's own normal map, adds its slopes.
pub(crate) fn layer_job(
    layered: &Layered,
    normal: bool,
    own: Option<&Source>,
    tints: Tints,
) -> Job {
    let key = if normal {
        Key::LayerNormal {
            blend: layered.fingerprint(true),
            own: own.map(Source::identity),
        }
    } else {
        Key::LayerBase {
            blend: layered.fingerprint(false),
        }
    };
    Job {
        key,
        diffuse: None,
        normal: own.cloned(),
        specular: None,
        tints,
        layered: Some(layered.clone()),
    }
}

/// The jobs of the base colour and the normal image of a layered material, and the texture files
/// its layers name that have no safe path.
pub(crate) type LayerJobs = (Option<Job>, Option<Job>, Vec<(TextureRole, String)>);

/// Returns the jobs of the base colour and the normal image that the layers of `material` give
/// when it has no diffuse map of its own and a layer has a diffuse or a normal map.
pub(crate) fn layer_jobs(material: &Material, found: &Sources, tints: Tints) -> LayerJobs {
    let Some(layered) = found
        .diffuse
        .is_none()
        .then(|| layered_of(material))
        .flatten()
    else {
        return (None, None, Vec::new());
    };
    (
        layered
            .has_diffuse()
            .then(|| layer_job(&layered, false, None, tints)),
        layered
            .has_normal()
            .then(|| layer_job(&layered, true, found.normal.as_ref(), tints)),
        layered.refused,
    )
}
