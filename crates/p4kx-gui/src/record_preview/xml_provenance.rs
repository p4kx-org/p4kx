// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{HashMap, TryReserveError};
use std::ops::Range;
use std::ptr;

use p4kx_extraction::{
    FieldOrigin, RecordXmlHeaderMember, RecordXmlSite, RecordXmlSpan, RecordXmlValueMember, Value,
};

use super::{RecordFieldProvenance, RecordHeaderProvenance};
use crate::{XML_TEXT_LIMIT, XmlPreviewOriginError};

/// An emitted Record XML field and its original contributors.
#[derive(Debug)]
#[non_exhaustive]
pub struct RecordXmlFieldProvenance {
    /// Half-open byte range in the retained XML, or the insertion point of an empty value.
    pub bytes: Range<u64>,
    /// Original contributors, completeness and immediate encryption marker.
    pub origin: FieldOrigin,
}

pub(super) struct PendingXml(Result<Vec<Span>, TryReserveError>);

struct Span {
    bytes: Range<u64>,
    site: Site,
}

enum Site {
    Header(RecordXmlHeaderMember),
    Value {
        value: usize,
        member: RecordXmlValueMember,
    },
    Unknown,
}

impl PendingXml {
    pub(super) const fn new() -> Self {
        Self(Ok(Vec::new()))
    }

    pub(super) fn observe(&mut self, span: RecordXmlSpan<'_, '_>) {
        if !retained(&span.bytes, XML_TEXT_LIMIT as u64) {
            return;
        }
        if let Ok(spans) = &mut self.0 {
            match spans.try_reserve(1) {
                Ok(()) => spans.push(Span {
                    bytes: span.bytes,
                    site: Site::from(span.site),
                }),
                Err(error) => self.0 = Err(error),
            }
        }
    }

    pub(super) fn bind(
        self,
        retained_end: u64,
        positions: RowPositions,
        header: &RecordHeaderProvenance,
        lines: &[RecordFieldProvenance],
        unknown: &FieldOrigin,
    ) -> Result<Vec<RecordXmlFieldProvenance>, XmlPreviewOriginError> {
        let spans = self.0?;
        let positions = positions.0?;
        let kept = spans
            .into_iter()
            .take_while(|span| span.bytes.start < retained_end);
        let mut bound = Vec::new();
        for span in kept.filter(|span| retained(&span.bytes, retained_end)) {
            bound.try_reserve(1)?;
            let origin = span
                .site
                .origin(&positions, header, lines)
                .unwrap_or(unknown)
                .clone();
            bound.push(RecordXmlFieldProvenance {
                bytes: span.bytes,
                origin,
            });
        }
        Ok(bound)
    }
}

pub(super) struct RowPositions(Result<HashMap<usize, usize>, TryReserveError>);

impl RowPositions {
    pub(super) fn reserve(count: usize) -> Self {
        let mut positions = HashMap::new();
        Self(positions.try_reserve(count).map(|()| positions))
    }

    pub(super) fn record(&mut self, value: &Value<'_>, row: usize) {
        if let Ok(positions) = &mut self.0 {
            positions.insert(value_address(value), row);
        }
    }
}

fn value_address(value: &Value<'_>) -> usize {
    ptr::from_ref(value).addr()
}

impl From<RecordXmlSite<'_, '_>> for Site {
    fn from(site: RecordXmlSite<'_, '_>) -> Self {
        match site {
            RecordXmlSite::Header(header) => Self::Header(header),
            RecordXmlSite::Value { value, member, .. } => Self::Value {
                value: value_address(value),
                member,
            },
            _ => Self::Unknown,
        }
    }
}

impl Site {
    fn origin<'a>(
        &self,
        positions: &HashMap<usize, usize>,
        header: &'a RecordHeaderProvenance,
        lines: &'a [RecordFieldProvenance],
    ) -> Option<&'a FieldOrigin> {
        match self {
            Self::Header(header_member) => header_origin(header, *header_member),
            Self::Value { value, member } => {
                row_origin(lines.get(*positions.get(value)?)?, *member)
            }
            Self::Unknown => None,
        }
    }
}

const fn header_origin(
    header: &RecordHeaderProvenance,
    member: RecordXmlHeaderMember,
) -> Option<&FieldOrigin> {
    match member {
        RecordXmlHeaderMember::Type => Some(&header.type_name),
        RecordXmlHeaderMember::Guid => Some(&header.guid),
        RecordXmlHeaderMember::Path => Some(&header.record_path),
        RecordXmlHeaderMember::Name => Some(&header.name),
        RecordXmlHeaderMember::Tag => header.tag.as_ref(),
        _ => None,
    }
}

fn row_origin(row: &RecordFieldProvenance, member: RecordXmlValueMember) -> Option<&FieldOrigin> {
    use RecordXmlValueMember::{Guid, InstancePath, Name, Path, Tag, Translation, Type, Value};
    match member {
        Name => Some(&row.name),
        Value => Some(&row.value),
        Tag | Translation => row.annotation.as_ref(),
        Guid => row
            .reference
            .as_ref()
            .map(|reference| &reference.guid)
            .or(row.dangling_guid.as_ref()),
        Path => Some(&row.reference.as_ref()?.path),
        Type => Some(&row.reference.as_ref()?.type_name),
        InstancePath => row.reference.as_ref()?.instance_path.as_ref(),
        _ => None,
    }
}

const fn retained(bytes: &Range<u64>, end: u64) -> bool {
    bytes.start < end && bytes.end <= end
}

#[cfg(test)]
mod tests {
    use p4kx_extraction::{RecordXmlHeaderMember, SourceUses};

    use super::{PendingXml, RecordHeaderProvenance, RowPositions, Site, Span};
    use crate::XmlPreviewOriginError;

    #[test]
    fn binding_keeps_only_complete_fragments_in_retained_xml() -> Result<(), XmlPreviewOriginError>
    {
        let uses = SourceUses::new();
        let unknown = uses.origin(&[], false);
        let header = RecordHeaderProvenance {
            name: unknown.clone(),
            type_name: unknown.clone(),
            guid: unknown.clone(),
            record_path: unknown.clone(),
            tag: None,
        };
        let pending = PendingXml(Ok([1..3, 3..3, 4..6, 5..5, 6..7]
            .into_iter()
            .map(|bytes| Span {
                bytes,
                site: Site::Header(RecordXmlHeaderMember::Name),
            })
            .collect()));
        let fields = pending.bind(5, RowPositions::reserve(0), &header, &[], &unknown)?;
        let ranges: Vec<_> = fields.into_iter().map(|field| field.bytes).collect();
        assert_eq!(
            ranges,
            [1..3, 3..3],
            "incomplete and omitted spans have no field evidence"
        );
        Ok(())
    }
}
