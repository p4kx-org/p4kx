// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;

use crate::materials::Slot;
use crate::texture_make::Outcome;
use crate::texture_plan::{
    BaseImage, Failure, MetallicRoughnessImage, SlotImages, TextureSet, Wanted,
};
use crate::texture_report::{TextureFallback, TextureRole};

type Reported = HashSet<(String, TextureRole, String, String)>;

/// The images one material of one model instance asks for, with the name it is reported by.
pub(crate) struct Wish {
    pub(crate) part: usize,
    pub(crate) slot: Slot,
    pub(crate) name: String,
    pub(crate) jobs: Wanted,
    /// The plain normal image of a material whose layered normal image is its normal job: the
    /// material is drawn with it when the layered one could not be made.
    pub(crate) fallback: Option<usize>,
    pub(crate) refused: Vec<Failure>,
}

impl Wish {
    /// Returns the job whose normal image the material is drawn with: its own, or the fallback
    /// when `outcomes` made no image for it.
    fn normal_job(&self, outcomes: &[Outcome]) -> Option<usize> {
        let [_, normal, _] = self.jobs;
        match (normal, self.fallback) {
            (Some(job), Some(fallback)) if outcomes.get(job).is_some_and(|o| o.image.is_none()) => {
                Some(fallback)
            }
            (normal, _) => normal,
        }
    }
}

/// Returns the images the material of `wish` is drawn with, `drawn` being its normal job.
fn slot_images(wish: &Wish, drawn: Option<usize>, made: &[(Option<usize>, Outcome)]) -> SlotImages {
    let [base, _, packed] = wish.jobs;
    let of = |job: Option<usize>| job.and_then(|job| made.get(job));
    SlotImages {
        base: of(base).and_then(|(index, outcome)| {
            Some(BaseImage {
                image: (*index)?,
                tinted: outcome.tinted,
                layered: outcome.layered,
            })
        }),
        normal: of(drawn).and_then(|(index, _)| *index),
        metallic_roughness: of(packed).and_then(|(index, outcome)| {
            Some(MetallicRoughnessImage {
                image: (*index)?,
                roughness: outcome.roughness,
                metallic: outcome.metallic,
            })
        }),
    }
}

impl TextureSet {
    /// Makes the set of the images `outcomes` hold that the materials of `wanted` use, and lists
    /// what they could not use.
    pub(crate) fn assemble(wanted: &[Wish], outcomes: Vec<Outcome>) -> Self {
        let mut used = vec![false; outcomes.len()];
        let drawn: Vec<Option<usize>> = wanted
            .iter()
            .map(|wish| wish.normal_job(&outcomes))
            .collect();
        for (wish, normal) in wanted.iter().zip(&drawn) {
            let [base, _, packed] = wish.jobs;
            for job in [base, *normal, packed].into_iter().flatten() {
                if let Some(used) = used.get_mut(job) {
                    *used = true;
                }
            }
        }
        let mut set = Self::default();
        let mut made: Vec<(Option<usize>, Outcome)> = Vec::with_capacity(outcomes.len());
        for (mut outcome, used) in outcomes.into_iter().zip(&used) {
            let index = outcome.image.take().filter(|_| *used).map(|image| {
                set.images.push(image);
                set.image_sources
                    .push(std::mem::take(&mut outcome.texture_sources));
                set.images.len() - 1
            });
            made.push((index, outcome));
        }
        let mut reported = Reported::new();
        for (wish, drawn) in wanted.iter().zip(drawn) {
            let [base, normal, packed] = wish.jobs;
            set.slots
                .insert((wish.part, wish.slot), slot_images(wish, drawn, &made));
            let made_failures = [
                base,
                normal,
                packed,
                drawn.filter(|job| Some(*job) != normal),
            ]
            .into_iter()
            .flatten()
            .filter_map(|job| made.get(job))
            .flat_map(|(_, outcome)| &outcome.failures);
            for failure in wish.refused.iter().chain(made_failures) {
                set.note(&mut reported, &wish.name, failure);
            }
        }
        set
    }

    /// Lists `failure` as a fallback of the material `name` unless `reported` has it already.
    fn note(&mut self, reported: &mut Reported, name: &str, failure: &Failure) {
        let key = (
            name.to_owned(),
            failure.role,
            failure.file.clone(),
            failure.reason.clone(),
        );
        if reported.insert(key) {
            self.fallbacks.push(TextureFallback {
                material: name.to_owned(),
                role: failure.role,
                file: failure.file.clone(),
                reason: failure.reason.clone(),
            });
        }
    }
}
