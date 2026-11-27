// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;

use p4kx_dataforge::{
    Annotations, DataForge, Guid, ObservedResolution, PositionedRecord, RecordJson,
    RecordJsonObserver, RecordJsonSource, RecordJsonUse,
};

use super::field_receipt::{Capture, RecordFieldReceipt};
use crate::resolved_texts::RecordLocalization;
use crate::{
    ConflictPolicy, DataForgeEntry, FieldPath, FilePublication, FileReceipt, ObservedTagTree,
    OriginOutputAddress, RecordJsonLocation, TagPathOrigins,
};

/// Canonical Record JSON and the contributors to its emitted fields.
pub struct RecordJsonReceipt(RecordFieldReceipt);

impl RecordJsonReceipt {
    /// Renders the positioned Record with the writer's layout and captures its original inputs.
    ///
    /// # Errors
    /// Returns a source mismatch, serialization or evidence allocation error.
    pub fn render(
        writer: &mut RecordJson,
        input: &DataForgeEntry,
        forge: &DataForge<'_>,
        record: &PositionedRecord<'_>,
    ) -> io::Result<Self> {
        Self::render_file(writer, input, forge, record).map(Self)
    }

    pub(crate) fn render_file(
        writer: &mut RecordJson,
        input: &DataForgeEntry,
        forge: &DataForge<'_>,
        record: &PositionedRecord<'_>,
    ) -> io::Result<RecordFieldReceipt> {
        let mut capture = Capture::from_record(input, forge, record)?;
        let mut bytes = Vec::new();
        writer.write_observed(record, &mut bytes, &mut capture)?;
        capture.finish(bytes)
    }

    /// Renders a Record with origin evidence for the exact indexed language Entry.
    ///
    /// # Errors
    /// Returns a source mismatch, serialization or evidence allocation error.
    pub fn render_localized(
        writer: &mut RecordJson,
        input: &DataForgeEntry,
        forge: &DataForge<'_>,
        record: &PositionedRecord<'_>,
        localization: &RecordLocalization<'_>,
    ) -> io::Result<Self> {
        Self::render_localized_file(writer, input, forge, record, localization).map(Self)
    }

    pub(crate) fn render_localized_file(
        writer: &mut RecordJson,
        input: &DataForgeEntry,
        forge: &DataForge<'_>,
        record: &PositionedRecord<'_>,
        localization: &RecordLocalization<'_>,
    ) -> io::Result<RecordFieldReceipt> {
        Self::render_annotated_file(writer, input, forge, record, Some(localization), None)
    }

    pub(crate) fn render_annotated_file(
        writer: &mut RecordJson,
        input: &DataForgeEntry,
        forge: &DataForge<'_>,
        record: &PositionedRecord<'_>,
        localization: Option<&RecordLocalization<'_>>,
        tags: Option<&ObservedTagTree>,
    ) -> io::Result<RecordFieldReceipt> {
        let capture = Capture::from_record(input, forge, record)?;
        let paths = tags
            .map(|tags| tags.bind_paths(forge, capture.uses()))
            .transpose()
            .map_err(io::Error::other)?;
        let mut observer = AnnotatedJsonObserver {
            capture,
            localization,
            paths,
        };
        let annotations = Annotations::none()
            .with_texts(localization.map(|source| source as &dyn p4kx_dataforge::TextResolver))
            .with_tags(tags.map(|source| source as &dyn p4kx_dataforge::TagResolver));
        let mut bytes = Vec::new();
        writer.write_observed_annotated(record, annotations, &mut bytes, &mut observer)?;
        observer.capture.finish(bytes)
    }

    /// Returns the exact JSON bytes that the receipt describes.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        self.0.bytes()
    }

    /// Publishes the observed JSON and its companion under the bound file's conflict policy.
    ///
    /// # Errors
    /// Returns a read, write, cancellation, ownership or receipt failure.
    pub fn publish(
        &self,
        receipt: &FileReceipt<'_>,
        policy: ConflictPolicy,
    ) -> io::Result<FilePublication> {
        receipt.publish_bytes(self.0.bytes(), policy, |out, binding| {
            self.0.describe(out, binding)
        })
    }
}

impl RecordJsonObserver for Capture {
    fn value(
        &mut self,
        pointer: &str,
        sources: &[RecordJsonSource<'_>],
        complete: bool,
    ) -> io::Result<()> {
        self.field(
            json_address(pointer)?,
            sources.iter().copied().map(RecordJsonLocation::of),
            complete,
        )
    }

    fn reference_value(
        &mut self,
        pointer: &str,
        sources: &[RecordJsonUse],
        complete: bool,
    ) -> io::Result<()> {
        self.field(
            json_address(pointer)?,
            sources.iter().map(RecordJsonLocation::of_use),
            complete,
        )
    }
}

struct AnnotatedJsonObserver<'job, 'source> {
    capture: Capture,
    localization: Option<&'job RecordLocalization<'source>>,
    paths: Option<TagPathOrigins<'job>>,
}

impl RecordJsonObserver for AnnotatedJsonObserver<'_, '_> {
    fn value(
        &mut self,
        pointer: &str,
        sources: &[RecordJsonSource<'_>],
        complete: bool,
    ) -> io::Result<()> {
        self.capture.value(pointer, sources, complete)
    }

    fn reference_value(
        &mut self,
        pointer: &str,
        sources: &[RecordJsonUse],
        complete: bool,
    ) -> io::Result<()> {
        self.capture.reference_value(pointer, sources, complete)
    }

    fn localization_observed(
        &mut self,
        pointer: &str,
        _key: &str,
        observation: &ObservedResolution<'_>,
        selectors: &[RecordJsonUse],
        selectors_complete: bool,
    ) -> io::Result<()> {
        let Some(localization) = self.localization else {
            return self.reference_value(pointer, selectors, false);
        };
        self.capture.localized_field(
            json_address(pointer)?,
            selectors,
            selectors_complete,
            localization,
            observation,
        )
    }

    fn tag(
        &mut self,
        pointer: &str,
        guid: Guid,
        _path: &str,
        selectors: &[RecordJsonUse],
        selectors_complete: bool,
    ) -> io::Result<()> {
        let Some(paths) = &self.paths else {
            return self.reference_value(pointer, selectors, false);
        };
        self.capture.tagged_field(
            json_address(pointer)?,
            selectors,
            selectors_complete,
            paths,
            guid,
        )
    }
}

fn json_address(pointer: &str) -> io::Result<OriginOutputAddress> {
    let path = FieldPath::parse(pointer).map_err(io::Error::other)?;
    Ok(OriginOutputAddress::JsonField { path })
}
