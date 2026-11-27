// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_foundation::{ByteReader, ParseError};

use crate::record_table::RecordLayout;
use crate::rows::{DataMapping, EnumDef, PropertyDef, StructDef};
use crate::table::Row;
use crate::{DataForgeError, ValueKind};

/// The DataForge versions this reader understands.
pub const SUPPORTED_VERSIONS: [u32; 2] = [6, 8];

const PART: &str = "header";

/// The number of words before the value counts: reserved, version, two more, five table counts.
const LEADING_WORDS: usize = 9;

/// The number of leading bytes [`looks_like_dataforge`] reads: the whole header.
pub const SNIFF_LEN: usize = 4 * (LEADING_WORDS + ValueKind::COUNT + 2);

/// DataForge header declarations with fixed original positions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum DataForgeHeaderField {
    /// Format version.
    Version,
    /// Number of struct definitions.
    StructCount,
    /// Number of enum definitions.
    EnumCount,
    /// Number of Record definitions.
    RecordCount,
}

impl DataForgeHeaderField {
    /// Returns the half-open original byte span common to the supported layouts.
    #[must_use]
    pub const fn byte_span(self) -> Range<u64> {
        let word = match self {
            Self::Version => 1,
            Self::StructCount => 4,
            Self::EnumCount => 6,
            Self::RecordCount => 8,
        };
        let width = size_of::<u32>() as u64;
        word * width..(word + 1) * width
    }
}

pub(crate) fn value_pool_count_span(kind: ValueKind) -> Option<Range<u64>> {
    let ordinal = ValueKind::HEADER
        .iter()
        .position(|candidate| *candidate == kind)?;
    let word = LEADING_WORDS + ordinal;
    let width = size_of::<u32>() as u64;
    Some(word as u64 * width..(word as u64 + 1) * width)
}

/// The largest version word that is taken for a DataForge version.
const MAX_PLAUSIBLE_VERSION: u32 = u16::MAX as u32;

/// The size of one Record row of the smallest layout.
const MIN_RECORD_ROW: u64 = 32;

/// Returns whether `prefix`, the first [`SNIFF_LEN`] bytes of a file of `file_len` bytes, is a
/// DataForge header: a reserved word of zero and a version from 1 to 65,535.
///
/// Any version passes, so that a version this reader does not support is reported as such and
/// not as an unknown file; but only a supported version may claim tables larger than the file,
/// so that a truncated DataForge is reported as truncated while other files with a zero word
/// and a small number in front are not taken for one. A file shorter than the header never
/// passes.
#[must_use]
pub fn looks_like_dataforge(prefix: &[u8], file_len: u64) -> bool {
    let Some(header) = prefix.get(..SNIFF_LEN) else {
        return false;
    };
    let mut words = header
        .as_chunks::<4>()
        .0
        .iter()
        .copied()
        .map(u32::from_le_bytes);
    let mut next = || words.next().unwrap_or_default();
    let (reserved, version) = (next(), next());
    if reserved != 0 || !(1..=MAX_PLAUSIBLE_VERSION).contains(&version) {
        return false;
    }
    let _words = (next(), next());
    let mut needed = 0_u64;
    for size in [
        StructDef::SIZE as u64,
        PropertyDef::SIZE as u64,
        EnumDef::SIZE as u64,
        DataMapping::SIZE as u64,
        MIN_RECORD_ROW,
    ] {
        needed += u64::from(next()) * size;
    }
    for kind in ValueKind::HEADER {
        needed += u64::from(next()) * kind.element_len() as u64;
    }
    needed += u64::from(next()) + u64::from(next());
    needed <= file_len || SUPPORTED_VERSIONS.contains(&version)
}

/// The counts and sizes the header gives for every table.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Header {
    pub(crate) version: u32,
    pub(crate) words: [u32; 2],
    pub(crate) record_layout: RecordLayout,
    pub(crate) structs: u32,
    pub(crate) properties: u32,
    pub(crate) enums: u32,
    pub(crate) mappings: u32,
    pub(crate) records: u32,
    pub(crate) values: [u32; ValueKind::COUNT],
    pub(crate) text_len: u32,
    pub(crate) blob_len: u32,
}

impl Header {
    /// Reads the header and rejects every version but the [`SUPPORTED_VERSIONS`].
    pub(crate) fn read(reader: &mut ByteReader<'_>) -> Result<Self, DataForgeError> {
        let truncated = |source: ParseError| DataForgeError::Truncated { part: PART, source };
        let mut word = || reader.read_u32().map_err(truncated);
        let _reserved = word()?;
        let version = word()?;
        let Some(record_layout) = RecordLayout::for_version(version) else {
            return Err(DataForgeError::UnsupportedVersion { version });
        };
        let words = [word()?, word()?];
        let [structs, properties, enums, mappings, records] =
            [word()?, word()?, word()?, word()?, word()?];
        let mut values = [0; ValueKind::COUNT];
        for kind in ValueKind::HEADER {
            if let Some(slot) = values.get_mut(kind.index()) {
                *slot = word()?;
            }
        }
        Ok(Self {
            version,
            words,
            record_layout,
            structs,
            properties,
            enums,
            mappings,
            records,
            values,
            text_len: word()?,
            blob_len: word()?,
        })
    }
}

#[cfg(test)]
mod tests {
    use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct};

    use super::{DataForgeHeaderField, SUPPORTED_VERSIONS};
    use crate::DataForge;

    #[test]
    fn summary_header_spans_are_the_words_used_by_both_supported_layouts()
    -> Result<(), Box<dyn std::error::Error>> {
        use DataForgeHeaderField::{EnumCount, RecordCount, StructCount, Version};

        for version in SUPPORTED_VERSIONS {
            let bytes = DataForgeBuilder::new()
                .version(version)
                .structure(TestStruct::new("Item"))
                .structure(TestStruct::new("Extra"))
                .enumeration("Selection", ["One"])
                .record(TestRecord::new(1, "Item", "Item.One", "records/one.xml"))
                .record(TestRecord::new(2, "Item", "Item.Two", "records/two.xml"))
                .record(TestRecord::new(
                    3,
                    "Item",
                    "Item.Three",
                    "records/three.xml",
                ))
                .build();
            let forge = DataForge::open(&bytes)?;
            for (field, expected) in [
                (Version, forge.version()),
                (StructCount, u32::try_from(forge.struct_count())?),
                (EnumCount, u32::try_from(forge.enum_count())?),
                (RecordCount, u32::try_from(forge.record_count())?),
            ] {
                let span = field.byte_span();
                let word = bytes
                    .get(usize::try_from(span.start)?..usize::try_from(span.end)?)
                    .ok_or("header word")?;
                assert_eq!(u32::from_le_bytes(word.try_into()?), expected, "{field:?}");
            }
        }
        Ok(())
    }
}
