// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_archive::{ObservedEntry, ObservedEntryBinding};
use p4kx_audio::{EmbeddedWemSource, Soundbank, Wem};
use p4kx_extraction::sound::SoundOriginInput;

use super::super::loop_origins::CapturedSoundOrigin;
use super::super::source::WemExportSource;

pub(super) struct GroupInput<'a> {
    pub(super) bytes: &'a [u8],
    pub(super) bank: Option<&'a Soundbank<&'a [u8]>>,
    pub(super) observed: Option<ObservedEntry<'a>>,
    pub(super) binding: Option<&'a ObservedEntryBinding>,
}

impl<'a> GroupInput<'a> {
    pub(super) fn for_wem(&self, id: u32, source: WemExportSource) -> Option<SoundInput<'a>> {
        let (bytes, prefetch, embedded) = match source {
            WemExportSource::Soundbank { offset, length, .. } => {
                let wem = self.bank?.wem(id)?;
                if wem.offset() != offset || wem.bytes().len() as u64 != length {
                    return None;
                }
                (wem.bytes(), wem.is_prefetch(), Some(wem.source()))
            }
            WemExportSource::Loose { .. } => (self.bytes, false, None),
        };
        Some(SoundInput {
            bytes,
            prefetch,
            observed: self.observed,
            binding: self.binding,
            embedded,
        })
    }
}

pub(super) struct SoundInput<'a> {
    pub(super) bytes: &'a [u8],
    pub(super) prefetch: bool,
    observed: Option<ObservedEntry<'a>>,
    binding: Option<&'a ObservedEntryBinding>,
    embedded: Option<EmbeddedWemSource>,
}

impl<'a> SoundInput<'a> {
    pub(super) fn observed_source(&self) -> Option<SoundOriginInput<'a>> {
        self.observed
            .map(|entry| SoundOriginInput::for_archive_entry(entry, self.embedded))
    }

    pub(super) fn origin(&self, id: u32, wem: Option<&Wem<&[u8]>>) -> Option<CapturedSoundOrigin> {
        if self.prefetch {
            return None;
        }
        Some(CapturedSoundOrigin::capture(
            id,
            wem,
            self.binding?,
            self.embedded,
            self.bytes.len() as u64,
        ))
    }
}
