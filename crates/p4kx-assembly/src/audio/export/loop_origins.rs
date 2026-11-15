// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::path::PathBuf;

use p4kx_archive::{Archive, ObservedEntry, ObservedEntryBinding};
use p4kx_audio::{EmbeddedWemSource, Loop, Wem};
use p4kx_extraction::sound::{WemOriginField, WemOriginPlan, WemOriginSite};

use super::{ExportState, ExportedSound};

/// The original inputs contributing to loop markers for newly written sounds.
#[derive(Debug, Default)]
pub struct ExportLoopOrigins {
    by_path: HashMap<PathBuf, ExportLoopOrigin>,
}

impl ExportLoopOrigins {
    /// Returns the loop inputs of `sound` from the corresponding export report.
    #[must_use]
    pub fn for_sound(&self, sound: &ExportedSound) -> Option<&ExportLoopOrigin> {
        if sound.state != ExportState::Written {
            return None;
        }
        let evidence = self.by_path.get(&sound.path)?;
        (sound.wem_id == evidence.wem_id && sound.loop_points == Some(evidence.points))
            .then_some(evidence)
    }

    pub(super) fn insert(&mut self, sound: &ExportedSound, origin: ExportLoopOrigin) {
        self.by_path.insert(sound.path.clone(), origin);
    }
}

/// Position and parsed loop inputs of the WEM used by one newly written sound.
#[derive(Debug)]
pub struct ExportLoopOrigin {
    binding: ObservedEntryBinding,
    wem_id: u32,
    points: Loop,
    embedded: Option<EmbeddedWemSource>,
    start: Option<Vec<WemOriginSite>>,
    end: Option<Vec<WemOriginSite>>,
}

impl ExportLoopOrigin {
    pub(super) fn capture(
        wem_id: u32,
        wem: &Wem<&[u8]>,
        binding: &ObservedEntryBinding,
        embedded: Option<EmbeddedWemSource>,
    ) -> Option<Self> {
        let points = wem.loop_points()?;
        let plan = WemOriginPlan::new(wem.info_source(), wem.codec(), wem.sample_count());
        let start = plan.field(WemOriginField::LoopStart).ok();
        let end = plan.field(WemOriginField::LoopEnd).ok();
        Some(Self {
            binding: binding.clone(),
            wem_id,
            points,
            embedded,
            start,
            end,
        })
    }

    /// Returns the actual archive Entry used for the WEM or its containing bank.
    #[must_use]
    pub fn entry<'archive>(&self, archive: &'archive Archive) -> Option<ObservedEntry<'archive>> {
        let candidate = archive.entries().get(self.binding.entry_index() as usize)?;
        let observed = archive.observed_entry(candidate)?;
        self.binding
            .is_same_entry(&observed.binding())
            .then_some(observed)
    }

    /// Returns the physical DIDX and DATA position for an embedded WEM.
    #[must_use]
    pub const fn embedded(&self) -> Option<EmbeddedWemSource> {
        self.embedded
    }

    /// Returns the checked WEM-relative inputs used to obtain the loop start.
    #[must_use]
    pub fn start(&self) -> Option<&[WemOriginSite]> {
        self.start.as_deref()
    }

    /// Returns the checked WEM-relative inputs used to obtain the exclusive loop end.
    #[must_use]
    pub fn end(&self) -> Option<&[WemOriginSite]> {
        self.end.as_deref()
    }
}

#[derive(Clone, Copy)]
pub(super) enum CaptureLoopOrigins {
    Omit,
    Retain,
}
