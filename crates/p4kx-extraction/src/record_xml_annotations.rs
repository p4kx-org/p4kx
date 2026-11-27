// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;

use p4kx_dataforge::{
    Annotations, DataForge, Guid, ObservedResolution, PositionedRecord, PositionedSources,
    PositionedValue, RecordJsonBytes, RecordJsonReferenceMember, RecordJsonReferenceSources,
    RecordJsonSource, RecordJsonUse, RecordXml, RecordXmlObserver, RecordXmlSite, RecordXmlSpan,
    RecordXmlValueMember, TagResolver, TextResolver, Value,
};

use super::{Capture, Fields, RecordFieldReceipt, XmlAddress, selector};
use crate::{
    DataForgeEntry, ObservedTagTree, RecordJsonLocation, RecordLocalization, SourceEvidenceError,
    SourceUseRole,
};

#[derive(Clone, Copy, Default)]
pub(crate) struct XmlAnnotationSources<'job, 'entry> {
    pub(crate) localization: Option<&'job RecordLocalization<'entry>>,
    pub(crate) tags: Option<&'job ObservedTagTree>,
}

impl<'job> XmlAnnotationSources<'job, '_> {
    pub(crate) fn validate(self, forge: &DataForge<'_>) -> io::Result<()> {
        if self.tags.is_some_and(|tags| !tags.is_from(forge)) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                SourceEvidenceError::ForeignEvidence,
            ));
        }
        Ok(())
    }

    pub(super) const fn is_empty(self) -> bool {
        self.localization.is_none() && self.tags.is_none()
    }

    fn annotations(self) -> Annotations<'job> {
        Annotations::none()
            .with_texts(self.localization.map(|source| source as &dyn TextResolver))
            .with_tags(self.tags.map(|source| source as &dyn TagResolver))
    }
}

pub(crate) fn render_annotated(
    writer: &mut RecordXml,
    input: &DataForgeEntry,
    forge: &DataForge<'_>,
    record: &PositionedRecord<'_>,
    sources: XmlAnnotationSources<'_, '_>,
) -> io::Result<RecordFieldReceipt> {
    let mut capture = Capture::from_record(input, forge, record)?;
    let mut observer = AnnotatedXmlObserver {
        fields: Fields::Single(&mut capture),
        forge,
        record,
        sources,
    };
    let mut bytes = Vec::new();
    writer.write_positioned_observed_annotated(
        record,
        sources.annotations(),
        &mut bytes,
        &mut observer,
    )?;
    capture.finish(bytes)
}

pub(super) fn write_wrapped<'entry>(
    writer: &mut RecordXml,
    forge: &DataForge<'_>,
    record: &PositionedRecord<'_>,
    sources: XmlAnnotationSources<'_, 'entry>,
    fields: Fields<'_, 'entry>,
    bytes: &mut Vec<u8>,
) -> io::Result<()> {
    let mut observer = AnnotatedXmlObserver {
        fields,
        forge,
        record,
        sources,
    };
    writer.write_wrapped_positioned_observed_annotated(
        record,
        sources.annotations(),
        1,
        bytes,
        &mut observer,
    )
}

struct AnnotatedXmlObserver<'job, 'entry, 'record, 'source, 'forge> {
    fields: Fields<'job, 'entry>,
    forge: &'job DataForge<'forge>,
    record: &'record PositionedRecord<'source>,
    sources: XmlAnnotationSources<'job, 'entry>,
}

impl AnnotatedXmlObserver<'_, '_, '_, '_, '_> {
    fn tag(
        &mut self,
        address: XmlAddress,
        guid: Guid,
        position: Option<PositionedValue<'_, '_>>,
        ancestors: &[PositionedSources],
    ) -> io::Result<()> {
        let guid_source = position
            .and_then(|position| position.reference_sources())
            .map(|reference| reference.member(RecordJsonReferenceMember::Guid));
        let path = self
            .sources
            .tags
            .map(|tags| tags.observed_locations(self.forge, guid))
            .transpose()
            .map_err(io::Error::other)?
            .flatten();
        let complete = path.is_some()
            && guid_source
                .as_ref()
                .is_some_and(RecordJsonReferenceSources::complete)
            && ancestors.iter().all(PositionedSources::complete);
        let route = ancestors
            .iter()
            .flat_map(|item| item.sources().iter().copied())
            .map(selector);
        let selectors = guid_source
            .iter()
            .flat_map(RecordJsonReferenceSources::sources)
            .map(reference_selector);
        self.fields.field(
            address,
            route.chain(selectors).chain(path.into_iter().flatten()),
            complete,
        )
    }
}

impl<'record, 'source> RecordXmlObserver<'record, 'source>
    for AnnotatedXmlObserver<'_, '_, 'record, 'source, '_>
{
    fn field(
        &mut self,
        span: RecordXmlSpan<'record, 'source>,
        position: Option<PositionedValue<'record, 'source>>,
        ancestors: &[PositionedSources],
    ) -> io::Result<()> {
        if let RecordXmlSite::Value {
            value: Value::Reference(reference),
            member: RecordXmlValueMember::Tag,
        } = span.site
        {
            return self.tag(
                XmlAddress::of(&span)?,
                reference.guid(),
                position,
                ancestors,
            );
        }
        self.fields
            .observe(self.forge, self.record, &span, position, ancestors)
    }

    fn localization(
        &mut self,
        span: RecordXmlSpan<'record, 'source>,
        position: Option<PositionedValue<'record, 'source>>,
        ancestors: &[PositionedSources],
        observed: &ObservedResolution<'_>,
    ) -> io::Result<()> {
        let local = position.as_ref().map(PositionedValue::value_sources);
        let key: Option<RecordJsonLocation> = local.as_ref().and_then(|local| {
            local
                .sources()
                .iter()
                .rev()
                .copied()
                .find_map(|site| match site {
                    RecordJsonSource::Bytes {
                        kind: RecordJsonBytes::Text,
                        ..
                    } => selector(site),
                    _ => None,
                })
        });
        let complete = local.as_ref().is_some_and(PositionedSources::complete)
            && ancestors.iter().all(PositionedSources::complete);
        let sites = ancestors
            .iter()
            .chain(local.as_ref())
            .flat_map(|item| item.sources().iter().copied())
            .map(selector);
        let lookup = self
            .sources
            .localization
            .and_then(|source| source.capture_lookup(observed));
        self.fields
            .localized_field(XmlAddress::of(&span)?, sites, key, complete, lookup)
    }
}

fn reference_selector(source: &RecordJsonUse) -> Option<RecordJsonLocation> {
    let mut site = RecordJsonLocation::of_use(source)?;
    if site.role == SourceUseRole::Payload {
        site.role = SourceUseRole::Selector;
    }
    Some(site)
}
