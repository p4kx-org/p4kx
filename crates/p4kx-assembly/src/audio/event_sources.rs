// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_archive::{Archive, ObservedEntry, ObservedEntryBinding};
use p4kx_audio::HircObject;

use super::{AudioIndex, IndexedSoundbank};

/// An original HIRC Event declaration in a bound soundbank Entry.
#[derive(Debug, Clone, Copy)]
pub struct SoundbankEventSource<'index, 'archive> {
    soundbank: &'index IndexedSoundbank,
    entry: ObservedEntry<'archive>,
    declaration: &'index EventDeclaration,
    version: Option<&'index Range<usize>>,
}

impl<'index, 'archive> SoundbankEventSource<'index, 'archive> {
    /// Returns the soundbank containing this Event declaration.
    #[must_use]
    pub const fn soundbank(&self) -> &'index IndexedSoundbank {
        self.soundbank
    }

    /// Returns the physically bound Archive Entry.
    #[must_use]
    pub const fn entry(&self) -> ObservedEntry<'archive> {
        self.entry
    }

    /// Returns the original occurrence in the bank's parsed HIRC object order.
    #[must_use]
    pub const fn object_ordinal(&self) -> usize {
        self.declaration.ordinal
    }

    /// Returns the bank-relative range of the original object-kind byte.
    #[must_use]
    pub fn kind_bytes(&self) -> Range<usize> {
        self.declaration.kind.clone()
    }

    /// Returns the bank-relative range of the original Event ID word.
    #[must_use]
    pub fn id_bytes(&self) -> Range<usize> {
        self.declaration.id_bytes.clone()
    }

    /// Returns the bank-relative version word used to classify the object.
    #[must_use]
    pub fn version_bytes(&self) -> Option<Range<usize>> {
        self.version.cloned()
    }
}

#[derive(Debug)]
pub(super) struct EventDeclaration {
    pub(super) id: u32,
    ordinal: usize,
    kind: Range<usize>,
    id_bytes: Range<usize>,
}

impl EventDeclaration {
    pub(super) const fn of(ordinal: usize, object: &HircObject) -> Self {
        Self {
            id: object.id,
            ordinal,
            kind: object.kind_byte_range(),
            id_bytes: object.id_byte_range(),
        }
    }
}

#[derive(Debug)]
pub(super) struct BankEventIndex {
    owner: ObservedEntryBinding,
    version: Option<Range<usize>>,
    declarations: Box<[EventDeclaration]>,
}

impl BankEventIndex {
    pub(super) fn new(
        entry: ObservedEntry<'_>,
        version: Option<Range<usize>>,
        mut declarations: Vec<EventDeclaration>,
    ) -> Self {
        declarations.sort_unstable_by_key(|declaration| (declaration.id, declaration.ordinal));
        Self {
            owner: entry.binding(),
            version,
            declarations: declarations.into_boxed_slice(),
        }
    }

    fn entry<'archive>(
        &self,
        archive: &'archive Archive,
        soundbank: &IndexedSoundbank,
    ) -> Option<ObservedEntry<'archive>> {
        let entry = archive.observed_entry(archive.entries().get(soundbank.entry_position())?)?;
        self.owner.is_same_entry(&entry.binding()).then_some(entry)
    }

    fn declarations(&self, id: u32) -> &[EventDeclaration] {
        let start = self.declarations.partition_point(|row| row.id < id);
        let end = self.declarations.partition_point(|row| row.id <= id);
        self.declarations.get(start..end).unwrap_or_default()
    }
}

impl AudioIndex {
    /// Returns the original Event declarations in the given banks, in path and object order.
    pub fn event_soundbank_sources<'index, 'archive>(
        &'index self,
        archive: &'archive Archive,
        id: u32,
    ) -> impl Iterator<Item = SoundbankEventSource<'index, 'archive>> + use<'index, 'archive> {
        self.event_holders(id)
            .iter()
            .filter_map(move |&position| {
                let position = usize::try_from(position).ok()?;
                let soundbank = self.soundbanks().get(position)?;
                let source = self.event_sources.get(position)?.as_ref()?;
                Some((soundbank, source, source.entry(archive, soundbank)?))
            })
            .flat_map(move |(soundbank, source, entry)| {
                source
                    .declarations(id)
                    .iter()
                    .map(move |declaration| SoundbankEventSource {
                        soundbank,
                        entry,
                        declaration,
                        version: source.version.as_ref(),
                    })
            })
    }
}
