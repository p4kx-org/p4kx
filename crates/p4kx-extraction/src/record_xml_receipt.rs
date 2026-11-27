// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;
use std::ops::Range;

use p4kx_dataforge::{
    DataForge, PositionedRecord, PositionedSources, PositionedValue, RecordJsonRecordField,
    RecordJsonReferenceMember, RecordJsonReferenceSources, RecordJsonSource, RecordText, RecordXml,
    RecordXmlHeaderMember, RecordXmlSite, RecordXmlSpan, RecordXmlToken, RecordXmlValueMember,
};

use super::field_receipt::{Capture, RecordFieldReceipt, RecordOriginal};

#[path = "record_xml_annotations.rs"]
mod annotations;

use crate::resolved_texts::LocalizationLookupEvidence;
use crate::{
    DataForgeEntry, DecimalU64, OriginCompanionSpool, OriginOutputAddress, OriginXmlField,
    RecordJsonLocation, SourceEvidenceError, SourceUseRole,
};
pub(crate) use annotations::{XmlAnnotationSources, render_annotated};

pub(crate) fn render(
    writer: &mut RecordXml,
    input: &DataForgeEntry,
    forge: &DataForge<'_>,
    record: &PositionedRecord<'_>,
) -> io::Result<RecordFieldReceipt> {
    let mut capture = Capture::from_record(input, forge, record)?;
    let mut bytes = Vec::new();
    let mut fields = Fields::Single(&mut capture);
    writer.write_positioned_observed(record, &mut bytes, &mut |span, value, ancestors| {
        fields.observe(forge, record, &span, value, ancestors)
    })?;
    capture.finish(bytes)
}

pub(crate) fn render_wrapped<'entry>(
    writer: &mut RecordXml,
    forge: &DataForge<'_>,
    record: &PositionedRecord<'_>,
    sources: XmlAnnotationSources<'_, 'entry>,
) -> io::Result<WrappedRecordXml<'entry>> {
    let mut wrapped = WrappedRecordXml {
        bytes: Vec::new(),
        fields: WrappedFields::default(),
    };
    let mut fields = Fields::Wrapped(&mut wrapped.fields);
    if sources.is_empty() {
        writer.write_wrapped_positioned_observed(
            record,
            1,
            &mut wrapped.bytes,
            &mut |span, value, ancestors| fields.observe(forge, record, &span, value, ancestors),
        )?;
    } else {
        annotations::write_wrapped(writer, forge, record, sources, fields, &mut wrapped.bytes)?;
    }
    Ok(wrapped)
}

pub(crate) struct WrappedRecordXml<'entry> {
    pub(crate) bytes: Vec<u8>,
    fields: WrappedFields<'entry>,
}

impl WrappedRecordXml<'_> {
    pub(crate) fn stage(
        self,
        start: u64,
        original: &RecordOriginal,
        spool: &mut OriginCompanionSpool,
    ) -> io::Result<()> {
        let mut lookups = self.fields.lookups.into_iter();
        for field in self.fields.values {
            let (sources, complete) = if field.localized {
                let lookup = lookups
                    .next()
                    .ok_or_else(|| io::Error::other("missing XML localization evidence"))?;
                original.bind_lookup(
                    field.sites,
                    lookup.key.as_ref(),
                    field.complete,
                    lookup.evidence.as_ref(),
                    spool.sources(),
                )?
            } else {
                original.bind(field.sites, field.complete)?
            };
            spool.push(&field.address.at(start)?, &sources, complete)?;
        }
        if lookups.next().is_some() {
            return Err(io::Error::other("unbound XML localization evidence"));
        }
        Ok(())
    }
}

enum Fields<'sink, 'entry> {
    Single(&'sink mut Capture),
    Wrapped(&'sink mut WrappedFields<'entry>),
}

impl<'entry> Fields<'_, 'entry> {
    fn observe(
        &mut self,
        forge: &DataForge<'_>,
        record: &PositionedRecord<'_>,
        span: &RecordXmlSpan<'_, '_>,
        value: Option<PositionedValue<'_, '_>>,
        ancestors: &[PositionedSources],
    ) -> io::Result<()> {
        let address = XmlAddress::of(span)?;
        match span.site {
            RecordXmlSite::Header(member) => write_header(self, address, record, forge, member),
            RecordXmlSite::Value { member, .. } => {
                write_value(self, address, value, member, ancestors)
            }
            _ => self.field(address, [], false),
        }
    }

    fn field(
        &mut self,
        address: XmlAddress,
        sites: impl IntoIterator<Item = Option<RecordJsonLocation>>,
        complete: bool,
    ) -> io::Result<()> {
        match self {
            Self::Single(capture) => capture.field(address.at(0)?, sites, complete),
            Self::Wrapped(fields) => fields.push(address, sites, complete, None),
        }
    }

    fn localized_field(
        &mut self,
        address: XmlAddress,
        sites: impl IntoIterator<Item = Option<RecordJsonLocation>>,
        key: Option<RecordJsonLocation>,
        complete: bool,
        lookup: Option<LocalizationLookupEvidence<'entry>>,
    ) -> io::Result<()> {
        match self {
            Self::Single(capture) => capture.localized_value(
                address.at(0)?,
                sites,
                key.as_ref(),
                complete,
                lookup.as_ref(),
            ),
            Self::Wrapped(fields) => fields.push(
                address,
                sites,
                complete,
                Some(WrappedLookup {
                    key,
                    evidence: lookup,
                }),
            ),
        }
    }
}

#[derive(Default)]
struct WrappedFields<'entry> {
    values: Vec<WrappedField>,
    lookups: Vec<WrappedLookup<'entry>>,
}

impl<'entry> WrappedFields<'entry> {
    fn push(
        &mut self,
        address: XmlAddress,
        sites: impl IntoIterator<Item = Option<RecordJsonLocation>>,
        complete: bool,
        lookup: Option<WrappedLookup<'entry>>,
    ) -> io::Result<()> {
        let sites = collect_sites(sites)?;
        self.values.try_reserve(1).map_err(io::Error::other)?;
        let localized = lookup.is_some();
        if let Some(lookup) = lookup {
            self.lookups.try_reserve(1).map_err(io::Error::other)?;
            self.lookups.push(lookup);
        }
        self.values.push(WrappedField {
            address,
            sites,
            complete,
            localized,
        });
        Ok(())
    }
}

struct WrappedLookup<'entry> {
    key: Option<RecordJsonLocation>,
    evidence: Option<LocalizationLookupEvidence<'entry>>,
}

fn collect_sites(
    sites: impl IntoIterator<Item = Option<RecordJsonLocation>>,
) -> io::Result<Vec<Option<RecordJsonLocation>>> {
    let sites = sites.into_iter();
    let mut collected = Vec::new();
    collected
        .try_reserve(sites.size_hint().0)
        .map_err(io::Error::other)?;
    for site in sites {
        collected.try_reserve(1).map_err(io::Error::other)?;
        collected.push(site);
    }
    Ok(collected)
}

struct WrappedField {
    address: XmlAddress,
    sites: Vec<Option<RecordJsonLocation>>,
    complete: bool,
    localized: bool,
}

struct XmlAddress {
    bytes: Range<u64>,
    field: OriginXmlField,
}

impl XmlAddress {
    fn of(span: &RecordXmlSpan<'_, '_>) -> io::Result<Self> {
        let field = match span.token {
            RecordXmlToken::ElementName => OriginXmlField::ElementName,
            RecordXmlToken::ClosingElementName => OriginXmlField::ClosingElementName,
            RecordXmlToken::AttributeName => OriginXmlField::AttributeName,
            RecordXmlToken::AttributeValue => OriginXmlField::AttributeValue,
            _ => return Err(io::Error::other("unsupported Record XML token form")),
        };
        Ok(Self {
            bytes: span.bytes.clone(),
            field,
        })
    }

    fn at(self, offset: u64) -> io::Result<OriginOutputAddress> {
        let add = |base: u64| {
            offset
                .checked_add(base)
                .map(DecimalU64::new)
                .ok_or_else(|| io::Error::other("Record XML field offset exceeds u64"))
        };
        Ok(OriginOutputAddress::XmlBytes {
            start: add(self.bytes.start)?,
            end_exclusive: add(self.bytes.end)?,
            field: self.field,
        })
    }
}

fn write_header(
    capture: &mut Fields<'_, '_>,
    address: XmlAddress,
    record: &PositionedRecord<'_>,
    forge: &DataForge<'_>,
    member: RecordXmlHeaderMember,
) -> io::Result<()> {
    let tree = record.tree();
    let index = tree.record_index();
    let header = |field| RecordJsonLocation::of(RecordJsonSource::Record { index, field });
    match member {
        RecordXmlHeaderMember::Guid => {
            capture.field(address, [header(RecordJsonRecordField::Guid)], true)
        }
        RecordXmlHeaderMember::Path => capture.field(
            address,
            [
                header(RecordJsonRecordField::FileName),
                text(forge, tree.metadata().file_name()),
            ],
            true,
        ),
        RecordXmlHeaderMember::Name => {
            let original = forge
                .record(index)
                .ok_or_else(|| io::Error::other(SourceEvidenceError::ForeignEvidence))?;
            let header = header(RecordJsonRecordField::Name);
            match original.stored_name() {
                Ok(RecordText::Present(stored)) => {
                    capture.field(address, [header, text(forge, stored)], true)
                }
                Ok(RecordText::Absent | RecordText::Null) => capture.field(address, [header], true),
                _ => capture.field(address, [header], false),
            }
        }
        RecordXmlHeaderMember::Type => capture.field(
            address,
            [
                header(RecordJsonRecordField::Struct),
                RecordJsonLocation::of(RecordJsonSource::StructName {
                    index: tree.root().struct_index(),
                }),
            ],
            true,
        ),
        RecordXmlHeaderMember::Tag => capture.field(
            address,
            [
                header(RecordJsonRecordField::Tag),
                tree.tag().and_then(|tag| text(forge, tag)),
            ],
            true,
        ),
        _ => capture.field(address, [], false),
    }
}

fn text(forge: &DataForge<'_>, text: &str) -> Option<RecordJsonLocation> {
    Some(RecordJsonLocation {
        location: RecordJsonLocation::borrowed_text(forge, text)?,
        role: SourceUseRole::Payload,
    })
}

fn write_value(
    capture: &mut Fields<'_, '_>,
    address: XmlAddress,
    position: Option<PositionedValue<'_, '_>>,
    member: RecordXmlValueMember,
    ancestors: &[PositionedSources],
) -> io::Result<()> {
    let inherited = || {
        ancestors
            .iter()
            .flat_map(|ancestor| ancestor.sources().iter().copied())
            .map(selector)
    };
    let complete = ancestors.iter().all(PositionedSources::complete);
    let Some(position) = position else {
        return capture.field(address, inherited(), false);
    };
    if let Some(reference) = reference(position, member) {
        return capture.field(
            address,
            inherited().chain(reference.sources().iter().map(RecordJsonLocation::of_use)),
            complete && reference.complete(),
        );
    }
    let local = match member {
        RecordXmlValueMember::Name => position.field_name_sources(),
        RecordXmlValueMember::Value => Some(position.value_sources()),
        _ => None,
    };
    if let Some(local) = local {
        capture.field(
            address,
            inherited().chain(local.sources().iter().copied().map(RecordJsonLocation::of)),
            complete && local.complete(),
        )
    } else {
        capture.field(address, inherited(), false)
    }
}

fn reference(
    position: PositionedValue<'_, '_>,
    member: RecordXmlValueMember,
) -> Option<RecordJsonReferenceSources> {
    let key = match member {
        RecordXmlValueMember::Guid => {
            if let Some(sources) = position.dangling_reference_guid_sources() {
                return Some(sources);
            }
            RecordJsonReferenceMember::Guid
        }
        RecordXmlValueMember::Path => RecordJsonReferenceMember::Path,
        RecordXmlValueMember::Type => RecordJsonReferenceMember::Type,
        RecordXmlValueMember::InstancePath => RecordJsonReferenceMember::InstancePath,
        RecordXmlValueMember::Value => return position.dangling_reference_error_sources(),
        _ => return None,
    };
    Some(position.reference_sources()?.member(key))
}

fn selector(source: RecordJsonSource<'static>) -> Option<RecordJsonLocation> {
    let mut location = RecordJsonLocation::of(source)?;
    if !matches!(source, RecordJsonSource::PropertyStorage { .. }) {
        location.role = SourceUseRole::Selector;
    }
    Some(location)
}
