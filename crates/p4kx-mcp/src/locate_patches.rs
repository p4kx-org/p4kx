// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_channels::{LauncherSnapshot, LauncherStamp, PublicPatches};

use crate::discovery_notes;

#[derive(Debug)]
struct CachedPatches {
    stamp: LauncherStamp,
    patches: Arc<PublicPatches>,
}

/// Cache admission is bound to the current physical sources and a complete read.
#[derive(Debug, Default)]
pub(super) struct PatchCache {
    complete: Option<CachedPatches>,
}

impl PatchCache {
    pub(super) fn read(&mut self, snapshot: &LauncherSnapshot) -> Arc<PublicPatches> {
        if snapshot.is_complete()
            && let Some(cached) = &self.complete
            && &cached.stamp == snapshot.stamp()
        {
            return Arc::clone(&cached.patches);
        }
        let report = snapshot.read_patches();
        discovery_notes::remember(report.issues());
        let complete = report.is_complete();
        self.record(snapshot, report.into_value(), complete)
    }

    pub(super) fn record(
        &mut self,
        snapshot: &LauncherSnapshot,
        patches: PublicPatches,
        complete: bool,
    ) -> Arc<PublicPatches> {
        let reusable = snapshot.is_complete() && complete;
        if reusable
            && let Some(cached) = &self.complete
            && &cached.stamp == snapshot.stamp()
            && cached.patches.as_ref() == &patches
        {
            return Arc::clone(&cached.patches);
        }
        let patches = Arc::new(patches);
        self.complete = reusable.then(|| CachedPatches {
            stamp: snapshot.stamp().clone(),
            patches: Arc::clone(&patches),
        });
        patches
    }
}
