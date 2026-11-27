// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Read, Seek};
use std::sync::Arc;

use thiserror::Error;

use crate::bank_parse;
use crate::bank_source_didx;
use crate::bank_structure::{self, SECTION_HEADER_LEN, SeenSections};
use crate::read_at::SeekSource;
use crate::{
    BankDidx, BankDidxEntry, BankError, BankHeader, BankHirc, BankMediaInfo, BankStid, SectionId,
};

const HEADER_FIELDS_LEN: usize = 12;

/// A failure to inspect a seekable BNK source.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum BankReadError {
    /// A bank declaration is missing, malformed or unsupported.
    #[error(transparent)]
    Format(#[from] BankError),
    /// The supplied input failed a physical read or seek.
    #[error("cannot read the soundbank source")]
    Io(#[from] io::Error),
}

/// Fixed-memory inspection of physical sections in a seekable BNK source.
pub struct BankSource<R> {
    input: SeekSource<R>,
    owner: Arc<()>,
}

/// The raw words and physical BKHD section of one observed header.
#[derive(Debug)]
pub struct BankObservedHeader {
    section: BankSourceSection,
    header: BankHeader,
}

impl BankObservedHeader {
    /// Returns the original version, bank ID and language words.
    #[must_use]
    pub const fn header(&self) -> BankHeader {
        self.header
    }

    /// Returns the physical section that supplied the header words.
    #[must_use]
    pub const fn section(&self) -> &BankSourceSection {
        &self.section
    }
}

/// An independent physical directory cursor bound to its original bank read.
pub struct BankSectionCursor {
    owner: Arc<()>,
    seen: SeenSections,
    position: u64,
    ordinal: u64,
    stopped: bool,
}

/// One physical BNK section, including repeated recognized section occurrences.
#[derive(Debug, Clone)]
pub struct BankSourceSection {
    pub(crate) owner: Arc<()>,
    id: SectionId,
    ordinal: u64,
    header_offset: u64,
    body_len: u64,
    duplicate_unique: bool,
}

impl BankSourceSection {
    /// Returns the original four-byte section tag.
    #[must_use]
    pub const fn id(&self) -> SectionId {
        self.id
    }

    /// Returns the zero-based occurrence position in the physical bank directory.
    #[must_use]
    pub const fn ordinal(&self) -> u64 {
        self.ordinal
    }

    /// Returns the byte offset of the original section header in the bank.
    #[must_use]
    pub const fn header_offset(&self) -> u64 {
        self.header_offset
    }

    /// Returns the byte offset of the section body in the bank.
    #[must_use]
    pub const fn body_offset(&self) -> u64 {
        self.header_offset + SECTION_HEADER_LEN as u64
    }

    /// Returns the declared section body length.
    #[must_use]
    pub const fn body_len(&self) -> u64 {
        self.body_len
    }

    /// Returns the exclusive bank-relative end of the section body.
    #[must_use]
    pub const fn body_end_exclusive(&self) -> u64 {
        self.body_offset() + self.body_len
    }

    /// Returns whether a section that must be unique occurred earlier.
    #[must_use]
    pub const fn is_duplicate_unique(&self) -> bool {
        self.duplicate_unique
    }

    pub(crate) fn require(&self, owner: &Arc<()>, expected: SectionId) -> Result<(), BankError> {
        if !Arc::ptr_eq(&self.owner, owner) {
            return Err(BankError::invalid(
                expected,
                "the section belongs to another soundbank read",
            ));
        }
        if self.id != expected {
            return Err(BankError::invalid(
                expected,
                "the supplied section has another tag",
            ));
        }
        Ok(())
    }
}

impl<R: Read + Seek> BankSource<R> {
    /// Opens a physical bank within a caller-supplied exact input length.
    ///
    /// # Errors
    /// Returns an error when the signature is not BKHD or the input cannot be read.
    pub fn open(reader: R, byte_length: u64) -> Result<Self, BankReadError> {
        let mut input = SeekSource::new(reader, byte_length);
        let mut tag = [0; 4];
        if byte_length < tag.len() as u64 {
            return Err(BankError::NotSoundbank.into());
        }
        input.read_io_at(0, &mut tag)?;
        if SectionId(tag) != SectionId::BKHD {
            return Err(BankError::NotSoundbank.into());
        }
        Ok(Self {
            input,
            owner: Arc::new(()),
        })
    }

    /// Starts an independently resumable scan of the original physical sections.
    #[must_use]
    pub fn section_cursor(&self) -> BankSectionCursor {
        BankSectionCursor {
            owner: Arc::clone(&self.owner),
            seen: SeenSections::default(),
            position: 0,
            ordinal: 0,
            stopped: false,
        }
    }

    /// Returns the next physical section, retaining recognized duplicate occurrences.
    /// A failed read or invalid section extent is the cursor's final item.
    pub fn next_section(
        &mut self,
        cursor: &mut BankSectionCursor,
    ) -> Option<Result<BankSourceSection, BankReadError>> {
        if cursor.stopped {
            return None;
        }
        let result = self.read_section(cursor);
        if !matches!(result, Ok(Some(_))) {
            cursor.stopped = true;
        }
        result.transpose()
    }

    /// Visits all physical sections in stored order.
    pub fn sections(
        &mut self,
    ) -> impl Iterator<Item = Result<BankSourceSection, BankReadError>> + '_ {
        let mut cursor = self.section_cursor();
        std::iter::from_fn(move || self.next_section(&mut cursor))
    }

    /// Decodes the header of the first physical BKHD section.
    ///
    /// # Errors
    /// Returns a malformed or unsupported header, or the original input error.
    pub fn read_header(&mut self) -> Result<BankHeader, BankReadError> {
        let mut cursor = self.section_cursor();
        let section = self
            .next_section(&mut cursor)
            .ok_or(BankError::NotSoundbank)??;
        self.header_for(&section, bank_parse::parse_header)
    }

    /// Reads the three original header words from the selected physical BKHD occurrence.
    ///
    /// # Errors
    /// Returns a foreign or wrong section, malformed header, or input error.
    pub fn read_header_for(
        &mut self,
        section: &BankSourceSection,
    ) -> Result<BankHeader, BankReadError> {
        Ok(self.read_observed_header_for(section)?.header())
    }

    /// Reads the raw BKHD words with their original physical section authority.
    ///
    /// # Errors
    /// Returns a foreign or wrong section, malformed header, or input error.
    pub fn read_observed_header_for(
        &mut self,
        section: &BankSourceSection,
    ) -> Result<BankObservedHeader, BankReadError> {
        let header = self.header_for(section, bank_parse::raw_header)?;
        Ok(BankObservedHeader {
            section: section.clone(),
            header,
        })
    }

    fn header_for(
        &mut self,
        section: &BankSourceSection,
        parse: fn(&[u8]) -> Result<BankHeader, BankError>,
    ) -> Result<BankHeader, BankReadError> {
        section.require(&self.owner, SectionId::BKHD)?;
        let mut fields = [0; HEADER_FIELDS_LEN];
        let count =
            usize::try_from(section.body_len.min(HEADER_FIELDS_LEN as u64)).map_err(|_| {
                BankError::invalid(SectionId::BKHD, "the header fields do not fit in memory")
            })?;
        let fields = fields.get_mut(..count).ok_or_else(|| {
            BankError::invalid(SectionId::BKHD, "the header fields are out of bounds")
        })?;
        self.input.read_io_at(section.body_offset(), fields)?;
        Ok(parse(fields)?)
    }

    /// Visits raw DIDX triplets in one original physical section.
    ///
    /// # Errors
    /// Returns a foreign or wrong section. A later failure ends the iterator.
    pub fn didx(&mut self, section: &BankSourceSection) -> Result<BankDidx<'_, R>, BankReadError> {
        section.require(&self.owner, SectionId::DIDX)?;
        Ok(BankDidx::new(&mut self.input, section))
    }

    /// Visits raw HIRC object occurrences in one original physical section.
    ///
    /// # Errors
    /// Returns a foreign or wrong section, malformed count or input read failure.
    pub fn hirc(&mut self, section: &BankSourceSection) -> Result<BankHirc<'_, R>, BankReadError> {
        section.require(&self.owner, SectionId::HIRC)?;
        BankHirc::open(&mut self.input, section)
    }

    /// Visits the original names in one physical STID section.
    ///
    /// # Errors
    /// Returns a foreign or wrong section, malformed header, or input read failure.
    pub fn stid(&mut self, section: &BankSourceSection) -> Result<BankStid<'_, R>, BankReadError> {
        section.require(&self.owner, SectionId::STID)?;
        BankStid::open(&mut self.input, section)
    }

    /// Classifies the verified range of one original triplet using its bounded media prefix.
    ///
    /// # Errors
    /// Returns a foreign section or triplet, invalid DATA range, or original input failure.
    pub fn inspect_media(
        &mut self,
        entry: &BankDidxEntry,
        data: &BankSourceSection,
    ) -> Result<BankMediaInfo, BankReadError> {
        bank_source_didx::inspect_media(&mut self.input, &self.owner, entry, data)
    }

    fn read_section(
        &mut self,
        cursor: &mut BankSectionCursor,
    ) -> Result<Option<BankSourceSection>, BankReadError> {
        if !Arc::ptr_eq(&self.owner, &cursor.owner) {
            return Err(BankError::invalid(
                SectionId::BKHD,
                "the directory cursor belongs to another soundbank read",
            )
            .into());
        }
        let remaining = self.input.source_length() - cursor.position;
        if remaining == 0 {
            return Ok(None);
        }
        let mut prefix = [0; SECTION_HEADER_LEN];
        let count = usize::try_from(remaining.min(SECTION_HEADER_LEN as u64)).map_err(|_| {
            BankError::invalid(SectionId::BKHD, "the section header does not fit in memory")
        })?;
        let prefix = prefix.get_mut(..count).ok_or_else(|| {
            BankError::invalid(SectionId::BKHD, "the section header is out of bounds")
        })?;
        self.input.read_io_at(cursor.position, prefix)?;
        let layout = bank_structure::section_layout(prefix, remaining)?;
        let section = BankSourceSection {
            owner: Arc::clone(&self.owner),
            id: layout.id,
            ordinal: cursor.ordinal,
            header_offset: cursor.position,
            body_len: layout.body_len,
            duplicate_unique: cursor.seen.repeat(layout.id),
        };
        cursor.position = section.body_end_exclusive();
        cursor.ordinal += 1;
        Ok(Some(section))
    }
}
