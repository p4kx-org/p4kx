// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::bank_object_fields::ObjectFields;
use crate::{BankError, SectionId};

/// Reads little-endian fields from the body of one section, failing with a [`BankError`] that
/// names the section.
#[derive(Debug, Clone)]
pub(crate) struct SectionReader<'a> {
    section: SectionId,
    body: &'a [u8],
    position: usize,
}

impl<'a> SectionReader<'a> {
    pub(crate) const fn new(section: SectionId, body: &'a [u8]) -> Self {
        Self {
            section,
            body,
            position: 0,
        }
    }

    pub(crate) const fn remaining(&self) -> usize {
        self.body.len().saturating_sub(self.position)
    }

    pub(crate) fn u8(&mut self) -> Result<u8, BankError> {
        let [byte] = self.array::<1>()?;
        Ok(byte)
    }

    pub(crate) fn u16(&mut self) -> Result<u16, BankError> {
        Ok(u16::from_le_bytes(self.array()?))
    }

    pub(crate) fn u32(&mut self) -> Result<u32, BankError> {
        Ok(u32::from_le_bytes(self.array()?))
    }

    /// Skips `len` bytes and returns their range in the body.
    pub(crate) fn skip(&mut self, len: usize) -> Result<std::ops::Range<usize>, BankError> {
        let start = self.position;
        let end = start
            .checked_add(len)
            .filter(|&end| end <= self.body.len())
            .ok_or_else(|| self.truncated(len))?;
        self.position = end;
        Ok(start..end)
    }

    /// Takes the next `len` bytes of the body.
    pub(crate) fn bytes(&mut self, len: usize) -> Result<&'a [u8], BankError> {
        let range = self.skip(len)?;
        Ok(self.body.get(range).unwrap_or_default())
    }

    pub(crate) fn f64(&mut self) -> Result<f64, BankError> {
        Ok(f64::from_le_bytes(self.array()?))
    }

    pub(crate) fn f32(&mut self) -> Result<f32, BankError> {
        Ok(f32::from_le_bytes(self.array()?))
    }

    pub(crate) fn i32(&mut self) -> Result<i32, BankError> {
        Ok(i32::from_le_bytes(self.array()?))
    }

    pub(crate) fn i16(&mut self) -> Result<i16, BankError> {
        Ok(i16::from_le_bytes(self.array()?))
    }

    /// Takes the bytes up to the next zero byte and skips that byte.
    pub(crate) fn c_string(&mut self) -> Result<&'a [u8], BankError> {
        let rest = self.body.get(self.position..).unwrap_or_default();
        let len = rest
            .iter()
            .position(|&byte| byte == 0)
            .ok_or_else(|| self.truncated(rest.len() + 1))?;
        self.position = self
            .position
            .checked_add(len)
            .and_then(|end| end.checked_add(1))
            .ok_or_else(|| self.truncated(len))?;
        Ok(rest.get(..len).unwrap_or_default())
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], BankError> {
        let field = self
            .body
            .get(self.position..)
            .and_then(<[u8]>::first_chunk::<N>)
            .copied()
            .ok_or_else(|| self.truncated(N))?;
        self.position += N;
        Ok(field)
    }

    const fn truncated(&self, len: usize) -> BankError {
        BankError::truncated(
            self.section,
            self.body.len(),
            self.position as u64 + len as u64,
        )
    }
}

impl ObjectFields for SectionReader<'_> {
    type Error = BankError;

    fn offset(&self) -> u64 {
        self.position as u64
    }

    fn remaining_bytes(&self) -> u64 {
        self.remaining() as u64
    }

    fn read_array<const N: usize>(&mut self) -> Result<[u8; N], Self::Error> {
        self.array()
    }

    fn skip_bytes(&mut self, len: u64) -> Result<(), Self::Error> {
        let len = usize::try_from(len)
            .map_err(|_| BankError::invalid(self.section, "a list is longer than its body"))?;
        self.skip(len).map(|_| ())
    }
}
