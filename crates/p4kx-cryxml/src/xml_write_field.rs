// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Write};
use std::ops::Range;

/// One source-backed field emitted by the XML serializer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum XmlFieldKind {
    /// The original node name in its opening tag.
    ElementName,
    /// The original node name in its closing tag.
    ClosingElementName,
    /// The original attribute name after XML disambiguation.
    AttributeName,
    /// The original attribute value after XML escaping.
    AttributeValue,
    /// The original node text after XML escaping.
    Text,
}

/// The exact original source coordinate and emitted byte range of a field.
#[derive(Debug, Clone)]
pub struct XmlWriteField {
    pub(crate) site: FieldSite,
    pub(crate) range: Range<u64>,
}

#[derive(Debug, Clone)]
pub(crate) struct FieldSite {
    pub(crate) node: u32,
    pub(crate) attribute: Option<u32>,
    pub(crate) kind: XmlFieldKind,
    pub(crate) preceding_names: Option<Range<u32>>,
}

impl FieldSite {
    pub(crate) const fn node(node: u32, kind: XmlFieldKind) -> Self {
        Self {
            node,
            attribute: None,
            kind,
            preceding_names: None,
        }
    }

    pub(crate) const fn attribute(node: u32, attribute: u32, kind: XmlFieldKind) -> Self {
        Self {
            node,
            attribute: Some(attribute),
            kind,
            preceding_names: None,
        }
    }
}

impl XmlWriteField {
    /// Returns the emitted field category.
    #[must_use]
    pub const fn kind(&self) -> XmlFieldKind {
        self.site.kind
    }

    /// Returns the original CryXml node table index.
    #[must_use]
    pub const fn node_index(&self) -> u32 {
        self.site.node
    }

    /// Returns the original CryXml attribute table index when applicable.
    #[must_use]
    pub const fn attribute_index(&self) -> Option<u32> {
        self.site.attribute
    }

    /// Returns the prior original attribute name range used by a suffix assignment.
    #[must_use]
    pub const fn preceding_names(&self) -> Option<&Range<u32>> {
        self.site.preceding_names.as_ref()
    }

    /// Returns the half-open byte range in the emitted XML.
    #[must_use]
    pub const fn range(&self) -> &Range<u64> {
        &self.range
    }
}

pub(crate) struct Tracked<W> {
    inner: W,
    offset: u64,
}

impl<W: Write> Tracked<W> {
    pub(crate) const fn new(inner: W) -> Self {
        Self { inner, offset: 0 }
    }

    pub(crate) fn field(
        &mut self,
        site: FieldSite,
        write: impl FnOnce(&mut Self) -> io::Result<()>,
        observe: &mut impl FnMut(XmlWriteField) -> io::Result<()>,
    ) -> io::Result<()> {
        let start = self.offset;
        write(self)?;
        observe(XmlWriteField {
            site,
            range: start..self.offset,
        })
    }
}

impl<W: Write> Write for Tracked<W> {
    fn write(&mut self, input: &[u8]) -> io::Result<usize> {
        let written = self.inner.write(input)?;
        self.offset = self
            .offset
            .checked_add(written as u64)
            .ok_or_else(|| io::Error::other("XML output byte count exceeds u64"))?;
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}
