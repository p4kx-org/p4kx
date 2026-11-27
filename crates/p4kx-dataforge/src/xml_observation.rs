// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Write};
use std::ops::Range;

use crate::{
    ObservedResolution, PositionedFields, PositionedRecord, PositionedSources, PositionedValue,
    Value,
};

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

/// The form of one sourced token written by the Record XML serializer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum RecordXmlToken {
    /// An opening element name.
    ElementName,
    /// A closing element name.
    ClosingElementName,
    /// An attribute name or its sourced suffix.
    AttributeName,
    /// An attribute's escaped value.
    AttributeValue,
}

/// A logical Record XML datum and its final encoded byte range.
#[non_exhaustive]
pub struct RecordXmlSpan<'record, 'source> {
    /// Half-open output range, or an insertion point for an explicitly empty value.
    pub bytes: Range<u64>,
    /// Exact original occurrence used by the writer.
    pub site: RecordXmlSite<'record, 'source>,
    /// The exact form of the emitted token.
    pub token: RecordXmlToken,
}

type Observe<'out, 'record, 'source> =
    dyn FnMut(RecordXmlSpan<'record, 'source>) -> io::Result<()> + 'out;

/// Receives exact field ranges from positioned Record XML.
pub trait RecordXmlObserver<'record, 'source> {
    /// Receives an ordinary field's byte range and original occurrence.
    ///
    /// # Errors
    /// Returns an error to stop serialization.
    fn field(
        &mut self,
        span: RecordXmlSpan<'record, 'source>,
        value: Option<PositionedValue<'record, 'source>>,
        ancestors: &[PositionedSources],
    ) -> io::Result<()>;

    /// Receives an emitted locale annotation and the actual lookup used to render it.
    /// The value and ancestors retain the original DataForge key and its selectors.
    ///
    /// # Errors
    /// Returns an error to stop serialization.
    fn localization(
        &mut self,
        span: RecordXmlSpan<'record, 'source>,
        value: Option<PositionedValue<'record, 'source>>,
        ancestors: &[PositionedSources],
        _observed: &ObservedResolution<'_>,
    ) -> io::Result<()> {
        self.field(span, value, ancestors)
    }
}

impl<'record, 'source, F> RecordXmlObserver<'record, 'source> for F
where
    F: FnMut(
        RecordXmlSpan<'record, 'source>,
        Option<PositionedValue<'record, 'source>>,
        &[PositionedSources],
    ) -> io::Result<()>,
{
    fn field(
        &mut self,
        span: RecordXmlSpan<'record, 'source>,
        value: Option<PositionedValue<'record, 'source>>,
        ancestors: &[PositionedSources],
    ) -> io::Result<()> {
        self(span, value, ancestors)
    }
}

struct PositionedObservation<'out, 'record, 'source> {
    record: &'record PositionedRecord<'source>,
    observe: &'out mut dyn RecordXmlObserver<'record, 'source>,
}

pub(super) struct Output<'out, 'record, 'source, W> {
    writer: &'out mut W,
    observe: Option<&'out mut Observe<'out, 'record, 'source>>,
    position: u64,
    positioned: Option<PositionedObservation<'out, 'record, 'source>>,
    value: Option<PositionedValue<'record, 'source>>,
    containers: Vec<PositionedSources>,
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
            positioned: None,
            value: None,
            containers: Vec::new(),
        }
    }

    pub(super) fn positioned(
        writer: &'out mut W,
        record: &'record PositionedRecord<'source>,
        observe: &'out mut dyn RecordXmlObserver<'record, 'source>,
    ) -> Self {
        let mut output = Self::new(writer, None);
        output.positioned = Some(PositionedObservation { record, observe });
        output
    }

    pub(super) fn root_fields(&self) -> io::Result<Option<PositionedFields<'record, 'source>>> {
        self.positioned
            .as_ref()
            .map(|positioned| positioned.record.root_fields().map_err(io::Error::other))
            .transpose()
    }

    pub(super) const fn value(&self) -> Option<PositionedValue<'record, 'source>> {
        self.value
    }

    pub(super) fn at<T>(
        &mut self,
        value: Option<PositionedValue<'record, 'source>>,
        write: impl FnOnce(&mut Self) -> io::Result<T>,
    ) -> io::Result<T> {
        let parent = std::mem::replace(&mut self.value, value);
        let result = write(self);
        self.value = parent;
        result
    }

    pub(super) fn enter(&mut self) -> io::Result<bool> {
        let Some(selection) = self.value.and_then(|value| value.selection_sources()) else {
            return Ok(false);
        };
        self.containers.try_reserve(1).map_err(io::Error::other)?;
        self.containers.push(selection);
        Ok(true)
    }

    pub(super) fn leave(&mut self) {
        drop(self.containers.pop());
    }

    pub(super) fn fragment(
        &mut self,
        site: Option<RecordXmlSite<'record, 'source>>,
        token: RecordXmlToken,
        write: impl FnOnce(&mut Self) -> io::Result<()>,
    ) -> io::Result<()> {
        self.fragment_observed(site, token, None, write)
    }

    pub(super) fn localized(
        &mut self,
        value: &'record Value<'source>,
        observed: &ObservedResolution<'_>,
        write: impl FnOnce(&mut Self) -> io::Result<()>,
    ) -> io::Result<()> {
        self.fragment_observed(
            Some(RecordXmlSite::member(
                value,
                RecordXmlValueMember::Translation,
            )),
            RecordXmlToken::AttributeValue,
            Some(observed),
            write,
        )
    }

    fn fragment_observed(
        &mut self,
        site: Option<RecordXmlSite<'record, 'source>>,
        token: RecordXmlToken,
        observed: Option<&ObservedResolution<'_>>,
        write: impl FnOnce(&mut Self) -> io::Result<()>,
    ) -> io::Result<()> {
        let start = self.position;
        write(self)?;
        if let Some(site) = site {
            let span = RecordXmlSpan {
                bytes: start..self.position,
                site,
                token,
            };
            if let Some(positioned) = &mut self.positioned {
                let local = match site {
                    RecordXmlSite::Header(_) => None,
                    RecordXmlSite::Value { value, .. } => {
                        self.value.filter(|slot| std::ptr::eq(slot.value(), value))
                    }
                };
                match observed {
                    Some(observed) => {
                        positioned
                            .observe
                            .localization(span, local, &self.containers, observed)?;
                    }
                    None => positioned.observe.field(span, local, &self.containers)?,
                }
            } else if let Some(observe) = &mut self.observe {
                observe(span)?;
            }
        }
        Ok(())
    }

    fn advance(&mut self, length: usize) -> io::Result<()> {
        if self.observe.is_some() || self.positioned.is_some() {
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
