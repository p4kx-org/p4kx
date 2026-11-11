// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read, Seek};
use std::ops::Range;

use crate::read_at::SeekSource;
use crate::{BankError, BankReadError, SectionId};

const MAX_VARIABLE_LENGTH_BYTES: usize = 5;
const STRING_READ_BYTES: usize = 64;

/// A scalar read and its original half-open byte range in the bank.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct BankHircField<T> {
    /// The decoded value.
    pub value: T,
    /// The original bank-relative range of the scalar.
    pub range: Range<u64>,
}

pub(crate) trait ObjectFields {
    type Error: From<BankError>;

    fn offset(&self) -> u64;
    fn remaining_bytes(&self) -> u64;
    fn read_array<const N: usize>(&mut self) -> Result<[u8; N], Self::Error>;
    fn skip_bytes(&mut self, len: u64) -> Result<(), Self::Error>;

    fn read_u8(&mut self) -> Result<BankHircField<u8>, Self::Error> {
        self.scalar(u8::from_le_bytes)
    }

    fn read_u16(&mut self) -> Result<BankHircField<u16>, Self::Error> {
        self.scalar(u16::from_le_bytes)
    }

    fn read_u32(&mut self) -> Result<BankHircField<u32>, Self::Error> {
        self.scalar(u32::from_le_bytes)
    }

    fn read_variable_length(
        &mut self,
        section: SectionId,
    ) -> Result<BankHircField<u32>, Self::Error> {
        let start = self.offset();
        let mut value = 0_u32;
        for _ in 0..MAX_VARIABLE_LENGTH_BYTES {
            let byte = self.read_u8()?.value;
            value = value.checked_mul(0x80).ok_or_else(|| {
                BankError::invalid(section, "a variable-length integer overflows")
            })? | u32::from(byte & 0x7F);
            if byte & 0x80 == 0 {
                return Ok(BankHircField {
                    value,
                    range: start..self.offset(),
                });
            }
        }
        Err(BankError::invalid(section, "a variable-length integer is longer than 5 bytes").into())
    }

    fn scalar<const N: usize, T>(
        &mut self,
        decode: impl FnOnce([u8; N]) -> T,
    ) -> Result<BankHircField<T>, Self::Error> {
        let start = self.offset();
        let value = decode(self.read_array()?);
        Ok(BankHircField {
            value,
            range: start..self.offset(),
        })
    }
}

pub(crate) fn item_bytes(count: u32, len: usize) -> Result<usize, BankError> {
    usize::try_from(count)
        .ok()
        .and_then(|count| count.checked_mul(len))
        .ok_or_else(|| BankError::invalid(SectionId::HIRC, "a list is longer than its body"))
}

pub(crate) struct SeekObjectFields<'input, R> {
    input: &'input mut SeekSource<R>,
    body_offset: u64,
    len: u64,
    position: u64,
}

impl<'input, R> SeekObjectFields<'input, R> {
    pub(crate) const fn after_id(
        input: &'input mut SeekSource<R>,
        body_offset: u64,
        len: u64,
        id_len: u64,
    ) -> Self {
        Self {
            input,
            body_offset,
            len,
            position: id_len,
        }
    }

    const fn checked_end(&self, len: u64) -> Result<u64, BankError> {
        let needed = self.position.saturating_add(len);
        if needed > self.len {
            return Err(BankError::Truncated {
                section: SectionId::HIRC,
                len: self.len,
                needed,
            });
        }
        Ok(needed)
    }
}

impl<R: Read + Seek> SeekObjectFields<'_, R> {
    pub(crate) fn read_nul_range(&mut self) -> Result<Range<u64>, BankReadError> {
        let start = self.offset();
        let mut scratch = [0; STRING_READ_BYTES];
        while self.remaining_bytes() > 0 {
            let len = usize::try_from(self.remaining_bytes())
                .unwrap_or(usize::MAX)
                .min(STRING_READ_BYTES);
            let bytes = scratch.get_mut(..len).ok_or_else(|| {
                BankError::invalid(
                    SectionId::HIRC,
                    "the marker name window exceeds its scratch buffer",
                )
            })?;
            self.input.read_io_at(self.offset(), bytes)?;
            if let Some(length) = bytes.iter().position(|&byte| byte == 0) {
                self.position += length as u64;
                let end = self.offset();
                self.position += 1;
                return Ok(start..end);
            }
            self.position += len as u64;
        }
        Err(BankError::Truncated {
            section: SectionId::HIRC,
            len: self.len,
            needed: self.len + 1,
        }
        .into())
    }
}

impl<R: Read + Seek> ObjectFields for SeekObjectFields<'_, R> {
    type Error = BankReadError;

    fn offset(&self) -> u64 {
        self.body_offset + self.position
    }

    fn remaining_bytes(&self) -> u64 {
        self.len - self.position
    }

    fn read_array<const N: usize>(&mut self) -> Result<[u8; N], Self::Error> {
        let end = self.checked_end(N as u64)?;
        let mut bytes = [0; N];
        self.input.read_io_at(self.offset(), &mut bytes)?;
        self.position = end;
        Ok(bytes)
    }

    fn skip_bytes(&mut self, len: u64) -> Result<(), Self::Error> {
        self.position = self.checked_end(len)?;
        Ok(())
    }
}
