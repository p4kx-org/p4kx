// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read, Seek};
use std::ops::Range;
use std::str::Utf8Error;

use crate::bank_reader::SectionReader;
use crate::read_at::SeekSource;
use crate::{BankError, BankReadError, BankSourceSection, BankWarning, SectionId};

const WORD_LEN: usize = size_of::<u32>();
const STID_HEADER_LEN: usize = 2 * WORD_LEN;
const NAME_HEADER_LEN: usize = WORD_LEN + size_of::<u8>();
const MAX_NAME_LEN: usize = u8::MAX as usize;

struct StidHeader {
    string_type: u32,
    count: u32,
}

impl StidHeader {
    fn read(reader: &mut SectionReader<'_>) -> Result<Self, BankError> {
        Ok(Self {
            string_type: reader.u32()?,
            count: reader.u32()?,
        })
    }
}

#[derive(Debug)]
struct NameHeader {
    id: u32,
    len: u8,
}

impl NameHeader {
    fn read(reader: &mut SectionReader<'_>) -> Result<Self, BankError> {
        Ok(Self {
            id: reader.u32()?,
            len: reader.u8()?,
        })
    }
}

/// The original bytes and coordinates of one name in a physical STID section.
#[derive(Debug)]
pub struct BankStidName {
    header: NameHeader,
    ordinal: u32,
    section_ordinal: u64,
    header_offset: u64,
    bytes: [u8; MAX_NAME_LEN],
}

impl BankStidName {
    /// Returns the zero-based occurrence position in its STID section.
    #[must_use]
    pub const fn ordinal(&self) -> u64 {
        self.ordinal as u64
    }

    /// Returns the original physical STID section position.
    #[must_use]
    pub const fn section_ordinal(&self) -> u64 {
        self.section_ordinal
    }

    /// Returns the byte offset of the original ID word, followed by the one-byte name length.
    #[must_use]
    pub const fn header_offset(&self) -> u64 {
        self.header_offset
    }

    /// Returns the original bank ID word.
    #[must_use]
    pub const fn id(&self) -> u32 {
        self.header.id
    }

    /// Returns the original one-byte name length.
    #[must_use]
    pub const fn name_len(&self) -> u8 {
        self.header.len
    }

    /// Returns the exact half-open byte range of the name in the bank.
    #[must_use]
    pub const fn name_range(&self) -> Range<u64> {
        let start = self.header_offset + NAME_HEADER_LEN as u64;
        start..start + self.header.len as u64
    }

    /// Returns the original name bytes, including bytes that are invalid UTF-8.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        self.bytes.split_at(usize::from(self.header.len)).0
    }

    /// Interprets the stored bytes as UTF-8 without replacing or trimming them.
    ///
    /// # Errors
    /// Returns the UTF-8 error when the name is invalid.
    pub fn utf8(&self) -> Result<&str, Utf8Error> {
        std::str::from_utf8(self.bytes())
    }
}

/// Fixed-memory iterator over the physical names of one STID section.
pub struct BankStid<'input, R> {
    input: &'input mut SeekSource<R>,
    section: BankSourceSection,
    header: StidHeader,
    position: u64,
    ordinal: u32,
    stopped: bool,
}

impl<'input, R: Read + Seek> BankStid<'input, R> {
    pub(crate) fn open(
        input: &'input mut SeekSource<R>,
        section: &BankSourceSection,
    ) -> Result<Self, BankReadError> {
        let start = section.body_offset();
        let raw = read_prefix::<STID_HEADER_LEN>(input, section, start)?;
        let header = StidHeader::read(&mut SectionReader::new(SectionId::STID, &raw))?;
        Ok(Self {
            input,
            section: section.clone(),
            header,
            position: start + STID_HEADER_LEN as u64,
            ordinal: 0,
            stopped: false,
        })
    }
}

impl<R> BankStid<'_, R> {
    /// Returns the original string type word at the start of the section body.
    #[must_use]
    pub const fn string_type(&self) -> u32 {
        self.header.string_type
    }

    /// Returns the original declared name count after the string type word.
    #[must_use]
    pub const fn name_count(&self) -> u32 {
        self.header.count
    }

    /// Returns the exact residual byte range after reading every declared name.
    #[must_use]
    pub const fn trailing_range(&self) -> Option<Range<u64>> {
        if self.stopped || self.ordinal != self.header.count {
            None
        } else {
            Some(self.position..self.section.body_end_exclusive())
        }
    }
}

impl<R: Read + Seek> Iterator for BankStid<'_, R> {
    type Item = Result<BankStidName, BankReadError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.stopped || self.ordinal == self.header.count {
            return None;
        }
        let result = self.name();
        if result.is_err() {
            self.stopped = true;
        }
        Some(result)
    }
}

impl<R: Read + Seek> BankStid<'_, R> {
    fn name(&mut self) -> Result<BankStidName, BankReadError> {
        let raw = read_prefix::<NAME_HEADER_LEN>(self.input, &self.section, self.position)?;
        let header = NameHeader::read(&mut SectionReader::new(SectionId::STID, &raw))?;
        let start = self.position + NAME_HEADER_LEN as u64;
        let len = usize::from(header.len);
        let end = start - self.section.body_offset() + len as u64;
        if end > self.section.body_len() {
            return Err(truncated(&self.section, end).into());
        }
        let mut bytes = [0; MAX_NAME_LEN];
        self.input.read_io_at(start, bytes.split_at_mut(len).0)?;
        let result = BankStidName {
            header,
            ordinal: self.ordinal,
            section_ordinal: self.section.ordinal(),
            header_offset: self.position,
            bytes,
        };
        self.position = result.name_range().end;
        self.ordinal += 1;
        Ok(result)
    }
}

fn read_prefix<const N: usize>(
    input: &mut SeekSource<impl Read + Seek>,
    section: &BankSourceSection,
    position: u64,
) -> Result<[u8; N], BankReadError> {
    let relative = position - section.body_offset();
    let remaining = section.body_len() - relative;
    if remaining < N as u64 {
        let needed = if remaining < WORD_LEN as u64 {
            WORD_LEN
        } else {
            N
        };
        return Err(truncated(section, relative + needed as u64).into());
    }
    let mut bytes = [0; N];
    input.read_io_at(position, &mut bytes)?;
    Ok(bytes)
}

const fn truncated(section: &BankSourceSection, needed: u64) -> BankError {
    BankError::Truncated {
        section: SectionId::STID,
        len: section.body_len(),
        needed,
    }
}

pub(crate) fn parse_names(
    body: &[u8],
    base: usize,
    warnings: &mut Vec<BankWarning>,
) -> Result<Vec<(u32, Range<usize>)>, BankError> {
    let mut reader = SectionReader::new(SectionId::STID, body);
    let header = StidHeader::read(&mut reader)?;
    let count = usize::try_from(header.count).unwrap_or(usize::MAX);
    if count > reader.remaining() / NAME_HEADER_LEN {
        return Err(BankError::invalid(
            SectionId::STID,
            "it lists more names than its body holds",
        ));
    }
    let mut names = Vec::with_capacity(count);
    for _ in 0..count {
        let name = NameHeader::read(&mut reader)?;
        let range = reader.skip(usize::from(name.len))?;
        if std::str::from_utf8(body.get(range.clone()).unwrap_or_default()).is_err() {
            warnings.push(BankWarning::NonUtf8BankName { id: name.id });
            continue;
        }
        names.push((name.id, base + range.start..base + range.end));
    }
    Ok(names)
}
