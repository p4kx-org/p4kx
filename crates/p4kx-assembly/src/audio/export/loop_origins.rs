// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::ops::Range;
use std::path::PathBuf;

use p4kx_archive::{Archive, ObservedEntry, ObservedEntryBinding};
use p4kx_audio::{EmbeddedWemSource, Loop, Wem};
use p4kx_extraction::sound::{WemOriginField, WemOriginPlan, WemOriginSite};

use super::{ExportState, ExportedSound};

/// The original inputs contributing to loop markers for newly written sounds.
#[derive(Debug, Default)]
pub struct ExportLoopOrigins {
    by_path: HashMap<PathBuf, CapturedSoundOrigin>,
}

impl ExportLoopOrigins {
    /// Returns the loop inputs of `sound` from the corresponding export report.
    #[must_use]
    pub fn for_sound(&self, sound: &ExportedSound) -> Option<&ExportLoopOrigin> {
        let CapturedSoundOrigin::Looped(evidence) = self.source_of(sound)? else {
            return None;
        };
        (sound.loop_points == Some(evidence.points)).then_some(evidence)
    }

    /// Returns the WEM consumed to write the reported complete sound.
    #[must_use]
    pub(crate) fn media_for_sound(&self, sound: &ExportedSound) -> Option<&ExportedWemOrigin> {
        self.source_of(sound).map(CapturedSoundOrigin::media)
    }

    fn source_of(&self, sound: &ExportedSound) -> Option<&CapturedSoundOrigin> {
        if sound.state != ExportState::Written || sound.prefetch_only {
            return None;
        }
        let evidence = self.by_path.get(&sound.path)?;
        (sound.wem_id == evidence.media().wem_id).then_some(evidence)
    }

    pub(super) fn insert(&mut self, sound: &ExportedSound, origin: CapturedSoundOrigin) {
        self.by_path.insert(sound.path.clone(), origin);
    }
}

#[derive(Debug)]
pub(super) enum CapturedSoundOrigin {
    WithoutLoop(ExportedWemOrigin),
    Looped(ExportLoopOrigin),
}

impl CapturedSoundOrigin {
    pub(super) fn capture(
        wem_id: u32,
        wem: Option<&Wem<&[u8]>>,
        binding: &ObservedEntryBinding,
        embedded: Option<EmbeddedWemSource>,
        decoded_length: u64,
    ) -> Self {
        let media = ExportedWemOrigin {
            binding: binding.clone(),
            wem_id,
            placement: embedded.map_or(
                WemPlacement::Loose { decoded_length },
                WemPlacement::Embedded,
            ),
        };
        let Some((wem, points)) = wem.and_then(|wem| wem.loop_points().map(|points| (wem, points)))
        else {
            return Self::WithoutLoop(media);
        };
        let plan = WemOriginPlan::new(wem.info_source(), wem.codec(), wem.sample_count());
        Self::Looped(ExportLoopOrigin {
            media,
            points,
            start: plan.field(WemOriginField::LoopStart).ok(),
            end: plan.field(WemOriginField::LoopEnd).ok(),
        })
    }

    const fn media(&self) -> &ExportedWemOrigin {
        match self {
            Self::WithoutLoop(media) => media,
            Self::Looped(origin) => &origin.media,
        }
    }
}

#[derive(Debug)]
enum WemPlacement {
    Loose { decoded_length: u64 },
    Embedded(EmbeddedWemSource),
}

/// The physical source of one newly written complete sound.
#[derive(Debug)]
pub(crate) struct ExportedWemOrigin {
    binding: ObservedEntryBinding,
    wem_id: u32,
    placement: WemPlacement,
}

impl ExportedWemOrigin {
    /// Returns the actual archive Entry used for the WEM or its containing bank.
    #[must_use]
    pub(crate) fn entry<'archive>(
        &self,
        archive: &'archive Archive,
    ) -> Option<ObservedEntry<'archive>> {
        let candidate = archive.entries().get(self.binding.entry_index() as usize)?;
        let observed = archive.observed_entry(candidate)?;
        self.binding
            .is_same_entry(&observed.binding())
            .then_some(observed)
    }

    /// Returns the physical DIDX and DATA position for an embedded WEM.
    #[must_use]
    pub(crate) const fn embedded(&self) -> Option<EmbeddedWemSource> {
        match self.placement {
            WemPlacement::Loose { .. } => None,
            WemPlacement::Embedded(source) => Some(source),
        }
    }

    /// Returns the half-open WEM payload range relative to the decoded Entry.
    #[must_use]
    pub(crate) const fn byte_range(&self) -> Range<u64> {
        match self.placement {
            WemPlacement::Loose { decoded_length } => 0..decoded_length,
            WemPlacement::Embedded(source) => source.byte_range(),
        }
    }
}

/// Position and parsed loop inputs of the WEM used by one newly written sound.
#[derive(Debug)]
pub struct ExportLoopOrigin {
    media: ExportedWemOrigin,
    points: Loop,
    start: Option<Vec<WemOriginSite>>,
    end: Option<Vec<WemOriginSite>>,
}

impl ExportLoopOrigin {
    /// Returns the actual archive Entry used for the WEM or its containing bank.
    #[must_use]
    pub fn entry<'archive>(&self, archive: &'archive Archive) -> Option<ObservedEntry<'archive>> {
        self.media.entry(archive)
    }

    /// Returns the physical DIDX and DATA position for an embedded WEM.
    #[must_use]
    pub const fn embedded(&self) -> Option<EmbeddedWemSource> {
        self.media.embedded()
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
pub(super) enum CaptureOrigins {
    Omit,
    Retain,
}
