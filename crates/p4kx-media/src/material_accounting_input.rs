// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::Read;
use std::time::{Duration, Instant};

use p4kx_extraction::model::{
    MaterialError, MaterialLibrary, MaterialReadError, MaterialSourceLimits,
};
use p4kx_extraction::{Archive, Entry};
use sha2::{Digest as _, Sha256};

use crate::checkpoint::IO_BLOCK_BYTES;
use crate::directory_inputs::RetainedPayload;
use crate::directory_lookup::DirectoryLookup;
use crate::directory_run::{self, Phase};
use crate::input_budget::InputBudget;
use crate::source_selection::EntrySelection;
use crate::traversal_visit::TraversalVisit;
use crate::{
    Algorithm, ByteScope, Digest, Identity, MediaError, MediaWriter, Provider, SourceContext,
    error, hashing, material_projection, raw_output, source,
};

use super::census::{Census, CensusCounts};
use super::sink::Sink;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum ParserOracle {
    Observed,
    AlsoUnobserved,
}

#[derive(Default)]
pub(super) struct Timings {
    pub(super) verified_payload: Duration,
    pub(super) parse: Duration,
    pub(super) projection: Duration,
    pub(super) compare: Duration,
    pub(super) shader_index: Duration,
    pub(super) schema_decode: Duration,
}

pub(super) struct Survey {
    pub(super) census: CensusCounts,
    pub(super) timings: Timings,
}

pub(super) struct Visit<'a> {
    providers: &'a [&'a dyn Provider],
    budget: &'a InputBudget,
    sink: &'a Sink,
    census: Census,
    read_buffer: Vec<u8>,
    oracle: ParserOracle,
    timings: Timings,
    last_report: Instant,
}

impl<'a> Visit<'a> {
    pub(super) fn new(
        providers: &'a [&'a dyn Provider],
        budget: &'a InputBudget,
        sink: &'a Sink,
        oracle: ParserOracle,
    ) -> Self {
        Self {
            providers,
            budget,
            sink,
            census: Census::default(),
            read_buffer: vec![0; IO_BLOCK_BYTES],
            oracle,
            timings: Timings::default(),
            last_report: Instant::now(),
        }
    }

    pub(super) fn complete(self) -> Result<Survey, MediaError> {
        Ok(Survey {
            census: self.census.finish()?,
            timings: self.timings,
        })
    }
}

impl TraversalVisit for Visit<'_> {
    fn counts_sources(&self) -> bool {
        true
    }

    fn directory(
        &mut self,
        archive: &Archive,
        lookup: &DirectoryLookup<'_>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        self.census.open(archive, lookup.chain)?;
        directory_run::run(
            archive,
            lookup,
            self.providers,
            self.budget,
            Phase::Open,
            writer,
        )
    }

    fn directory_complete(
        &mut self,
        archive: &Archive,
        lookup: &DirectoryLookup<'_>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        self.census.close(lookup.chain)?;
        directory_run::run(
            archive,
            lookup,
            self.providers,
            self.budget,
            Phase::Complete,
            writer,
        )
    }

    fn entry(
        &mut self,
        entry: &Entry,
        selection: EntrySelection,
        lookup: &DirectoryLookup<'_>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<bool, MediaError> {
        self.report_progress();
        let Some(identity) = self.census.entry(lookup.index.archive, entry, selection)? else {
            return Ok(false);
        };
        let declared = source::entry(lookup.root, lookup.index.archive, entry, lookup.chain)?;
        let inputs = lookup.input_state()?;
        if inputs.begin(entry.index(), writer)?.is_some() {
            return Err(invalid("material payload already retained"));
        }
        let mut retained = None;
        let result = self
            .project(entry, lookup, declared, &identity, writer)
            .map(|raw| {
                retained = Some(RetainedPayload {
                    raw,
                    original: None,
                });
                true
            });
        error::finish(result, inputs.end(entry.index(), retained))
    }

    fn excluded_container(&mut self) {}
}

impl Visit<'_> {
    #[expect(
        clippy::print_stderr,
        reason = "the opt-in replay prints anonymous progress and timings"
    )]
    fn report_progress(&mut self) {
        const REPORT_INTERVAL: Duration = Duration::from_secs(60);

        if self.last_report.elapsed() < REPORT_INTERVAL {
            return;
        }
        self.last_report = Instant::now();
        let counts = self.census.counts();
        eprintln!(
            "material-source-export entries={} libraries={} verified_payload_ms={} parse_ms={} projection_ms={} compare_ms={}",
            counts.enumerated,
            counts.selected,
            self.timings.verified_payload.as_millis(),
            self.timings.parse.as_millis(),
            self.timings.projection.as_millis(),
            self.timings.compare.as_millis(),
        );
    }

    fn project(
        &mut self,
        entry: &Entry,
        lookup: &DirectoryLookup<'_>,
        mut source: crate::Source,
        identity: &Identity,
        writer: &mut MediaWriter<'_>,
    ) -> Result<crate::RawReference, MediaError> {
        let archive = lookup.index.archive;
        let limits = MaterialSourceLimits::default();
        let started = Instant::now();
        let observed = Payload::read(
            archive,
            entry,
            limits,
            self.budget,
            writer,
            &mut self.read_buffer,
        )?;
        self.timings.verified_payload += started.elapsed();
        let independent = (self.oracle == ParserOracle::AlsoUnobserved)
            .then(|| observed.parse(entry, limits, writer))
            .transpose()?;
        let raw = raw_output::receipt(observed.digest, source.size);
        let input = writer.origins.input(archive, entry)?;
        raw_output::record(&raw, &input, writer)?;
        source::retained(&mut source, entry, raw.clone());
        writer.retained_with_input(&source, &input)?;
        writer.begin_with_input(0, source.clone(), input)?;
        let payload = writer.root.join(&raw.path);
        let context = SourceContext {
            source: &source,
            entry,
            directory: lookup,
            payload: &payload,
        };
        let started = Instant::now();
        let mut parsed = material_projection::read(
            &context,
            limits,
            writer,
            |_, _| {
                observed
                    .bytes
                    .ok_or_else(|| invalid("admitted material payload missing"))
            },
            |_| Ok(()),
        )?;
        self.timings.parse += started.elapsed();
        let started = Instant::now();
        let projected = material_projection::write(&context, &mut parsed, writer)?;
        writer.end_source(projected)?;
        self.timings.projection += started.elapsed();
        let started = Instant::now();
        let original = parsed
            .original()
            .ok_or_else(|| invalid("unsupported material reader outcome"))?;
        let expected = independent.as_ref().map_or(original, Result::as_ref);
        self.sink
            .check_entry(identity, &source, expected, entry.uncompressed_size())?;
        self.timings.compare += started.elapsed();
        Ok(raw)
    }
}

struct Payload {
    bytes: Option<Vec<u8>>,
    digest: Digest,
}

impl Payload {
    fn read(
        archive: &Archive,
        entry: &Entry,
        limits: MaterialSourceLimits,
        budget: &InputBudget,
        writer: &mut MediaWriter<'_>,
        buffer: &mut [u8],
    ) -> Result<Self, MediaError> {
        let size = entry.uncompressed_size();
        if size > writer.limits().entry_bytes {
            return Err(MediaError::Limit {
                budget: "entry_bytes",
                limit: writer.limits().entry_bytes,
            });
        }
        budget.bytes(size, writer.limits())?;
        writer.reserve_output(size)?;
        let mut reader = archive
            .open_entry_verified(entry)
            .map_err(|cause| MediaError::Archive(Box::new(cause)))?;
        let retain = usize::try_from(size).is_ok_and(|size| size <= limits.source_bytes);
        let mut bytes = retain.then(Vec::new);
        let mut hash = Sha256::new();
        let mut remaining = size;
        loop {
            writer.checkpoint()?;
            let count = reader
                .read(buffer)
                .map_err(|cause| error::io(archive.path(), cause))?;
            if count == 0 {
                break;
            }
            remaining = remaining
                .checked_sub(count as u64)
                .ok_or_else(|| invalid("original material size exceeded"))?;
            let chunk = buffer
                .get(..count)
                .ok_or_else(|| invalid("invalid material read length"))?;
            hash.update(chunk);
            if let Some(bytes) = &mut bytes {
                bytes.extend_from_slice(chunk);
            }
        }
        if remaining != 0 {
            return Err(invalid("original material ended before inventoried size"));
        }
        Ok(Self {
            bytes,
            digest: Digest {
                algorithm: Algorithm::Sha256,
                scope: ByteScope::DecodedEntryPayload,
                hex: hashing::hex(hash.finalize().into()),
            },
        })
    }

    fn parse(
        &self,
        entry: &Entry,
        limits: MaterialSourceLimits,
        writer: &MediaWriter<'_>,
    ) -> Result<Result<MaterialLibrary, MaterialError>, MediaError> {
        let Some(bytes) = &self.bytes else {
            return Ok(Err(MaterialError::SourceBytes {
                limit: limits.source_bytes,
            }));
        };
        match MaterialLibrary::parse_with(entry.path(), bytes, limits, || writer.checkpoint()) {
            Ok(library) => Ok(Ok(library)),
            Err(MaterialReadError::Material(error)) => Ok(Err(error)),
            Err(MaterialReadError::Interrupted(error)) => Err(error),
            Err(_) => Err(invalid("unsupported material reader outcome")),
        }
    }
}

fn invalid(message: &'static str) -> MediaError {
    MediaError::Contract(message.into())
}
