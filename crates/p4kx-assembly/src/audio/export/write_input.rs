// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_archive::ObservedEntryBinding;
use p4kx_audio::{EmbeddedWemSource, Soundbank, Wem};

use super::super::loop_origins::CapturedSoundOrigin;

pub(super) struct GroupInput<'a> {
    pub(super) bytes: &'a [u8],
    pub(super) bank: Option<&'a Soundbank<&'a [u8]>>,
    pub(super) binding: Option<&'a ObservedEntryBinding>,
}

impl<'a> GroupInput<'a> {
    pub(super) fn for_wem(&self, id: u32) -> Option<SoundInput<'a>> {
        let (bytes, prefetch, embedded) = if let Some(bank) = self.bank {
            let wem = bank.wem(id)?;
            (
                wem.bytes(),
                wem.is_prefetch(),
                self.binding.map(|_| wem.source()),
            )
        } else {
            (self.bytes, false, None)
        };
        Some(SoundInput {
            bytes,
            prefetch,
            binding: self.binding,
            embedded,
        })
    }
}

pub(super) struct SoundInput<'a> {
    pub(super) bytes: &'a [u8],
    pub(super) prefetch: bool,
    binding: Option<&'a ObservedEntryBinding>,
    embedded: Option<EmbeddedWemSource>,
}

impl SoundInput<'_> {
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
