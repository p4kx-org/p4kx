// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{HashMap, HashSet};

use crate::audio::{SoundVariant, WemLocation};

/// The physical media read for an exported WEM, within its opening Archive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum WemExportSource {
    /// One checked embedded span in an Archive soundbank.
    Soundbank {
        /// The position of the bank in the Archive directory.
        entry: usize,
        /// The offset from the start of the bank's DATA body.
        offset: u64,
        /// The length of the embedded span in bytes.
        length: u64,
    },
    /// One loose WEM Archive Entry.
    Loose {
        /// The position of the media in the Archive directory.
        entry: usize,
    },
}

impl WemExportSource {
    fn from_location(location: WemLocation<'_>) -> Option<Self> {
        match location {
            WemLocation::Soundbank {
                soundbank,
                offset,
                len,
            } => Some(Self::Soundbank {
                entry: soundbank.entry_position(),
                offset,
                length: len,
            }),
            WemLocation::Entry { entry, .. } => Some(Self::Loose {
                entry: usize::try_from(entry).ok()?,
            }),
        }
    }

    pub(super) const fn entry(self) -> SourceEntry {
        match self {
            Self::Soundbank { entry, .. } => SourceEntry::Soundbank(entry),
            Self::Loose { entry } => SourceEntry::Loose(entry),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum SourceEntry {
    Soundbank(usize),
    Loose(usize),
}

struct Candidate<'archive> {
    source: WemExportSource,
    language: Option<&'archive str>,
    prefetch: bool,
}

impl<'archive> Candidate<'archive> {
    fn for_variant(variant: &SoundVariant<'archive>, id: u32) -> Option<Self> {
        let &location = variant.location()?;
        let prefetch = matches!(location, WemLocation::Soundbank { soundbank, .. }
            if soundbank.has_prefetched_wem(id));
        Some(Self {
            source: WemExportSource::from_location(location)?,
            language: location.language(),
            prefetch,
        })
    }
}

pub(in crate::audio) struct ResolvedMedia<'archive> {
    whole: HashMap<u32, Vec<Candidate<'archive>>>,
}

impl<'archive> ResolvedMedia<'archive> {
    pub(in crate::audio) fn new(variants: &[SoundVariant<'archive>]) -> Self {
        let mut whole: HashMap<u32, Vec<Candidate<'archive>>> = HashMap::new();
        let mut checked = HashSet::new();
        for variant in variants {
            let Some((id, candidate)) = variant.wem_id().and_then(|id| {
                Candidate::for_variant(variant, id).map(|candidate| (id, candidate))
            }) else {
                continue;
            };
            if !candidate.prefetch && checked.insert((id, candidate.source)) {
                whole.entry(id).or_default().push(candidate);
            }
        }
        Self { whole }
    }

    pub(in crate::audio) fn source(
        &self,
        variant: &SoundVariant<'archive>,
        id: u32,
    ) -> Option<WemExportSource> {
        let candidate = Candidate::for_variant(variant, id)?;
        if candidate.prefetch
            && let Some(whole) = self.unique_whole(id, candidate.language)
        {
            return Some(whole);
        }
        Some(candidate.source)
    }

    fn unique_whole(&self, id: u32, language: Option<&str>) -> Option<WemExportSource> {
        let mut matching =
            self.whole
                .get(&id)?
                .iter()
                .filter(|candidate| match (language, candidate.language) {
                    (Some(first), Some(second)) => first.eq_ignore_ascii_case(second),
                    (None, None) => true,
                    _ => false,
                });
        let found = matching.next()?;
        matching.next().is_none().then_some(found.source)
    }
}
