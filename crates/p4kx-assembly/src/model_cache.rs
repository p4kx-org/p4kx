// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};

use p4kx_extraction::model::{LayerBudget, MaterialStatus, Model, RigLogicBytes};

use crate::provider::{ModelFailure, ModelProvider, ProvidedModel};

/// The bytes of models a [`ModelCache`] keeps unless told otherwise.
pub const DEFAULT_MODEL_CACHE_BYTES: usize = 256 * 1024 * 1024;

#[derive(Debug)]
struct Cached {
    lowest: u32,
    highest: u32,
    used: u64,
    bytes: usize,
    model: Arc<ProvidedModel>,
}

#[derive(Debug, Default)]
struct State {
    models: HashMap<String, Vec<Cached>>,
    bytes: usize,
    clock: u64,
}

/// Models already read, kept within a bound of memory so building an Assembly again, after a
/// change of the Loadout or the level of detail, does not read them from the Archive again.
///
/// The least recently used models are dropped first. A model larger than the bound is not kept.
/// The cache may be shared between threads and between builds.
///
/// The bound counts the geometry buffers and the `RigLogic` chunk of the models, not their
/// nodes, names or chunk lists, so the memory held is somewhat above it. While a build runs, the models of the
/// Assembly being built and those of the cache are in memory together.
#[derive(Debug)]
pub struct ModelCache {
    limit: usize,
    state: Mutex<State>,
}

impl Default for ModelCache {
    fn default() -> Self {
        Self::new(DEFAULT_MODEL_CACHE_BYTES)
    }
}

fn key_of(path: &str) -> String {
    path.chars()
        .map(|c| {
            if c == '\\' {
                '/'
            } else {
                c.to_ascii_lowercase()
            }
        })
        .collect()
}

/// Returns the bytes of the geometry buffers of `model`, its vertex streams and indices, and of
/// its `RigLogic` chunk.
fn bytes_of(model: &Model) -> usize {
    let size = |len: usize, each: usize| len.saturating_mul(each);
    let chunk = model.rig_logic.as_ref().map_or(0, RigLogicBytes::len);
    model
        .lods
        .iter()
        .flat_map(|lod| &lod.meshes)
        .map(|mesh| {
            size(mesh.positions.len(), 12)
                .saturating_add(size(mesh.source.len(), 4))
                .saturating_add(size(mesh.normals.as_ref().map_or(0, Vec::len), 12))
                .saturating_add(size(mesh.tangents.as_ref().map_or(0, Vec::len), 16))
                .saturating_add(size(mesh.colours.as_ref().map_or(0, Vec::len), 4))
                .saturating_add(size(
                    mesh.influences.as_ref().map_or(0, Vec::len),
                    size_of::<p4kx_extraction::model::Influences>(),
                ))
                .saturating_add(size(mesh.uvs.len(), 8))
                .saturating_add(size(mesh.second_uvs.as_ref().map_or(0, Vec::len), 8))
                .saturating_add(size(mesh.indices.len(), 4))
        })
        .fold(chunk, usize::saturating_add)
}

impl ModelCache {
    /// Creates a cache that keeps at most about `limit` bytes of models.
    #[must_use]
    pub fn new(limit: usize) -> Self {
        Self {
            limit,
            state: Mutex::new(State::default()),
        }
    }

    fn state(&self) -> MutexGuard<'_, State> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Returns the number of models kept.
    #[must_use]
    pub fn len(&self) -> usize {
        self.state().models.values().map(Vec::len).sum()
    }

    /// Returns whether no model is kept.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns about how many bytes of models are kept.
    #[must_use]
    pub fn bytes(&self) -> usize {
        self.state().bytes
    }

    /// Drops every model.
    pub fn clear(&self) {
        *self.state() = State::default();
    }

    fn get(&self, key: &str, lod: u32) -> Option<Arc<ProvidedModel>> {
        self.state().touch(key, lod)
    }

    fn store(&self, key: String, lod: u32, model: &ProvidedModel) {
        let bytes = model
            .attachments
            .iter()
            .fold(bytes_of(&model.model), |sum, attached| {
                sum.saturating_add(bytes_of(&attached.model))
            });
        if bytes > self.limit {
            return;
        }
        let level = model.model.lods.first().map_or(lod, |found| found.level);
        let lowest = if level <= lod { level } else { 0 };
        let copy = Arc::new(model.clone());
        let mut state = self.state();
        state.clock += 1;
        let used = state.clock;
        let entries = state.models.entry(key).or_default();
        if entries
            .iter()
            .any(|cached| cached.lowest <= lowest && lod <= cached.highest)
        {
            return;
        }
        let mut freed = 0;
        entries.retain(|cached| {
            let covered = lowest <= cached.lowest && cached.highest <= lod;
            if covered {
                freed += cached.bytes;
            }
            !covered
        });
        entries.push(Cached {
            lowest,
            highest: lod,
            used,
            bytes,
            model: copy,
        });
        state.bytes = state.bytes.saturating_sub(freed).saturating_add(bytes);
        state.evict(self.limit);
    }
}

impl State {
    fn touch(&mut self, key: &str, lod: u32) -> Option<Arc<ProvidedModel>> {
        self.clock += 1;
        let clock = self.clock;
        let found = self
            .models
            .get_mut(key)?
            .iter_mut()
            .find(|cached| (cached.lowest..=cached.highest).contains(&lod))?;
        found.used = clock;
        Some(Arc::clone(&found.model))
    }

    fn evict(&mut self, limit: usize) {
        while self.bytes > limit {
            let oldest = self
                .models
                .iter()
                .flat_map(|(key, list)| {
                    list.iter()
                        .enumerate()
                        .map(move |(at, c)| (c.used, key, at))
                })
                .min_by_key(|(used, _, _)| *used)
                .map(|(_, key, at)| (key.clone(), at));
            let Some((key, at)) = oldest else {
                debug_assert!(false, "the cache counts bytes without holding a model");
                return;
            };
            if let Some(list) = self.models.get_mut(&key) {
                let gone = list.swap_remove(at);
                self.bytes = self.bytes.saturating_sub(gone.bytes);
                if list.is_empty() {
                    self.models.remove(&key);
                }
            }
        }
    }
}

/// A [`ModelProvider`] that answers from a [`ModelCache`] and reads from `inner` only what the
/// cache lacks.
///
/// A model read at the level of detail `lod` that came out at the level `found` answers every
/// request from `found` up to `lod`, and, when the model has no level up to `lod`, every request
/// below it; a level the cache has not read is read from `inner`.
pub struct CachedModels<'a> {
    inner: &'a dyn ModelProvider,
    cache: &'a ModelCache,
}

impl<'a> CachedModels<'a> {
    /// Wraps `inner` with `cache`.
    #[must_use]
    pub const fn new(inner: &'a dyn ModelProvider, cache: &'a ModelCache) -> Self {
        Self { inner, cache }
    }
}

impl ModelProvider for CachedModels<'_> {
    fn model(&self, path: &str, lod: u32) -> Result<ProvidedModel, ModelFailure> {
        let key = key_of(path);
        if let Some(found) = self.cache.get(&key, lod) {
            return Ok(ProvidedModel::clone(&found));
        }
        let read = self.inner.model(path, lod)?;
        self.cache.store(key, lod, &read);
        Ok(read)
    }

    fn library(&self, entry: &str, name: &str) -> MaterialStatus {
        self.inner.library(entry, name)
    }

    fn layer_budget(&self) -> LayerBudget {
        self.inner.layer_budget()
    }

    fn layer_library(&self, path: &str, budget: &mut LayerBudget) -> Arc<MaterialStatus> {
        self.inner.layer_library(path, budget)
    }

    fn file(&self, path: &str, limit: u64) -> Result<Vec<u8>, ModelFailure> {
        self.inner.file(path, limit)
    }
}
