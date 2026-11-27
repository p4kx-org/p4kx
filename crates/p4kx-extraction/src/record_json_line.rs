// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;

use p4kx_dataforge::{
    Annotations, DataForge, Guid, JsonLayout, ObservedResolution, PositionedRecord, RecordJson,
    RecordJsonBytes, RecordJsonObserver, RecordJsonSource, RecordJsonUse, TagResolver,
    TextResolver,
};

use super::record_stream_input::{RecordStreamInput, foreign};
use super::stream_fields::{RecordStreamRows, StreamAddress, StreamField, StreamLookup};
use crate::{FieldPath, ObservedTagTree, RecordJsonLocation, RecordLocalization};

impl<'input, 'locale> RecordStreamRows<'input, 'locale> {
    /// Writes a compact Record line with its original field and annotation observations.
    ///
    /// # Errors
    /// Returns a foreign evidence, positioning, serialization or allocation error.
    pub fn write_json(
        input: RecordStreamInput<'input, '_>,
        writer: &mut RecordJson,
        record: &PositionedRecord<'_>,
        out: &mut Vec<u8>,
        locale: Option<&RecordLocalization<'locale>>,
        tags: Option<&ObservedTagTree>,
    ) -> io::Result<Self> {
        if writer.layout() != JsonLayout::Compact {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Record NDJSON requires compact JSON",
            ));
        }
        input.check(record)?;
        if tags.is_some_and(|tree| !tree.is_from(input.forge())) {
            return Err(foreign());
        }
        let mut capture = Capture {
            forge: input.forge(),
            locale,
            tags,
            fields: Vec::new(),
        };
        let annotations = Annotations::none()
            .with_texts(locale.map(|source| source as &dyn TextResolver))
            .with_tags(tags.map(|source| source as &dyn TagResolver));
        writer.write_observed_annotated(record, annotations, out, &mut capture)?;
        out.push(b'\n');
        Ok(Self::new(input, 1, capture.fields))
    }
}

struct Capture<'job, 'database, 'locale> {
    forge: &'database DataForge<'database>,
    locale: Option<&'job RecordLocalization<'locale>>,
    tags: Option<&'job ObservedTagTree>,
    fields: Vec<StreamField<'locale>>,
}

impl<'locale> Capture<'_, '_, 'locale> {
    fn field(
        &mut self,
        pointer: &str,
        sites: impl IntoIterator<Item = Option<RecordJsonLocation>>,
        complete: bool,
        lookup: Option<StreamLookup<'locale>>,
    ) -> io::Result<()> {
        let path = FieldPath::parse(pointer).map_err(io::Error::other)?;
        let sites = sites.into_iter();
        let mut located = Vec::new();
        located
            .try_reserve(sites.size_hint().0)
            .map_err(io::Error::other)?;
        for site in sites {
            located.try_reserve(1).map_err(io::Error::other)?;
            located.push(site);
        }
        self.fields.try_reserve(1).map_err(io::Error::other)?;
        self.fields.push(StreamField {
            address: StreamAddress::Json { row: 0, path },
            sites: located,
            complete,
            lookup,
        });
        Ok(())
    }
}

impl RecordJsonObserver for Capture<'_, '_, '_> {
    fn value(
        &mut self,
        pointer: &str,
        sites: &[RecordJsonSource<'_>],
        complete: bool,
    ) -> io::Result<()> {
        self.field(
            pointer,
            sites.iter().copied().map(RecordJsonLocation::of),
            complete,
            None,
        )
    }

    fn reference_value(
        &mut self,
        pointer: &str,
        sites: &[RecordJsonUse],
        complete: bool,
    ) -> io::Result<()> {
        self.field(
            pointer,
            sites.iter().map(RecordJsonLocation::of_use),
            complete,
            None,
        )
    }

    fn localization_observed(
        &mut self,
        pointer: &str,
        _key: &str,
        observation: &ObservedResolution<'_>,
        selectors: &[RecordJsonUse],
        complete: bool,
    ) -> io::Result<()> {
        let authored_key = selectors
            .iter()
            .rfind(|input| {
                matches!(
                    input.source,
                    RecordJsonSource::Bytes {
                        kind: RecordJsonBytes::Text,
                        ..
                    }
                )
            })
            .and_then(RecordJsonLocation::of_use);
        let evidence = self
            .locale
            .and_then(|locale| locale.capture_lookup(observation));
        self.field(
            pointer,
            selectors.iter().map(RecordJsonLocation::of_use),
            complete,
            Some(StreamLookup {
                authored_key,
                evidence,
            }),
        )
    }

    fn tag(
        &mut self,
        pointer: &str,
        guid: Guid,
        _path: &str,
        selectors: &[RecordJsonUse],
        complete: bool,
    ) -> io::Result<()> {
        let Some(tags) = self.tags else {
            return self.reference_value(pointer, selectors, false);
        };
        let locations = tags
            .observed_locations(self.forge, guid)
            .map_err(io::Error::other)?;
        let complete = complete && locations.is_some();
        self.field(
            pointer,
            selectors
                .iter()
                .map(RecordJsonLocation::of_use)
                .chain(locations.into_iter().flatten()),
            complete,
            None,
        )
    }
}
