// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::{Archive, EntryIssue, EntryRecordFormat, MetadataRange, ObservedEntry};

const COMPRESSED_SIZE_OFFSET: u64 = 10;
const DATA_OFFSET_OFFSET: u64 = 26;
const PAYLOAD_END_OFFSET: u64 = 64;
const WORD_BYTES: u64 = size_of::<u64>() as u64;

/// One cached v2 bound operand and the source-relative bytes that declared it.
#[derive(Debug, Clone, Copy)]
pub struct ObservedV2BoundWord {
    value: u64,
    range: MetadataRange,
}

impl ObservedV2BoundWord {
    /// Returns the original little-endian word.
    #[must_use]
    pub const fn value(self) -> u64 {
        self.value
    }

    /// Returns the exact bytes of the word in its owning Archive source.
    #[must_use]
    pub const fn range(self) -> MetadataRange {
        self.range
    }
}

/// Operands of a reported root v2 data extent failure.
#[derive(Debug, Clone, Copy)]
pub struct ObservedV2DataOutsideFile<'archive> {
    entry: ObservedEntry<'archive>,
    data_offset: ObservedV2BoundWord,
    compressed_size: ObservedV2BoundWord,
    payload_end: ObservedV2BoundWord,
}

impl<'archive> ObservedV2DataOutsideFile<'archive> {
    /// Returns the admitted physical Entry that supplied the two directory operands.
    #[must_use]
    pub const fn entry(self) -> ObservedEntry<'archive> {
        self.entry
    }

    /// Returns the stored payload start offset.
    #[must_use]
    pub const fn data_offset(self) -> ObservedV2BoundWord {
        self.data_offset
    }

    /// Returns the stored payload byte count.
    #[must_use]
    pub const fn compressed_size(self) -> ObservedV2BoundWord {
        self.compressed_size
    }

    /// Returns the data boundary from the admitted v2 trailer.
    #[must_use]
    pub const fn payload_end(self) -> ObservedV2BoundWord {
        self.payload_end
    }
}

impl Archive {
    /// Borrows the cached operands for a retained root v2 `DataOutsideFile` issue.
    ///
    /// The position indexes [`Self::issues`]. The words describe the normal v2 directory
    /// check and do not establish payload integrity.
    #[must_use]
    pub fn observed_v2_data_outside_file(
        &self,
        issue_position: usize,
    ) -> Option<ObservedV2DataOutsideFile<'_>> {
        self.root_directory_entry_count()?;
        let (index, EntryIssue::DataOutsideFile) = self.issues().get(issue_position)? else {
            return None;
        };
        let trailer = self.metadata().v2_directory()?;
        let entry = self.entries().get(*index)?;
        let record = entry.metadata();
        if record.record_format() != EntryRecordFormat::P4kV2 {
            return None;
        }
        let range = record.record_range();
        Some(ObservedV2DataOutsideFile {
            entry: self.observed_entry(entry)?,
            data_offset: word(entry.data_offset, range, DATA_OFFSET_OFFSET)?,
            compressed_size: word(entry.compressed_size, range, COMPRESSED_SIZE_OFFSET)?,
            payload_end: word(trailer.payload_end(), trailer.trailer(), PAYLOAD_END_OFFSET)?,
        })
    }
}

fn word(value: u64, section: MetadataRange, relative: u64) -> Option<ObservedV2BoundWord> {
    let offset = section.offset().checked_add(relative)?;
    let range = MetadataRange::checked(offset, WORD_BYTES, section.end()).ok()?;
    Some(ObservedV2BoundWord { value, range })
}
