// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_foundation::{ByteReader, ParseError};

/// The existing reader's total-length extra framing, including its four-byte header.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ExtraBlockHeader {
    id: u16,
    declared_len: u16,
}

impl ExtraBlockHeader {
    pub(crate) fn read(reader: &mut ByteReader<'_>) -> Result<Self, ParseError> {
        Ok(Self {
            id: reader.read_u16()?,
            declared_len: reader.read_u16()?,
        })
    }
    pub(crate) const fn id(self) -> u16 {
        self.id
    }
    pub(crate) const fn declared_len(self) -> u16 {
        self.declared_len
    }
    pub(crate) fn body_len(self) -> Option<usize> {
        self.declared_len.checked_sub(4).map(usize::from)
    }
}
