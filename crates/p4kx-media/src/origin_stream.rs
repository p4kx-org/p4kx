// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Write};
use std::path::Path;

use p4kx_extraction::{DecimalU64, FieldOrigin, SourceUses};
use sha2::{Digest as _, Sha256};

use crate::artifact::{Stream, StreamDestination};
use crate::checkpoint::{Checkpoint, IO_BLOCK_BYTES};
use crate::origin_contract::use_sets::Sets;
use crate::origin_contract::{
    OriginCoverage, OriginField, OriginProfile, OriginProtocol, OriginReceipts, OriginRow,
    OriginScope, OriginStream, OriginStreamSummary,
};
use crate::origin_contributions::OriginContributions;
use crate::origin_prepared::PreparedRecordOrigins;
use crate::origin_resolver::OriginResolver;
use crate::{Artifact, MediaError, MediaKind, error, hashing, origin_error, origin_json};

#[path = "origin_stream_reference_cache.rs"]
mod reference_cache;

/// Streaming provenance files with unchanged payload-stream admission budgets.
pub(crate) struct OriginTransport<'a> {
    fields: Stream<'a>,
    row_buffer: Vec<u8>,
    sets: Sets,
    summaries: Vec<OriginStreamSummary>,
    destination: StreamDestination<'a>,
    max_depth: u32,
}

impl<'a> OriginTransport<'a> {
    pub(crate) fn create(
        root: &Path,
        kinds: impl IntoIterator<Item = MediaKind>,
        destination: StreamDestination<'a>,
        max_depth: u32,
    ) -> Result<Self, MediaError> {
        let mut summaries: Vec<_> = kinds
            .into_iter()
            .map(|kind| summary(OriginStream::Records { kind }))
            .collect();
        summaries.push(summary(OriginStream::Sources));
        summaries.push(summary(OriginStream::Artifacts));
        summaries.push(summary(OriginStream::IndexBuild));
        Ok(Self {
            fields: destination.create(root, "field-origins.jsonl".into())?,
            row_buffer: Vec::with_capacity(IO_BLOCK_BYTES),
            sets: Sets::default(),
            summaries,
            destination,
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
        let mut references = reference_cache::Cache::default();
        origin_json::visit(
            payload,
            payload_depth(stream, self.max_depth),
            &mut |field| {
                self.destination.checkpoint().check()?;
                let address = address(stream, field);
                let field = address.as_ref();
                let contribution = resolver.resolve(field, prepared)?;
                self.emit_field(&scope, field, &contribution, &mut coverage, &mut references)
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

    fn emit_field(
        &mut self,
        scope: &OriginScope,
        field: &str,
        contribution: &OriginContributions,
        coverage: &mut OriginCoverage,
        references: &mut reference_cache::Cache,
    ) -> Result<(), MediaError> {
        if let Some(shared) = &contribution.shared {
            let id = self.sets.register(shared)?;
            let origin = contribution.field_origin()?;
            let local: Vec<_> = contribution
                .uses
                .iter()
                .filter(|used| !shared.contains(used))
                .map(p4kx_extraction::SourceUse::reference)
                .collect();
            observe(coverage, &origin)?;
            return self.write(&OriginRow::Field {
                scope,
                field,
                origin: OriginField::with_set(&origin, &local, id),
                cuts: &contribution.cuts,
            });
        }
        references.with_origin(contribution, |origin, encoded| {
            observe(coverage, origin)?;
            self.write(&OriginRow::Field {
                scope,
                field,
                origin: OriginField::new(origin, encoded),
                cuts: &contribution.cuts,
            })
        })
    }

    pub(crate) fn finish(
        mut self,
        root: &Path,
        owner: &SourceUses,
    ) -> Result<OriginReceipts, MediaError> {
        let inventory = owner.freeze().map_err(origin_error::evidence)?;
        let checkpoint = self.destination.checkpoint();
        let sources = inventory_stream(
            root,
            "origin-sources.jsonl",
            inventory.sources(),
            self.destination,
            &mut self.row_buffer,
        )?;
        let uses = inventory_stream(
            root,
            "origin-uses.jsonl",
            inventory.uses(),
            self.destination,
            &mut self.row_buffer,
        )?;
        write_row(
            &mut self.fields,
            &mut self.row_buffer,
            &OriginRow::Eof {
                streams: &self.summaries,
            },
            checkpoint,
        )?;
        let mut sets = self
            .destination
            .create(root, "origin-use-sets.jsonl".into())?;
        self.sets
            .visit(|row| write_row(&mut sets, &mut self.row_buffer, &row, checkpoint))?;
        Ok(OriginReceipts {
            protocol: OriginProtocol::V2,
            sets: sets.finish()?,
            sources,
            uses,
            fields: self.fields.finish()?,
            streams: self.summaries,
        })
    }

    fn write(&mut self, row: &OriginRow<'_>) -> Result<(), MediaError> {
        write_row(
            &mut self.fields,
            &mut self.row_buffer,
            row,
            self.destination.checkpoint(),
        )
    }
}

fn inventory_stream<T: serde::Serialize>(
    root: &Path,
    relative: &str,
    rows: &[T],
    destination: StreamDestination<'_>,
    row_buffer: &mut Vec<u8>,
) -> Result<Artifact, MediaError> {
    let mut stream = destination.create(root, relative.into())?;
    for row in rows {
        write_row(&mut stream, row_buffer, row, destination.checkpoint())?;
    }
    stream.finish()
}

fn write_row<T: serde::Serialize>(
    stream: &mut Stream<'_>,
    bytes: &mut Vec<u8>,
    row: &T,
    checkpoint: Checkpoint<'_>,
) -> Result<(), MediaError> {
    checkpoint.check()?;
    bytes.clear();
    let mut buffered = BufferedRow { stream, bytes };
    let serialised = serde_json::to_writer(
        crate::observed_write::ObservedWrite::new(&mut buffered, &checkpoint),
        row,
    );
    crate::json_output::finish(serialised, checkpoint)?;
    let written = buffered
        .write_all(b"\n")
        .and_then(|()| buffered.drain())
        .map_err(|cause| error::io(buffered.stream.path(), cause));
    crate::output_operation::checked(written, checkpoint.check())
}

struct BufferedRow<'buffer, 'stream> {
    stream: &'buffer mut Stream<'stream>,
    bytes: &'buffer mut Vec<u8>,
}

impl BufferedRow<'_, '_> {
    fn drain(&mut self) -> io::Result<()> {
        self.stream.write_all(self.bytes)?;
        self.bytes.clear();
        Ok(())
    }
}

impl Write for BufferedRow<'_, '_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.bytes.len() == IO_BLOCK_BYTES {
            self.drain()?;
        }
        let count = bytes.len().min(IO_BLOCK_BYTES - self.bytes.len());
        let (prefix, _) = bytes.split_at(count);
        self.bytes.extend_from_slice(prefix);
        Ok(count)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.drain()?;
        self.stream.flush()
    }
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
        OriginStream::Sources | OriginStream::Artifacts | OriginStream::IndexBuild => {
            crate::limits::MAX_VALUE_DEPTH
        }
    }
}

pub(crate) fn address(stream: OriginStream, path: &str) -> std::borrow::Cow<'_, str> {
    if stream == OriginStream::IndexBuild {
        std::borrow::Cow::Owned(format!("/source/build{path}"))
    } else {
        std::borrow::Cow::Borrowed(path)
    }
}
