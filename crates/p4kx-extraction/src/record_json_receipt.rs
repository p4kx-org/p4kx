// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Write};

use p4kx_dataforge::{
    DataForge, PositionedRecord, RecordJson, RecordJsonObserver, RecordJsonSource, RecordJsonUse,
};

use crate::{
    ConflictPolicy, DataForgeEntry, DataForgeOrigin, FieldOrigin, FieldPath, FilePublication,
    FileReceipt, InputScope, OriginArtifactBinding, OriginOutputAddress, RecordJsonLocation,
    SourceEvidenceError, SourceInput, SourceInventory, SourceScope, SourceUses,
    write_origin_companion,
};

/// Canonical, unannotated Record JSON and the contributors to its emitted fields.
pub struct RecordJsonReceipt {
    bytes: Vec<u8>,
    fields: Vec<(OriginOutputAddress, FieldOrigin)>,
    inventory: SourceInventory,
}

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
        let original = forge
            .record(record.tree().record_index())
            .filter(|original| record.tree().is_from(original))
            .and_then(|_| input.origin_of(forge))
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    SourceEvidenceError::ForeignEvidence,
                )
            })?;
        let mut capture = Capture::new(original)?;
        let mut bytes = Vec::new();
        writer.write_observed(record, &mut bytes, &mut capture)?;
        let inventory = capture.uses.freeze().map_err(io::Error::other)?;
        Ok(Self {
            bytes,
            fields: capture.fields,
            inventory,
        })
    }

    /// Returns the exact JSON bytes that the receipt describes.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
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
        receipt.publish_bytes(&self.bytes, policy, |out, binding| {
            self.describe(out, binding)
        })
    }

    pub(crate) fn describe(
        &self,
        out: &mut dyn Write,
        binding: &OriginArtifactBinding,
    ) -> io::Result<()> {
        write_origin_companion(
            out,
            binding,
            &self.inventory,
            self.fields
                .iter()
                .map(|(address, origin)| (address, origin)),
        )
    }
}

struct Capture {
    uses: SourceUses,
    scope: SourceScope,
    complete: bool,
    fields: Vec<(OriginOutputAddress, FieldOrigin)>,
}

impl Capture {
    fn new(origin: &DataForgeOrigin) -> io::Result<Self> {
        let (source, complete) = match origin {
            DataForgeOrigin::Archive(entry) => (
                SourceInput::Archive(entry),
                entry.ancestors().all(|step| step.is_some()),
            ),
            DataForgeOrigin::File(file) => (SourceInput::File(file), true),
            DataForgeOrigin::Unknown => (SourceInput::Unknown, false),
        };
        let uses = SourceUses::with_compact_use_refs();
        let scope = uses
            .observe(source, InputScope::Payload)
            .map_err(io::Error::other)?;
        Ok(Self {
            uses,
            scope,
            complete,
            fields: Vec::new(),
        })
    }

    fn field(
        &mut self,
        pointer: &str,
        sites: impl ExactSizeIterator<Item = Option<RecordJsonLocation>>,
        complete: bool,
    ) -> io::Result<()> {
        let count = sites.len();
        let mut contributors = Vec::new();
        contributors
            .try_reserve_exact(count)
            .map_err(io::Error::other)?;
        for site in sites.flatten() {
            contributors.push(
                self.scope
                    .at(site.location, site.role)
                    .map_err(io::Error::other)?,
            );
        }
        let origin = self.uses.origin(
            &contributors,
            self.complete && complete && contributors.len() == count,
        );
        let path = FieldPath::parse(pointer).map_err(io::Error::other)?;
        self.fields.try_reserve(1).map_err(io::Error::other)?;
        self.fields
            .push((OriginOutputAddress::JsonField { path }, origin));
        Ok(())
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
            pointer,
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
            pointer,
            sources.iter().map(RecordJsonLocation::of_use),
            complete,
        )
    }
}
