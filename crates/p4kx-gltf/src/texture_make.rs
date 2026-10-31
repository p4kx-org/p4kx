// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_texture::{RgbaImage, write_png};
use rayon::prelude::*;

use crate::layer_make::{make_base as make_base_layers, make_normal as make_normal_layers};
use crate::texture_budget::{Tally, Verdict};
use crate::texture_cache::{Decoded, MAX_HELD_PIXELS, SourceStore};
use crate::texture_loader::{LoadedTexture, TextureLoadError, TextureLoader};
use crate::texture_pixels::{base_colour, metallic_roughness, normal_map, opacity_colour};
use crate::texture_plan::{EncodedImage, Failure, Job, Key, MAX_TEXTURE_IMAGES};
use crate::texture_report::TextureRole;
use crate::texture_roles::Source;

pub(crate) use crate::texture_budget::Budget;

#[cfg(test)]
mod tests;

const PARALLEL_IMAGES: usize = 4;
const TOO_LARGE: &str = "the textures of this layered image would decode more than 2^27 pixels together; use a higher --texture-mip";
const OVER_BUDGET: &str =
    "the images of this export would exceed the budget; use a higher --texture-mip";

/// What making one image came to.
#[derive(Default)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "each flag says one thing about the image that the glTF material needs"
)]
pub(crate) struct Outcome {
    pub(crate) image: Option<EncodedImage>,
    /// For a base colour image: whether the material's colours are in it.
    pub(crate) tinted: bool,
    /// For a base colour image: whether it is blended of the layers of a layered material.
    pub(crate) layered: bool,
    /// For a metallic-roughness image: whether it carries the roughness.
    pub(crate) roughness: bool,
    /// For a metallic-roughness image: whether it carries the metalness.
    pub(crate) metallic: bool,
    pub(crate) failures: Vec<Failure>,
}

impl Outcome {
    fn pixels(&self) -> Option<u64> {
        self.image
            .as_ref()
            .map(|image| u64::from(image.width) * u64::from(image.height))
    }
}

/// Makes the image of every job, a few at a time to bound the memory of decoded textures; an
/// image that would exceed `budget` is dropped and its job fails, like every job after it.
///
/// The budget is checked against the header sizes before a job decodes anything, when the loader
/// gives them, and against the image made otherwise. The sources of a group of jobs are decoded
/// once however many of its jobs use them, and those the next group needs stay in memory up to a
/// pixel cap. A group is at most four jobs, fewer when the distinct sources of the jobs would
/// decode more than the decode limit together, by their headers; a source whose size is unknown
/// or unreadable counts for the whole limit. The sources in memory therefore never exceed the
/// decode limit plus the held cap, whatever the loader allows per texture, unless one job alone
/// needs more.
///
/// `guards[n]` lists the jobs, all numbered below `n`, whose images make job `n` unneeded: job
/// `n` is skipped, taking no image slot and no decode, when every one of them made an image. A
/// group ends before a guarded job whose guards are still to be made.
pub(crate) fn run_guarded(
    jobs: &[Job],
    mip: u32,
    loader: &dyn TextureLoader,
    budget: Budget,
    guards: &[Vec<usize>],
) -> Vec<Outcome> {
    let store = SourceStore::new(loader, mip, MAX_HELD_PIXELS);
    run_with_guards(jobs, store, budget, guards)
}

fn guarded_len(guards: &[Vec<usize>], start: usize, len: usize) -> usize {
    (1..len)
        .find(|at| {
            guards
                .get(start + at)
                .is_some_and(|guard| guard.iter().any(|job| *job >= start))
        })
        .unwrap_or(len)
}

#[cfg(test)]
pub(crate) fn run(
    jobs: &[Job],
    mip: u32,
    loader: &dyn TextureLoader,
    budget: Budget,
) -> Vec<Outcome> {
    run_guarded(jobs, mip, loader, budget, &[])
}

#[cfg(test)]
fn run_with(jobs: &[Job], store: SourceStore<'_>, budget: Budget) -> Vec<Outcome> {
    run_with_guards(jobs, store, budget, &[])
}

fn unneeded(guard: Option<&Vec<usize>>, outcomes: &[Outcome]) -> bool {
    guard.is_some_and(|guard| {
        !guard.is_empty()
            && guard
                .iter()
                .all(|job| outcomes.get(*job).is_some_and(|o| o.image.is_some()))
    })
}

fn settle(job: &Job, outcome: Outcome, tally: &mut Tally) -> Outcome {
    match outcome.pixels() {
        Some(size) if !tally.fits(size) => refused(
            job,
            if tally.images_spent() {
                spent_reason()
            } else {
                OVER_BUDGET.to_owned()
            },
        ),
        Some(size) => {
            tally.take(size);
            outcome
        }
        None => outcome,
    }
}

fn run_with_guards(
    jobs: &[Job],
    mut store: SourceStore<'_>,
    budget: Budget,
    guards: &[Vec<usize>],
) -> Vec<Outcome> {
    let mut tally = Tally::new(budget);
    let mut outcomes: Vec<Outcome> = Vec::with_capacity(jobs.len());
    let mut rest = jobs;
    while !rest.is_empty() {
        let start = outcomes.len();
        let len = guarded_len(guards, start, store.group_len(rest, PARALLEL_IMAGES));
        let (chunk, after) = rest.split_at(len);
        let next = after.get(..PARALLEL_IMAGES).unwrap_or(after);
        rest = after;
        let verdicts: Vec<Verdict> = chunk
            .iter()
            .enumerate()
            .map(|(at, job)| {
                if unneeded(guards.get(start + at), &outcomes) {
                    Verdict::Skipped
                } else {
                    tally.plan(job, &mut store)
                }
            })
            .collect();
        let decoded = store.prepare(
            chunk
                .iter()
                .zip(&verdicts)
                .filter(|(_, verdict)| verdict.runs())
                .map(|(job, _)| job),
        );
        let made: Vec<Option<Outcome>> = chunk
            .par_iter()
            .zip(&verdicts)
            .map(|(job, verdict)| verdict.runs().then(|| make(job, &decoded)))
            .collect();
        store.keep(decoded, next);
        for verdict in &verdicts {
            if let Verdict::Reserved(size) = verdict {
                tally.give_back(*size);
            }
        }
        for ((job, verdict), outcome) in chunk.iter().zip(verdicts).zip(made) {
            let outcome = match (outcome, verdict) {
                (Some(outcome), _) => outcome,
                (None, Verdict::Failed(failure)) => Outcome {
                    failures: vec![failure],
                    ..Outcome::default()
                },
                (None, Verdict::Skipped) => Outcome::default(),
                (None, Verdict::Spent) => refused(job, spent_reason()),
                (None, Verdict::TooLarge) => refused(job, TOO_LARGE.to_owned()),
                (None, _) => refused(job, OVER_BUDGET.to_owned()),
            };
            outcomes.push(settle(job, outcome, &mut tally));
        }
    }
    outcomes
}

fn spent_reason() -> String {
    format!(
        "the images of this export are as many as allowed ({MAX_TEXTURE_IMAGES}), so this one is left out"
    )
}

fn refused(job: &Job, reason: String) -> Outcome {
    Outcome {
        failures: job
            .primary()
            .map(|(role, source)| Failure {
                role,
                file: job.file_of(source),
                reason,
            })
            .into_iter()
            .collect(),
        ..Outcome::default()
    }
}

pub(crate) fn load(
    decoded: &Decoded,
    role: TextureRole,
    source: Option<&Source>,
    failures: &mut Vec<Failure>,
) -> Option<Arc<LoadedTexture>> {
    let source = source?;
    let loaded = decoded.get(&source.path).cloned().unwrap_or_else(|| {
        Err(TextureLoadError::Unreadable(
            "the texture was not prepared".to_owned(),
        ))
    });
    match loaded {
        Ok(texture) => Some(texture),
        Err(error) => {
            failures.push(Failure {
                role,
                file: source.file.clone(),
                reason: error.to_string(),
            });
            None
        }
    }
}

pub(crate) fn encode(image: &RgbaImage) -> Result<EncodedImage, String> {
    let mut png = Vec::new();
    write_png(image, &mut png).map_err(|error| error.to_string())?;
    Ok(EncodedImage {
        png,
        width: image.width(),
        height: image.height(),
    })
}

/// The image a job made, or why it could not be made; `None` when there was nothing to make it
/// of, because the textures failed to load, which `failures` records.
pub(crate) type Made = Option<Result<EncodedImage, String>>;

fn make(job: &Job, decoded: &Decoded) -> Outcome {
    let mut outcome = Outcome::default();
    let mut failures = Vec::new();
    let made = match &job.key {
        Key::Base { keep_alpha, .. } => {
            make_base(job, *keep_alpha, decoded, &mut outcome, &mut failures)
        }
        Key::Normal { .. } => make_normal(job, decoded, &mut failures),
        Key::MetallicRoughness { .. } => make_packed(job, decoded, &mut outcome, &mut failures),
        Key::LayerBase { .. } => make_base_layers(job, decoded, &mut outcome, &mut failures),
        Key::LayerNormal { .. } => make_normal_layers(job, decoded, &mut failures),
    };
    match made {
        Some(Ok(image)) => outcome.image = Some(image),
        Some(Err(reason)) => {
            if let Some((role, source)) = job.primary() {
                failures.push(Failure {
                    role,
                    file: job.file_of(source),
                    reason,
                });
            }
        }
        None => {}
    }
    outcome.failures = failures;
    outcome
}

fn make_base(
    job: &Job,
    keep_alpha: bool,
    decoded: &Decoded,
    outcome: &mut Outcome,
    failures: &mut Vec<Failure>,
) -> Made {
    let diffuse = load(
        decoded,
        TextureRole::Diffuse,
        job.diffuse.as_ref(),
        failures,
    )?;
    let specular = load(
        decoded,
        TextureRole::Specular,
        job.specular.as_ref(),
        failures,
    );
    outcome.tinted = specular.is_some();
    if job.diffuse.as_ref().is_some_and(|source| source.opacity) {
        return Some(
            opacity_colour(&diffuse.image, job.tints.decal)
                .map_err(|error| error.to_string())
                .and_then(|image| encode(&image)),
        );
    }
    Some(
        base_colour(
            &diffuse.image,
            specular.as_ref().map(|specular| &specular.image),
            job.tints,
            keep_alpha,
        )
        .map_err(|error| error.to_string())
        .and_then(|image| encode(&image)),
    )
}

fn make_normal(job: &Job, decoded: &Decoded, failures: &mut Vec<Failure>) -> Made {
    let normal = load(decoded, TextureRole::Normal, job.normal.as_ref(), failures)?;
    Some(
        normal_map(&normal.image)
            .map_err(|error| error.to_string())
            .and_then(|image| encode(&image)),
    )
}

fn make_packed(
    job: &Job,
    decoded: &Decoded,
    outcome: &mut Outcome,
    failures: &mut Vec<Failure>,
) -> Made {
    let gloss = load(decoded, TextureRole::Normal, job.normal.as_ref(), failures)
        .filter(|gloss| gloss.has_alpha);
    let specular = load(
        decoded,
        TextureRole::Specular,
        job.specular.as_ref(),
        failures,
    );
    match metallic_roughness(
        gloss.as_ref().map(|gloss| &gloss.image),
        specular.as_ref().map(|specular| &specular.image),
        job.tints.specular,
    ) {
        Ok(Some(packed)) => {
            outcome.roughness = packed.roughness;
            outcome.metallic = packed.metallic;
            Some(encode(&packed.image))
        }
        Ok(None) => None,
        Err(error) => Some(Err(error.to_string())),
    }
}
