// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use crate::bank_parse::{self, MediaRange, SectionRange};
use crate::media_class;
use crate::{AudioError, BankError, BankWarning, HircObject, MediaClass, SectionId, Wem};

/// The fields of the `BKHD` section this reader decodes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BankHeader {
    /// The bank version, which selects the layout of the other sections.
    pub version: u32,
    /// The soundbank ID, the hash of its name.
    pub id: u32,
    /// The language ID: an enum value up to bank version 122, from version 123 on the FNV hash of
    /// the language name.
    pub language_id: u32,
}

impl BankHeader {
    /// Returns whether the decoded bank reader supports this header's version.
    #[must_use]
    pub const fn is_supported_version(self) -> bool {
        bank_parse::supported_version(self.version)
    }
}

/// One section of a soundbank with its raw body.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Section<'a> {
    /// The section tag.
    pub id: SectionId,
    /// The body after the 8-byte section header.
    pub body: &'a [u8],
}

/// A WEM embedded in a soundbank: its ID and its bytes, a slice of the bank.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EmbeddedWem<'a> {
    id: u32,
    offset: u64,
    bytes: &'a [u8],
}

impl<'a> EmbeddedWem<'a> {
    /// Returns the WEM ID.
    #[must_use]
    pub const fn id(&self) -> u32 {
        self.id
    }

    /// Returns the offset of the WEM in the `DATA` section.
    #[must_use]
    pub const fn offset(&self) -> u64 {
        self.offset
    }

    /// Returns the bytes of the WEM, as a loose WEM file holds them.
    #[must_use]
    pub const fn bytes(&self) -> &'a [u8] {
        self.bytes
    }

    /// Returns what the media holds: a WEM, MIDI, plug-in data or something unknown.
    #[must_use]
    pub fn class(&self) -> MediaClass {
        MediaClass::of(self.bytes)
    }

    /// Returns the length the RIFF or RIFX header declares for the whole WEM, if there is one.
    #[must_use]
    pub fn declared_len(&self) -> Option<u64> {
        media_class::declared_len(self.bytes)
    }

    /// Returns whether the bank holds only the prefetched start of a streamed WEM, whose whole
    /// file is a loose WEM named by the same ID.
    #[must_use]
    pub fn is_prefetch(&self) -> bool {
        media_class::is_prefetch(self.declared_len(), self.bytes.len() as u64)
    }

    /// Parses the WEM like a loose one, borrowing the bank's bytes.
    ///
    /// # Errors
    ///
    /// Returns an [`AudioError`] when the bytes are no valid WEM, e.g. plug-in media or the
    /// truncated start of a prefetched WEM.
    pub fn parse(&self) -> Result<Wem<&'a [u8]>, AudioError> {
        Wem::parse(self.bytes)
    }
}

/// A Wwise BNK soundbank parsed from bytes the caller holds: `BKHD`, `DIDX`, `DATA`, `HIRC` and
/// `STID` are decoded, every section is kept raw too.
#[derive(Debug, Clone)]
pub struct Soundbank<B> {
    bytes: B,
    header: BankHeader,
    sections: Vec<SectionRange>,
    media: Vec<MediaRange>,
    objects: Vec<HircObject>,
    object_index: Vec<(u32, usize)>,
    names: Vec<(u32, Range<usize>)>,
    warnings: Vec<BankWarning>,
}

impl<B: AsRef<[u8]>> Soundbank<B> {
    /// Parses a little-endian soundbank, checking every `DIDX` entry against `DATA`.
    ///
    /// A quirk that leaves the rest of the bank readable, e.g. bytes after the last `HIRC`
    /// object, a repeated `DIDX` ID, `DIDX` without `DATA` or a non-UTF-8 name, is worked around
    /// and listed by [`Soundbank::warnings`].
    ///
    /// # Errors
    ///
    /// Returns [`BankError::NotSoundbank`] when the data does not start with `BKHD`, and another
    /// [`BankError`] naming the section when a section is truncated, repeated or invalid.
    pub fn parse(bytes: B) -> Result<Self, BankError> {
        let parsed = bank_parse::parse(bytes.as_ref())?;
        let mut object_index: Vec<(u32, usize)> = parsed
            .objects
            .iter()
            .enumerate()
            .map(|(position, object)| (object.id, position))
            .collect();
        object_index.sort_unstable();
        Ok(Self {
            object_index,
            header: parsed.header,
            sections: parsed.sections,
            media: parsed.media,
            objects: parsed.objects,
            names: parsed.names,
            warnings: parsed.warnings,
            bytes,
        })
    }

    /// Returns every section in file order.
    pub fn sections(&self) -> impl Iterator<Item = Section<'_>> {
        self.sections.iter().map(|section| Section {
            id: section.id,
            body: self.slice(&section.body),
        })
    }

    /// Returns the first section with the tag `id`.
    #[must_use]
    pub fn section(&self, id: SectionId) -> Option<Section<'_>> {
        self.sections().find(|section| section.id == id)
    }

    /// Returns the embedded WEMs in ascending ID order.
    pub fn wems(&self) -> impl ExactSizeIterator<Item = EmbeddedWem<'_>> {
        self.media.iter().map(|media| self.embedded(media))
    }

    /// Returns the embedded WEM `id`.
    #[must_use]
    pub fn wem(&self, id: u32) -> Option<EmbeddedWem<'_>> {
        let index = self
            .media
            .binary_search_by_key(&id, |media| media.id)
            .ok()?;
        self.media.get(index).map(|media| self.embedded(media))
    }

    /// Returns the raw body of a `HIRC` object of this bank, its ID included.
    #[must_use]
    pub fn object_body(&self, object: &HircObject) -> &[u8] {
        self.slice(&object.body)
    }

    /// Returns the soundbank names of the `STID` section with their IDs, in file order.
    pub fn bank_names(&self) -> impl Iterator<Item = (u32, &str)> {
        self.names.iter().map(|(id, name)| {
            (
                *id,
                std::str::from_utf8(self.slice(name)).unwrap_or_default(),
            )
        })
    }

    /// Returns the bytes the bank was parsed from.
    pub fn into_inner(self) -> B {
        self.bytes
    }

    fn embedded(&self, media: &MediaRange) -> EmbeddedWem<'_> {
        EmbeddedWem {
            id: media.id,
            offset: media.offset,
            bytes: self.slice(&media.range),
        }
    }

    fn slice(&self, range: &Range<usize>) -> &[u8] {
        self.bytes.as_ref().get(range.clone()).unwrap_or_default()
    }
}

impl<B> Soundbank<B> {
    /// Returns the decoded `BKHD` fields.
    #[must_use]
    pub const fn header(&self) -> &BankHeader {
        &self.header
    }

    /// Returns the quirks the parser worked around, in the order it met them.
    #[must_use]
    pub fn warnings(&self) -> &[BankWarning] {
        &self.warnings
    }

    /// Returns the objects of the `HIRC` section in file order; empty without one.
    #[must_use]
    pub fn objects(&self) -> &[HircObject] {
        &self.objects
    }

    /// Returns the `HIRC` object `id`, the first in file order when several share it.
    #[must_use]
    pub fn object(&self, id: u32) -> Option<&HircObject> {
        let first = self.object_index.partition_point(|&(key, _)| key < id);
        match self.object_index.get(first) {
            Some(&(key, position)) if key == id => self.objects.get(position),
            _ => None,
        }
    }
}
