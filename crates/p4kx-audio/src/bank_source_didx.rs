// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read, Seek};
use std::ops::Range;
use std::sync::Arc;

use crate::bank_reader::SectionReader;
use crate::bank_structure::{DIDX_ENTRY_LEN, MediaTriplet};
use crate::media_class;
use crate::read_at::SeekSource;
use crate::{
    BankDeclaredLengthSource, BankError, BankMediaClassSource, BankReadError, BankSourceSection,
    MediaClass, SectionId,
};

/// One original DIDX triplet, retaining repeated media IDs and stored order.
#[derive(Debug, Clone)]
pub struct BankDidxEntry {
    pub(crate) owner: Arc<()>,
    ordinal: u64,
    section_ordinal: u64,
    triplet_offset: u64,
    triplet: MediaTriplet,
}

impl BankDidxEntry {
    /// Returns the zero-based triplet position in the DIDX section.
    #[must_use]
    pub const fn ordinal(&self) -> u64 {
        self.ordinal
    }

    /// Returns the original physical DIDX section position.
    #[must_use]
    pub const fn section_ordinal(&self) -> u64 {
        self.section_ordinal
    }

    /// Returns the bank-relative start of the twelve-byte triplet.
    #[must_use]
    pub const fn triplet_offset(&self) -> u64 {
        self.triplet_offset
    }

    /// Returns the original media identifier word.
    #[must_use]
    pub const fn media_id(&self) -> u32 {
        self.triplet.id
    }

    /// Returns the authored offset relative to a DATA body.
    #[must_use]
    pub const fn offset(&self) -> u32 {
        self.triplet.offset
    }

    /// Returns the authored encoded byte length.
    #[must_use]
    pub const fn length(&self) -> u32 {
        self.triplet.length
    }

    /// Resolves this original range within an observed DATA section of the same bank read.
    ///
    /// # Errors
    /// Returns a foreign or wrong section, or a triplet that extends beyond DATA.
    pub fn range_in(&self, data: &BankSourceSection) -> Result<Range<u64>, BankReadError> {
        data.require(&self.owner, SectionId::DATA)?;
        Ok(self.triplet.range_in(data.body_offset(), data.body_len())?)
    }
}

/// Classification of an observed DIDX range from its actual bounded prefix.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct BankMediaInfo {
    /// The verified half-open byte range in the bank.
    pub range: Range<u64>,
    /// The content class of the retained media prefix.
    pub class: MediaClass,
    /// Original decision inputs used to classify the media.
    pub class_source: BankMediaClassSource,
    /// The RIFF or RIFX extent when its size word was readable.
    pub declared_len: Option<u64>,
    /// Original decision inputs used to determine the declared extent.
    pub declared_len_source: BankDeclaredLengthSource,
    /// Whether the declared media extent exceeds the retained range.
    pub prefetch: bool,
    /// Actual bytes read for classification, at most twelve.
    pub prefix_len: u8,
}

/// Fixed-memory iterator over the original physical triplets of one DIDX section.
pub struct BankDidx<'input, R> {
    input: &'input mut SeekSource<R>,
    section: BankSourceSection,
    position: u64,
    ordinal: u64,
    stopped: bool,
}

impl<'input, R> BankDidx<'input, R> {
    pub(crate) fn new(input: &'input mut SeekSource<R>, section: &BankSourceSection) -> Self {
        Self {
            input,
            section: section.clone(),
            position: section.body_offset(),
            ordinal: 0,
            stopped: false,
        }
    }
}

impl<R: Read + Seek> Iterator for BankDidx<'_, R> {
    type Item = Result<BankDidxEntry, BankReadError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.stopped || self.position == self.section.body_end_exclusive() {
            return None;
        }
        let result = self.triplet();
        if result.is_err() {
            self.stopped = true;
        }
        Some(result)
    }
}

impl<R: Read + Seek> BankDidx<'_, R> {
    /// Classifies the verified DATA range of a triplet from this original cursor's section.
    ///
    /// # Errors
    /// Returns a foreign triplet or section, invalid DATA range, or original input failure.
    pub fn inspect_media(
        &mut self,
        entry: &BankDidxEntry,
        data: &BankSourceSection,
    ) -> Result<BankMediaInfo, BankReadError> {
        self.require_entry(entry)?;
        inspect_media(self.input, &self.section.owner, entry, data)
    }

    /// Opens a zero-based byte window over the triplet in the selected physical DATA section.
    /// Reads and seeks stay within the original range.
    ///
    /// # Errors
    /// Returns a foreign triplet or section, or an invalid DATA range.
    pub fn media_reader(
        &mut self,
        entry: &BankDidxEntry,
        data: &BankSourceSection,
    ) -> Result<impl Read + Seek + '_, BankReadError> {
        self.require_entry(entry)?;
        let range = media_range(&self.section.owner, entry, data)?;
        Ok(self.input.window(range)?)
    }

    fn require_entry(&self, entry: &BankDidxEntry) -> Result<(), BankReadError> {
        if entry.section_ordinal != self.section.ordinal() {
            return Err(BankError::invalid(
                SectionId::DIDX,
                "the triplet belongs to another DIDX section",
            )
            .into());
        }
        Ok(())
    }

    fn triplet(&mut self) -> Result<BankDidxEntry, BankReadError> {
        if self.section.body_end_exclusive() - self.position < DIDX_ENTRY_LEN as u64 {
            return Err(BankError::invalid(
                SectionId::DIDX,
                "its size is not a multiple of 12 bytes",
            )
            .into());
        }
        let mut raw = [0; DIDX_ENTRY_LEN];
        self.input.read_io_at(self.position, &mut raw)?;
        let triplet = MediaTriplet::read(&mut SectionReader::new(SectionId::DIDX, &raw))?;
        let entry = BankDidxEntry {
            owner: Arc::clone(&self.section.owner),
            ordinal: self.ordinal,
            section_ordinal: self.section.ordinal(),
            triplet_offset: self.position,
            triplet,
        };
        self.position += DIDX_ENTRY_LEN as u64;
        self.ordinal += 1;
        Ok(entry)
    }
}

pub(crate) fn inspect_media<R: Read + Seek>(
    input: &mut SeekSource<R>,
    owner: &Arc<()>,
    entry: &BankDidxEntry,
    data: &BankSourceSection,
) -> Result<BankMediaInfo, BankReadError> {
    let range = media_range(owner, entry, data)?;
    let available = range.end - range.start;
    let mut prefix = [0; 12];
    let prefix_len = u8::try_from(available.min(prefix.len() as u64)).map_err(|_| {
        BankError::invalid(SectionId::DATA, "the media prefix does not fit in memory")
    })?;
    let bytes = prefix
        .get_mut(..usize::from(prefix_len))
        .ok_or_else(|| BankError::invalid(SectionId::DATA, "the media prefix is out of bounds"))?;
    input.read_io_at(range.start, bytes)?;
    let (class, class_source) = MediaClass::inspect(bytes);
    let (declared_len, declared_len_source) = media_class::declared_len_with_source(bytes);
    Ok(BankMediaInfo {
        range,
        class,
        class_source,
        declared_len,
        declared_len_source,
        prefetch: media_class::is_prefetch(declared_len, available),
        prefix_len,
    })
}

fn media_range(
    owner: &Arc<()>,
    entry: &BankDidxEntry,
    data: &BankSourceSection,
) -> Result<Range<u64>, BankReadError> {
    if !Arc::ptr_eq(owner, &entry.owner) {
        return Err(BankError::invalid(
            SectionId::DIDX,
            "the triplet belongs to another soundbank read",
        )
        .into());
    }
    entry.range_in(data)
}
