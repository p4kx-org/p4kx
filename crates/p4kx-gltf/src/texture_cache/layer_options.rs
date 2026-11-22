// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;
use std::collections::{HashMap, HashSet};

use super::SourceStore;
use crate::layer_plan::{LayerMaps, Layered};
use crate::texture_plan::{Failure, Job, Key};
use crate::texture_report::TextureRole;

/// A planned image whose optional layered maps have been checked against the decode budget.
pub(crate) struct PreparedTextureJob<'job> {
    job: Cow<'job, Job>,
    pub(crate) failures: Vec<Failure>,
}

impl PreparedTextureJob<'_> {
    pub(crate) fn job(&self) -> &Job {
        &self.job
    }

    pub(crate) fn needs_image(&self) -> bool {
        !matches!(self.job.key, Key::LayerFinish { .. })
            || self
                .job
                .layered
                .as_ref()
                .is_some_and(|layered| layered.has_specular_map() || layered.has_sampled_finish())
    }
}

struct DecodeLane<'path> {
    paths: HashSet<&'path str>,
    pixels: u64,
}

impl<'path> DecodeLane<'path> {
    fn new(store: &mut SourceStore<'_>, paths: impl Iterator<Item = &'path str>) -> Self {
        let mut seen = HashSet::new();
        let pixels = store.header_pixels(paths.inspect(|path| {
            seen.insert(*path);
        }));
        Self {
            paths: seen,
            pixels,
        }
    }

    fn adding(&self, path: &str, pixels: u64) -> u64 {
        self.pixels
            .saturating_add(if self.paths.contains(path) { 0 } else { pixels })
    }

    fn reserve(&mut self, path: &'path str, pixels: u64) {
        self.pixels = self.adding(path, pixels);
        self.paths.insert(path);
    }
}

impl SourceStore<'_> {
    pub(crate) fn admit_layer_specular<'job>(
        &mut self,
        job: &'job Job,
    ) -> PreparedTextureJob<'job> {
        let mut prepared = PreparedTextureJob {
            job: Cow::Borrowed(job),
            failures: Vec::new(),
        };
        if !matches!(job.key, Key::LayerBase { .. } | Key::LayerFinish { .. }) {
            return prepared;
        }
        let Some(layered) = job
            .layered
            .as_ref()
            .filter(|layered| layered.has_specular_map())
        else {
            return prepared;
        };
        let refusals = self.refused_layer_specular(layered);
        if !refusals.is_empty()
            && let Some(layered) = &mut prepared.job.to_mut().layered
        {
            for layer in &mut layered.layers {
                if let Some((source, _)) = &layer.specular_map
                    && let Some(reason) = refusals.get(source.path.as_str())
                {
                    prepared.failures.push(Failure {
                        role: TextureRole::Specular,
                        file: source.file.clone(),
                        reason: reason.clone(),
                    });
                    layer.specular_map = None;
                }
            }
        }
        prepared
    }

    fn refused_layer_specular<'layer>(
        &mut self,
        layered: &'layer Layered,
    ) -> HashMap<&'layer str, String> {
        let mut base = DecodeLane::new(self, layered.required_paths(LayerMaps::Diffuse));
        let mut finish = DecodeLane::new(self, layered.required_paths(LayerMaps::Finish));
        let mut considered = HashSet::new();
        let mut refused = HashMap::new();
        for path in layered
            .specular_sources()
            .map(|source| source.path.as_str())
        {
            if !considered.insert(path) {
                continue;
            }
            let reason = match self.dimensions(path) {
                Ok(Some((width, height))) => {
                    let pixels = u64::from(width) * u64::from(height);
                    if base.adding(path, pixels) <= self.limit && finish.adding(path, pixels) <= self.limit {
                        base.reserve(path, pixels);
                        finish.reserve(path, pixels);
                        continue;
                    }
                    "the optional layer specular map would exceed the texture decode budget; use a higher --texture-mip".to_owned()
                }
                Ok(None) => "the optional layer specular map has no verified dimensions for the texture decode budget".to_owned(),
                Err(error) => error.to_string(),
            };
            refused.insert(path, reason);
        }
        refused
    }
}
