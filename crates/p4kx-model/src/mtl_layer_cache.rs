// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, PoisonError};

use crate::mtl_load::request_path;
use crate::{
    CompanionPath, CompanionResolver, LayerSurface, Material, MaterialLibrary, MaterialStatus,
};

/// The most distinct layer files one load of a material library may be charged for; layers beyond
/// are reported as unresolved.
pub const MAX_LAYER_FILES: usize = 256;

/// The bytes of layer files after which one load of a material library stops fetching; layers
/// beyond are reported as unresolved.
pub const MAX_LAYER_BYTES: usize = 64 * 1024 * 1024;

/// The most layer files a [`LayerCache`] keeps by default; it starts over when it is full.
pub const MAX_CACHED_LAYER_FILES: usize = 4 * MAX_LAYER_FILES;

/// The limits of the layer files a load reads and a cache keeps.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LayerLimits {
    /// The distinct layer files one load is charged for.
    pub files_per_load: usize,
    /// The bytes of layer files after which one load stops fetching.
    pub bytes_per_load: usize,
    /// The layer files a cache keeps before it starts over.
    pub cached_files: usize,
    /// The bytes of the layer files a cache keeps before it starts over; the libraries it holds
    /// are parsed from those bytes, so they bound its memory.
    pub cached_bytes: usize,
}

impl Default for LayerLimits {
    fn default() -> Self {
        Self {
            files_per_load: MAX_LAYER_FILES,
            bytes_per_load: MAX_LAYER_BYTES,
            cached_files: MAX_CACHED_LAYER_FILES,
            cached_bytes: MAX_LAYER_BYTES,
        }
    }
}

/// What one load of a material library may still be charged for. A file counts once per load,
/// whether the cache held it or not, so which layers resolve does not depend on the load order.
#[derive(Debug)]
pub struct LayerBudget {
    files: usize,
    bytes: usize,
    charged: HashSet<String>,
}

impl Default for LayerBudget {
    /// Returns a budget with the default per-load limits.
    fn default() -> Self {
        Self::new(LayerLimits::default())
    }
}

impl LayerBudget {
    fn new(limits: LayerLimits) -> Self {
        Self {
            files: limits.files_per_load,
            bytes: limits.bytes_per_load,
            charged: HashSet::new(),
        }
    }

    /// Returns whether the file `key` may be read: it was charged already, or the budget is left.
    fn admits(&self, key: &str) -> bool {
        self.charged.contains(key) || (self.files > 0 && self.bytes > 0)
    }

    fn charge(&mut self, key: &str, bytes: usize) {
        if self.charged.insert(key.to_owned()) {
            self.files = self.files.saturating_sub(1);
            self.bytes = self.bytes.saturating_sub(bytes);
        }
    }
}

/// What fetching a layer file came to: the bytes it held and the library, or why there is none.
#[derive(Debug)]
struct Fetched {
    bytes: usize,
    status: Arc<MaterialStatus>,
}

type Table = HashMap<String, Arc<OnceLock<Fetched>>>;

/// Returns the position among `names` of the sub-material a layer's `Submtl` names: the first
/// for a single material file or an empty `submaterial`, else the one with that name, ignoring
/// case.
fn pick_position<'a>(
    single: bool,
    submaterial: &str,
    mut names: impl Iterator<Item = &'a str>,
) -> Option<usize> {
    if single || submaterial.is_empty() {
        names.next().map(|_| 0)
    } else {
        names.position(|name| name.eq_ignore_ascii_case(submaterial))
    }
}

impl MaterialLibrary {
    /// Returns the position of the sub-material the layer attribute `Submtl` names: the first
    /// one when the file is a single material or the name is empty, else the one with that name,
    /// ignoring case; `None` when there is none.
    #[must_use]
    pub fn layer_position(&self, submaterial: &str) -> Option<usize> {
        pick_position(
            self.single,
            submaterial.trim(),
            self.materials.iter().map(|material| material.name.as_str()),
        )
    }
}

impl LayerSurface {
    pub(crate) fn of(material: &Material) -> Self {
        Self {
            diffuse: material.diffuse,
            specular: material.specular,
            textures: material.textures.clone(),
        }
    }
}

/// The layer files read so far, as parsed libraries.
///
/// The loads of several material libraries and the Blender package share it, from several threads
/// at once, so that a file many libraries layer in is fetched and parsed once. The lock guards
/// only the table of files; a file is fetched and parsed outside it, once per key.
#[derive(Debug, Default)]
pub struct LayerCache {
    limits: LayerLimits,
    files: Mutex<Table>,
    cached_bytes: AtomicUsize,
}

impl LayerCache {
    /// Returns an empty cache with the default limits.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns an empty cache with the limits `limits`.
    #[must_use]
    pub fn with_limits(limits: LayerLimits) -> Self {
        Self {
            limits,
            files: Mutex::default(),
            cached_bytes: AtomicUsize::new(0),
        }
    }

    /// Returns how many layer files the cache holds.
    #[must_use]
    pub fn len(&self) -> usize {
        self.table().len()
    }

    /// Returns whether the cache holds no layer file.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.table().is_empty()
    }

    /// Returns a budget with the per-load limits of this cache.
    #[must_use]
    pub fn budget(&self) -> LayerBudget {
        LayerBudget::new(self.limits)
    }

    /// Returns the surface of the sub-material `submaterial` of the layer file at `path`, `None`
    /// when the path is unsafe, the file is missing or unreadable, has no such sub-material, or
    /// `budget` does not admit the file.
    pub(crate) fn surface(
        &self,
        path: &str,
        submaterial: &str,
        companions: &mut dyn CompanionResolver,
        budget: &mut LayerBudget,
    ) -> Option<LayerSurface> {
        let status = self.library(path, companions, budget);
        let library = status.library()?;
        library
            .materials
            .get(library.layer_position(submaterial)?)
            .map(LayerSurface::of)
    }

    /// Returns the layer file at `path` as a material library, read without its own layers and
    /// diffuse maps: a path that is unsafe, a file that is missing or unreadable, or one that
    /// `budget` does not admit is a status, not a failure. The file is fetched and parsed once
    /// per cache, whatever the loads that ask for it.
    #[must_use]
    pub fn library(
        &self,
        path: &str,
        companions: &mut dyn CompanionResolver,
        budget: &mut LayerBudget,
    ) -> Arc<MaterialStatus> {
        let Some(request) = request_path(path) else {
            return Arc::new(MaterialStatus::Unreadable {
                path: path.to_owned(),
                reason: "the path leaves the Data folder or is no plain path".to_owned(),
            });
        };
        let key = request.to_ascii_lowercase();
        if !budget.admits(&key) {
            return Arc::new(MaterialStatus::Unreadable {
                path: path.to_owned(),
                reason: "one load reads no more layer files than its budget allows".to_owned(),
            });
        }
        let cell = self.cell(&key);
        let mut fetching = false;
        let fetched = cell.get_or_init(|| {
            fetching = true;
            fetch(&request, companions)
        });
        if fetching {
            self.cached_bytes
                .fetch_add(fetched.bytes, Ordering::Relaxed);
        }
        budget.charge(&key, fetched.bytes);
        Arc::clone(&fetched.status)
    }

    fn table(&self) -> MutexGuard<'_, Table> {
        self.files.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn cell(&self, key: &str) -> Arc<OnceLock<Fetched>> {
        let mut files = self.table();
        if let Some(cell) = files.get(key) {
            return Arc::clone(cell);
        }
        if files.len() >= self.limits.cached_files
            || self.cached_bytes.load(Ordering::Relaxed) >= self.limits.cached_bytes
        {
            files.clear();
            self.cached_bytes.store(0, Ordering::Relaxed);
        }
        Arc::clone(files.entry(key.to_owned()).or_default())
    }
}

fn fetch(request: &str, companions: &mut dyn CompanionResolver) -> Fetched {
    let path = request.to_owned();
    let (bytes, status) = match companions.resolve(CompanionPath::Material(request)) {
        Ok(None) => (0, MaterialStatus::Missing { path }),
        Err(error) => (
            0,
            MaterialStatus::Unreadable {
                path,
                reason: error.to_string(),
            },
        ),
        Ok(Some(bytes)) => (
            bytes.len(),
            match MaterialLibrary::parse(request, &bytes) {
                Ok(library) => MaterialStatus::Loaded(library),
                Err(error) => MaterialStatus::Unreadable {
                    path,
                    reason: error.to_string(),
                },
            },
        ),
    };
    Fetched {
        bytes,
        status: Arc::new(status),
    }
}
