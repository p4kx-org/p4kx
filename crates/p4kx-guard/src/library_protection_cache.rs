// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::PathBuf;

use p4kx_channels::{DiscoveryIssue, LauncherSnapshot, LauncherStamp};

use crate::Installation;

/// Reuses complete log data while refreshing current library targets.
#[derive(Debug, Default)]
pub(super) struct LibraryProtectionCache {
    complete: Option<CachedLibraries>,
}

#[derive(Debug)]
struct CachedLibraries {
    stamp: LauncherStamp,
    paths: Vec<PathBuf>,
}

impl LibraryProtectionCache {
    pub(super) const fn new() -> Self {
        Self { complete: None }
    }

    pub(super) fn refresh(
        &mut self,
        snapshot: &LauncherSnapshot,
        installation: &mut Installation,
    ) -> Vec<DiscoveryIssue> {
        if snapshot.is_complete()
            && let Some(cached) = &self.complete
            && &cached.stamp == snapshot.stamp()
        {
            protect_current_targets(&cached.paths, installation);
            return Vec::new();
        }
        let report = snapshot.read_libraries();
        let reusable = snapshot.is_complete() && report.is_complete();
        let (libraries, issues) = report.into_parts();
        protect_current_targets(&libraries, installation);
        self.complete = reusable.then(|| CachedLibraries {
            stamp: snapshot.stamp().clone(),
            paths: libraries,
        });
        issues
    }
}

fn protect_current_targets(libraries: &[PathBuf], installation: &mut Installation) {
    for library in libraries {
        installation.add(library);
    }
}

#[cfg(test)]
#[path = "library_protection_fixture.rs"]
mod fixture;

#[cfg(test)]
#[path = "library_protection_cache_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "library_protection_rebind_tests.rs"]
mod rebind_tests;
