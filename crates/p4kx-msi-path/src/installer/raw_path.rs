// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::installer::fault::Fault;

pub(crate) const MAX_PATH_BYTES: usize = 1 << 20;

/// The registry string kind is independent of the bytes it contains.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StringKind {
    Plain,
    Expandable,
}

/// Exact source bytes include their original terminator spelling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RawPath {
    Absent,
    Present { kind: StringKind, bytes: Vec<u8> },
}

pub(crate) struct Parts<'a> {
    pub(crate) kind: StringKind,
    pub(crate) body: &'a [u8],
    pub(crate) tail: &'a [u8],
}

impl RawPath {
    pub(crate) fn split(&self) -> Result<Option<Parts<'_>>, Fault> {
        let Self::Present { kind, bytes } = self else {
            return Ok(None);
        };
        if bytes.len() > MAX_PATH_BYTES || !bytes.len().is_multiple_of(2) {
            return Err(Fault::InvalidRawPath);
        }
        let (words, []) = bytes.as_chunks::<2>() else {
            return Err(Fault::InvalidRawPath);
        };
        let end = words
            .iter()
            .position(|word| *word == [0, 0])
            .map_or(bytes.len(), |index| index * 2);
        let body = bytes.get(..end).ok_or(Fault::InvalidRawPath)?;
        let tail = bytes.get(end..).ok_or(Fault::InvalidRawPath)?;
        Ok(Some(Parts {
            kind: *kind,
            body,
            tail,
        }))
    }

    pub(crate) fn with_body(&self, body: Vec<u8>) -> Result<Self, Fault> {
        let (kind, tail) = match self.split()? {
            None => (StringKind::Plain, &[0, 0][..]),
            Some(Parts { kind, tail, .. }) => (kind, tail),
        };
        let size = body
            .len()
            .checked_add(tail.len())
            .ok_or(Fault::InvalidRawPath)?;
        if size > MAX_PATH_BYTES {
            return Err(Fault::InvalidRawPath);
        }
        let mut bytes = body;
        bytes.extend_from_slice(tail);
        Ok(Self::Present { kind, bytes })
    }
}
