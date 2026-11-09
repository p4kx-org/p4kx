// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::MediaKind;
use p4kx_extraction::FieldOrigin;

use std::io::Write;
use std::path::Path;

use p4kx_extraction::{DecimalU64, SourceUses};
use sha2::{Digest as _, Sha256};

use crate::artifact::Stream;
use crate::checkpoint::Checkpoint;
use crate::origin_contract::{
    OriginCoverage, OriginProfile, OriginProtocol, OriginReceipts, OriginRow, OriginScope,
    OriginStream, OriginStreamSummary,
};
use crate::origin_prepared::PreparedRecordOrigins;
use crate::origin_resolver::OriginResolver;
use crate::{Artifact, MediaError, error, hashing, origin_error, origin_json};

/// Streaming provenance files with unchanged payload-stream admission budgets.
pub(crate) struct OriginTransport<'a> {
    fields: Stream<'a>,
    summaries: Vec<OriginStreamSummary>,
    checkpoint: Checkpoint<'a>,
    max_depth: u32,
}

impl<'a> OriginTransport<'a> {
    pub(crate) fn create(
        root: &Path,
        kinds: impl IntoIterator<Item = MediaKind>,
        checkpoint: Checkpoint<'a>,
        max_depth: u32,
    ) -> Result<Self, MediaError> {
        let mut summaries: Vec<_> = kinds
            .into_iter()
            .map(|kind| summary(OriginStream::Records { kind }))
            .collect();
        summaries.push(summary(OriginStream::Sources));
        summaries.push(summary(OriginStream::Artifacts));
        Ok(Self {
            fields: Stream::create(root, "field-origins.jsonl".into(), &checkpoint)?,
            summaries,
            checkpoint,
            max_depth,
        })
    }

    pub(crate) fn emit(
        &mut self,
        stream: OriginStream,
        profile: OriginProfile,
        payload: &[u8],
        resolver: &OriginResolver<'_>,
        prepared: Option<&PreparedRecordOrigins>,
    ) -> Result<(), MediaError> {
        let position = self
            .summaries
            .iter()
            .position(|summary| summary.stream == stream)
            .ok_or_else(|| MediaError::Contract("unregistered media origin stream".into()))?;
        let scope = OriginScope {
            stream,
            ordinal: self
                .summaries
                .get(position)
                .ok_or_else(origin_error::counter)?
                .occurrences,
        };
        let payload_sha256 = hashing::hex(Sha256::digest(payload).into());
        self.write(&OriginRow::Occurrence {
            scope: &scope,
            profile,
            payload_sha256: &payload_sha256,
        })?;
        let mut coverage = empty_coverage();
        origin_json::visit(
            payload,
            payload_depth(stream, self.max_depth),
            &mut |field| {
                self.checkpoint.check()?;
                let contribution = resolver.resolve(field, prepared)?;
                let origin = contribution.field_origin();
                observe(&mut coverage, &origin)?;
                self.write(&OriginRow::Field {
                    scope: &scope,
                    field,
                    origin: &origin,
                    cuts: &contribution.cuts,
                })
            },
        )?;
        self.write(&OriginRow::End {
            scope: &scope,
            coverage: &coverage,
        })?;
        let summary = self
            .summaries
            .get_mut(position)
            .ok_or_else(origin_error::counter)?;
        summary.occurrences = increment(summary.occurrences, 1)?;
        summary.fields = increment(summary.fields, coverage.fields.get())?;
        Ok(())
    }

    pub(crate) fn finish(
        mut self,
        root: &Path,
        owner: &SourceUses,
    ) -> Result<OriginReceipts, MediaError> {
        let inventory = owner.freeze().map_err(origin_error::evidence)?;
        let sources = inventory_stream(
            root,
            "origin-sources.jsonl",
            inventory.sources(),
            self.checkpoint,
        )?;
        let uses = inventory_stream(root, "origin-uses.jsonl", inventory.uses(), self.checkpoint)?;
        write_row(
            &mut self.fields,
            &OriginRow::Eof {
                streams: &self.summaries,
            },
            self.checkpoint,
        )?;
        Ok(OriginReceipts {
            protocol: OriginProtocol::V1,
            sources,
            uses,
            fields: self.fields.finish()?,
            streams: self.summaries,
        })
    }

    fn write(&mut self, row: &OriginRow<'_>) -> Result<(), MediaError> {
        write_row(&mut self.fields, row, self.checkpoint)
    }
}

fn inventory_stream<T: serde::Serialize>(
    root: &Path,
    relative: &str,
    rows: &[T],
    checkpoint: Checkpoint<'_>,
) -> Result<Artifact, MediaError> {
    let mut stream = Stream::create(root, relative.into(), &checkpoint)?;
    for row in rows {
        write_row(&mut stream, row, checkpoint)?;
    }
    stream.finish()
}

fn write_row<T: serde::Serialize>(
    stream: &mut Stream<'_>,
    row: &T,
    checkpoint: Checkpoint<'_>,
) -> Result<(), MediaError> {
    checkpoint.check()?;
    let serialised = serde_json::to_writer(
        crate::observed_write::ObservedWrite::new(&mut *stream, &checkpoint),
        row,
    );
    crate::json_output::finish(serialised, checkpoint)?;
    stream
        .write_all(b"\n")
        .map_err(|cause| error::io(stream.path(), cause))?;
    checkpoint.check()
}

const fn summary(stream: OriginStream) -> OriginStreamSummary {
    OriginStreamSummary {
        stream,
        occurrences: DecimalU64::new(0),
        fields: DecimalU64::new(0),
    }
}

const fn empty_coverage() -> OriginCoverage {
    OriginCoverage {
        fields: DecimalU64::new(0),
        complete: DecimalU64::new(0),
        incomplete: DecimalU64::new(0),
        unknown: DecimalU64::new(0),
        encrypted: DecimalU64::new(0),
    }
}

fn observe(coverage: &mut OriginCoverage, origin: &FieldOrigin) -> Result<(), MediaError> {
    coverage.fields = increment(coverage.fields, 1)?;
    let completeness = if origin.source_set_complete() {
        &mut coverage.complete
    } else {
        &mut coverage.incomplete
    };
    *completeness = increment(*completeness, 1)?;
    match origin.contains_encrypted_source() {
        None => coverage.unknown = increment(coverage.unknown, 1)?,
        Some(true) => coverage.encrypted = increment(coverage.encrypted, 1)?,
        Some(false) => {}
    }
    Ok(())
}

fn increment(value: DecimalU64, extra: u64) -> Result<DecimalU64, MediaError> {
    value
        .get()
        .checked_add(extra)
        .map(DecimalU64::new)
        .ok_or_else(origin_error::counter)
}
pub(crate) const fn payload_depth(stream: OriginStream, configured: u32) -> u32 {
    match stream {
        OriginStream::Records { .. } => configured,
        OriginStream::Sources | OriginStream::Artifacts => crate::limits::MAX_VALUE_DEPTH,
    }
}
