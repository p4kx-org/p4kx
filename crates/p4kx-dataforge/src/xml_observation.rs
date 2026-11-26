// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Write};
use std::ops::Range;

use crate::Value;

/// An original member of the emitted Record header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum RecordXmlHeaderMember {
    /// Type name in a tag or fallback attribute.
    Type,
    /// Stored Record GUID.
    Guid,
    /// Displayed Record path.
    Path,
    /// Authored Record name.
    Name,
    /// Authored Record tag when present.
    Tag,
}

/// The part of a field or array element that Record XML wrote.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum RecordXmlValueMember {
    /// Original property name as an attribute, element name or escaped name value.
    Name,
    /// Formatted scalar, nested type, null or error value.
    Value,
    /// A resolved localisation value or the explicit missing-text decision.
    Translation,
    /// Reference GUID, including an authored dangling GUID.
    Guid,
    /// Selected Reference target path.
    Path,
    /// Selected Reference target type.
    Type,
    /// Selected instance path within a Reference target.
    InstancePath,
    /// Selected Reference tag path when requested.
    Tag,
}

/// An emitted Record XML field and its exact occurrence in the canonical tree.
#[derive(Clone, Copy)]
#[non_exhaustive]
pub enum RecordXmlSite<'record, 'source> {
    /// The supplied Record's original header member.
    Header(RecordXmlHeaderMember),
    /// A member of the exact field or array element being written.
    Value {
        /// The selected canonical field value or array element.
        value: &'record Value<'source>,
        /// The actual member emitted for the value.
        member: RecordXmlValueMember,
    },
}

impl<'record, 'source> RecordXmlSite<'record, 'source> {
    pub(super) const fn member(
        value: &'record Value<'source>,
        member: RecordXmlValueMember,
    ) -> Self {
        Self::Value { value, member }
    }
}

/// A logical Record XML datum and its final encoded byte range.
#[non_exhaustive]
pub struct RecordXmlSpan<'record, 'source> {
    /// Half-open output range, or an insertion point for an explicitly empty value.
    pub bytes: Range<u64>,
    /// Exact original occurrence used by the writer.
    pub site: RecordXmlSite<'record, 'source>,
}

type Observe<'out, 'record, 'source> =
    dyn FnMut(RecordXmlSpan<'record, 'source>) -> io::Result<()> + 'out;

pub(super) struct Output<'out, 'record, 'source, W> {
    writer: &'out mut W,
    observe: Option<&'out mut Observe<'out, 'record, 'source>>,
    position: u64,
}

impl<'out, 'record, 'source, W: Write> Output<'out, 'record, 'source, W> {
    pub(super) fn new(
        writer: &'out mut W,
        observe: Option<&'out mut Observe<'out, 'record, 'source>>,
    ) -> Self {
        Self {
            writer,
            observe,
            position: 0,
        }
    }

    pub(super) fn fragment(
        &mut self,
        site: Option<RecordXmlSite<'record, 'source>>,
        write: impl FnOnce(&mut Self) -> io::Result<()>,
    ) -> io::Result<()> {
        let start = self.position;
        write(self)?;
        if let (Some(observe), Some(site)) = (&mut self.observe, site) {
            observe(RecordXmlSpan {
                bytes: start..self.position,
                site,
            })?;
        }
        Ok(())
    }

    fn advance(&mut self, length: usize) -> io::Result<()> {
        if self.observe.is_some() {
            self.position = self
                .position
                .checked_add(length as u64)
                .ok_or_else(|| io::Error::other("Record XML byte position exceeds u64"))?;
        }
        Ok(())
    }
}

impl<W: Write> Write for Output<'_, '_, '_, W> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let written = self.writer.write(bytes)?;
        self.advance(written)?;
        Ok(written)
    }

    fn write_all(&mut self, bytes: &[u8]) -> io::Result<()> {
        self.writer.write_all(bytes)?;
        self.advance(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.writer.flush()
    }
}
