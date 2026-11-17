// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use rayon::prelude::*;

use crate::texture_loader::{LoadedTexture, TextureLoadError, TextureLoader};
use crate::texture_plan::Job;

/// The most pixels the decoded sources kept for the next group of jobs may hold together.
pub(crate) const MAX_HELD_PIXELS: u64 = 1 << 27;

/// The most pixels the distinct sources of one group of jobs may decode together, measured from
/// the headers; a single job whose sources exceed it still runs, alone.
pub(crate) const MAX_DECODE_PIXELS: u64 = 1 << 27;

/// A decoded source texture, or why it could not be loaded.
pub(crate) type Loaded = Result<Arc<LoadedTexture>, TextureLoadError>;

/// The decoded sources one group of jobs is made from, by path.
pub(crate) type Decoded = HashMap<String, Loaded>;

/// Returns the paths of the source textures `job` may load.
pub(crate) fn paths(job: &Job) -> impl Iterator<Item = &str> {
    primary_paths(job).chain(job.explicit_gloss.iter().map(|source| source.path.as_str()))
}

fn primary_paths(job: &Job) -> impl Iterator<Item = &str> {
    let maps = job.layer_maps();
    [&job.diffuse, &job.normal, &job.specular]
        .into_iter()
        .flatten()
        .map(|source| source.path.as_str())
        .chain(
            job.layered
                .iter()
                .flat_map(move |layered| layered.paths(maps)),
        )
}

fn has_ddna_smoothness(job: &Job, decoded: &Decoded) -> bool {
    job.normal
        .as_ref()
        .and_then(|source| decoded.get(&source.path))
        .and_then(|loaded| loaded.as_ref().ok())
        .is_some_and(|texture| texture.has_alpha)
}

/// Decodes the source textures of a build a group of jobs at a time, each once per group however
/// many jobs of it use it, sizes the groups so their sources fit a pixel limit, and keeps those
/// the next group needs while they fit a pixel cap.
pub(crate) struct SourceStore<'l> {
    loader: &'l dyn TextureLoader,
    mip: u32,
    cap: u64,
    limit: u64,
    held: Decoded,
    sizes: HashMap<String, Result<Option<(u32, u32)>, TextureLoadError>>,
}

fn pixels(loaded: &Loaded) -> u64 {
    loaded.as_ref().map_or(0, |texture| {
        u64::from(texture.image.width()) * u64::from(texture.image.height())
    })
}

impl<'l> SourceStore<'l> {
    /// Creates a store that loads at mip `mip` with `loader` and holds at most `cap` pixels
    /// between groups.
    pub(crate) fn new(loader: &'l dyn TextureLoader, mip: u32, cap: u64) -> Self {
        Self {
            loader,
            mip,
            cap,
            limit: MAX_DECODE_PIXELS,
            held: HashMap::new(),
            sizes: HashMap::new(),
        }
    }

    /// Sets the pixels the distinct sources of one group of jobs may decode together.
    #[cfg(test)]
    pub(crate) const fn decoding(mut self, limit: u64) -> Self {
        self.limit = limit;
        self
    }

    /// Returns the decoded sources of `jobs`, taking the held ones and decoding the rest in
    /// parallel.
    pub(crate) fn prepare<'a, Jobs>(&mut self, jobs: Jobs) -> Decoded
    where
        Jobs: IntoIterator<Item = &'a Job>,
        Jobs::IntoIter: Clone,
    {
        let jobs = jobs.into_iter();
        let mut decoded = Decoded::new();
        self.load_paths(jobs.clone().flat_map(primary_paths), &mut decoded);
        for job in jobs.filter(|job| !job.explicit_gloss.is_empty()) {
            if has_ddna_smoothness(job, &decoded) {
                continue;
            }
            for source in &job.explicit_gloss {
                self.load_paths(std::iter::once(source.path.as_str()), &mut decoded);
                if decoded.get(&source.path).is_some_and(Result::is_ok) {
                    break;
                }
            }
        }
        decoded
    }

    fn load_paths<'a>(&mut self, paths: impl Iterator<Item = &'a str>, decoded: &mut Decoded) {
        let mut missing: Vec<&str> = Vec::new();
        for path in paths {
            if decoded.contains_key(path) || missing.contains(&path) {
                continue;
            }
            match self.held.remove_entry(path) {
                Some((path, held)) => {
                    decoded.insert(path, held);
                }
                None => missing.push(path),
            }
        }
        let (loader, mip) = (self.loader, self.mip);
        let fresh: Vec<(String, Loaded)> = missing
            .par_iter()
            .map(|path| ((*path).to_owned(), loader.load(path, mip).map(Arc::new)))
            .collect();
        decoded.extend(fresh);
    }

    /// Keeps the sources, held before or in `decoded`, that `next` uses, as many as fit the cap.
    pub(crate) fn keep<'a>(&mut self, decoded: Decoded, next: impl IntoIterator<Item = &'a Job>) {
        let wanted: HashSet<&str> = next.into_iter().flat_map(paths).collect();
        self.held.retain(|path, _| wanted.contains(path.as_str()));
        let mut held_pixels: u64 = self.held.values().map(pixels).sum();
        for (path, loaded) in decoded {
            let size = pixels(&loaded);
            if wanted.contains(path.as_str()) && held_pixels.saturating_add(size) <= self.cap {
                held_pixels += size;
                self.held.insert(path, loaded);
            }
        }
    }

    /// Returns the pixels the held sources hold.
    #[cfg(test)]
    pub(crate) fn held_pixels(&self) -> u64 {
        self.held.values().map(pixels).sum()
    }

    /// Returns the size of the texture at `path` without decoding it, when the loader knows; the
    /// loader is asked once per path, its error included.
    pub(crate) fn dimensions(
        &mut self,
        path: &str,
    ) -> Result<Option<(u32, u32)>, TextureLoadError> {
        if let Some(size) = self.sizes.get(path) {
            return size.clone();
        }
        let size = self.loader.dimensions(path, self.mip);
        self.sizes.insert(path.to_owned(), size.clone());
        size
    }

    /// Returns the pixels the distinct textures `paths` decode together, by their headers; a
    /// texture whose size is unknown counts for the whole decode limit, and one whose header
    /// cannot be read for nothing, because it cannot be decoded either.
    pub(crate) fn header_pixels<'a>(&mut self, paths: impl Iterator<Item = &'a str>) -> u64 {
        let mut seen: Vec<&str> = Vec::new();
        let mut total = 0_u64;
        for path in paths {
            if seen.contains(&path) {
                continue;
            }
            seen.push(path);
            total = total.saturating_add(match self.dimensions(path) {
                Ok(Some((width, height))) => u64::from(width) * u64::from(height),
                Ok(None) => self.limit,
                Err(_) => 0,
            });
        }
        total
    }

    /// Returns how many of the leading `jobs`, at most `most`, may be made together: the
    /// distinct sources they decode stay within the decode limit, but the first job always counts.
    /// A source whose size is unknown or cannot be read counts for the whole limit.
    pub(crate) fn group_len(&mut self, jobs: &[Job], most: usize) -> usize {
        let mut seen: HashSet<&str> = HashSet::new();
        let mut total = 0u64;
        let mut count = 0;
        for job in jobs.iter().take(most) {
            let mut added = 0u64;
            let mut fresh: Vec<&str> = Vec::new();
            for path in paths(job) {
                if seen.contains(path) || fresh.contains(&path) {
                    continue;
                }
                fresh.push(path);
                added = added.saturating_add(match self.dimensions(path) {
                    Ok(Some((width, height))) => u64::from(width) * u64::from(height),
                    Ok(None) | Err(_) => self.limit,
                });
            }
            if count > 0 && total.saturating_add(added) > self.limit {
                break;
            }
            total = total.saturating_add(added);
            seen.extend(fresh);
            count += 1;
        }
        count
    }
}
