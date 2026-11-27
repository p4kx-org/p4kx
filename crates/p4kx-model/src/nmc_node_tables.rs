// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_foundation::ParseError;

use crate::nmc_layout::{NmcLayout, name_line};
use crate::nmc_properties::{self, Parts};
use crate::{NmcFrame, NmcFrames};

/// Name, property and material-index tables of the original NMC layout.
pub struct NmcNodeTables<'source> {
    layout: NmcLayout<'source>,
}

impl<'source> NmcNodeTables<'source> {
    /// Reads the positioned tables independently of rendering and parent validation.
    ///
    /// # Errors
    /// Returns a parse error when the mandatory tables do not fit the original chunk.
    pub fn read(bytes: &'source [u8]) -> Result<Self, ParseError> {
        Ok(Self {
            layout: NmcLayout::read(bytes)?,
        })
    }

    /// Returns the complete physical NMC records.
    #[must_use]
    pub const fn frames(&self) -> NmcFrames<'source> {
        NmcFrames::from_records(self.layout.records)
    }

    /// Returns the byte extent of the names table within this chunk.
    #[must_use]
    pub const fn names_range(&self) -> Range<u64> {
        self.layout.names_range()
    }

    /// Returns the available property table extent within this chunk.
    #[must_use]
    pub const fn properties_range(&self) -> Range<u64> {
        self.layout.properties_range()
    }

    /// Returns the property table byte count stored in the original header.
    #[must_use]
    pub const fn declared_property_bytes(&self) -> u32 {
        self.layout.declared_property_bytes
    }

    /// Returns the material index word extent within this chunk.
    #[must_use]
    pub const fn material_range(&self) -> Range<u64> {
        self.layout.material_start
            ..self.layout.material_start + self.layout.material_bytes.len() as u64
    }

    /// Streams the original material index words in stored table order.
    pub fn material_indices(&self) -> impl ExactSizeIterator<Item = u16> + '_ {
        self.layout
            .material_bytes
            .as_chunks::<2>()
            .0
            .iter()
            .copied()
            .map(u16::from_le_bytes)
    }

    /// Streams every physical node and its matching string table slots.
    pub fn nodes(&self) -> impl ExactSizeIterator<Item = NmcNode<'source>> + '_ {
        let mut names = TextTable::new(self.layout.names, self.layout.names_start);
        let mut props = TextTable::new(self.layout.properties, self.layout.properties_start);
        self.frames().frames().map(move |frame| NmcNode {
            frame,
            name: names.next(),
            properties: props.next(),
        })
    }
}

/// One physical NMC node and its paired, possibly absent, string slots.
pub struct NmcNode<'source> {
    frame: NmcFrame<'source>,
    name: Option<NmcText<'source>>,
    properties: Option<NmcText<'source>>,
}

impl<'source> NmcNode<'source> {
    /// Borrows the original node's structured transform fields.
    #[must_use]
    pub const fn frame(&self) -> NmcFrame<'source> {
        self.frame
    }

    /// Returns the whole stored name entry, excluding its optional NUL separator.
    #[must_use]
    pub const fn name(&self) -> Option<NmcText<'source>> {
        self.name
    }

    /// Returns the entire original property entry.
    #[must_use]
    pub const fn properties(&self) -> Option<NmcText<'source>> {
        self.properties
    }
}

/// Exact original bytes of one NMC string slot.
#[derive(Clone, Copy)]
pub struct NmcText<'source> {
    bytes: &'source [u8],
    start: u64,
    terminated: bool,
}

impl<'source> NmcText<'source> {
    /// Returns the original bytes without the null separator.
    #[must_use]
    pub const fn bytes(self) -> &'source [u8] {
        self.bytes
    }

    /// Returns whether the original entry ended with a null separator.
    #[must_use]
    pub const fn terminated(self) -> bool {
        self.terminated
    }

    /// Returns the original chunk-relative text extent, excluding the null.
    #[must_use]
    pub const fn range(self) -> Range<u64> {
        self.start..self.start + self.bytes.len() as u64
    }

    /// Returns the original first line the renderer uses as the display name.
    #[must_use]
    pub fn name_line(self) -> &'source [u8] {
        name_line(self.bytes)
    }

    /// Streams original property lines, preserving unparsed and duplicate lines.
    pub fn property_lines(self) -> impl Iterator<Item = NmcPropertyLine<'source>> {
        let mut at = self.start;
        self.bytes
            .split_inclusive(|&byte| byte == b'\n')
            .map(move |entry| {
                let line = entry.strip_suffix(b"\n").unwrap_or(entry);
                let start = at;
                at += entry.len() as u64;
                NmcPropertyLine { line, start }
            })
    }
}

/// A physical line in an NMC property slot.
pub struct NmcPropertyLine<'source> {
    line: &'source [u8],
    start: u64,
}

impl NmcPropertyLine<'_> {
    /// Returns the unaltered line bytes without a line feed.
    #[must_use]
    pub const fn bytes(&self) -> &[u8] {
        self.line
    }

    /// Returns the original chunk-relative line extent.
    #[must_use]
    pub const fn range(&self) -> Range<u64> {
        self.start..self.start + self.line.len() as u64
    }

    /// Returns original trimmed key and value byte extents when the line parses.
    #[must_use]
    pub fn property(&self) -> Option<(Range<u64>, Range<u64>)> {
        let Parts { key, value } = nmc_properties::parts(self.line)?;
        Some((
            nmc_properties::trimmed(key, self.start),
            nmc_properties::trimmed(value, self.start + key.len() as u64 + 1),
        ))
    }
}

struct TextTable<'source> {
    remaining: &'source [u8],
    offset: u64,
}

impl<'source> TextTable<'source> {
    const fn new(bytes: &'source [u8], start: u64) -> Self {
        Self {
            remaining: bytes,
            offset: start,
        }
    }

    fn next(&mut self) -> Option<NmcText<'source>> {
        if self.remaining.is_empty() {
            return None;
        }
        let (bytes, tail) = self
            .remaining
            .iter()
            .position(|&byte| byte == 0)
            .and_then(|index| {
                Some((
                    self.remaining.get(..index)?,
                    self.remaining.get(index + 1..)?,
                ))
            })
            .map_or((self.remaining, None), |(bytes, tail)| (bytes, Some(tail)));
        let start = self.offset;
        self.offset += bytes.len() as u64 + u64::from(tail.is_some());
        self.remaining = tail.unwrap_or_default();
        Some(NmcText {
            bytes,
            start,
            terminated: tail.is_some(),
        })
    }
}
