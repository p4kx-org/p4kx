// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Read};

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
    Algorithm, ByteScope, Digest, KindSummary, MediaError, MediaWriter, Provider, RawReference,
    Source, SourceContext, Tally, error, hashing, raw_output, source,
};

use super::rows::{AnimationRows, Family, Totals};
use super::stream::Observation;
use super::{TestResult, failure, require};

pub(super) struct Visit<'a> {
    family: Family,
    providers: &'a [&'a dyn Provider],
    budget: &'a InputBudget,
    observation: &'a Observation,
    schema: &'a jsonschema::Validator,
    inventory: Inventory,
    totals: Totals,
}

#[derive(Default)]
struct Inventory {
    entries: u64,
    matches: u64,
    containers: u64,
    opened: u64,
}

impl<'a> Visit<'a> {
    pub(super) fn new(
        family: Family,
        providers: &'a [&'a dyn Provider],
        budget: &'a InputBudget,
        observation: &'a Observation,
        schema: &'a jsonschema::Validator,
    ) -> Self {
        Self {
            family,
            providers,
            budget,
            observation,
            schema,
            inventory: Inventory::default(),
            totals: Totals::default(),
        }
    }

    pub(super) fn finish(mut self, traversal: &Tally, kind: &KindSummary) -> TestResult<Totals> {
        require(
            self.inventory.entries == traversal.enumerated
                && self.inventory.matches == traversal.written
                && self.inventory.containers == self.inventory.opened
                && self.totals.entries == traversal.written
                && traversal.partial == 0
                && traversal.failed == 0,
            "animation source census agrees with every original directory and container",
        )?;
        let records =
            self.totals.entries + self.totals.chunks + self.totals.clips + self.totals.controllers;
        let counts = &kind.counts;
        require(
            counts.source_entries.enumerated == self.totals.entries
                && counts.records.enumerated == records
                && counts.records.partial == self.totals.partial
                && counts.records.failed == 0
                && counts.converted_outputs.enumerated == self.totals.sample_artifacts,
            "animation writer summaries agree with the original source and key inventory",
        )?;
        self.totals.complete = self.totals.partial == 0
            && counts.source_entries.partial == 0
            && counts.source_entries.failed == 0;
        Ok(self.totals)
    }

    fn project(
        &mut self,
        entry: &Entry,
        lookup: &DirectoryLookup<'_>,
        mut source: Source,
        writer: &mut MediaWriter<'_>,
    ) -> Result<RawReference, MediaError> {
        let archive = lookup.index.archive;
        let payload = Payload::read(archive, entry, self.budget, writer)?;
        let raw = raw_output::receipt(payload.digest, source.size);
        let input = writer.origins.input(archive, entry)?;
        raw_output::record(&raw, &input, writer)?;
        source::retained(&mut source, entry, raw.clone());
        writer.retained_with_input(&source, &input)?;
        writer.begin_with_input(0, source.clone(), input.clone())?;
        let path = writer.root.join(&raw.path);
        let context = SourceContext {
            source: &source,
            entry,
            directory: lookup,
            payload: &path,
        };
        let projected = super::super::provider::project_with(&context, writer, |_, writer| {
            let blocks = source.size.div_ceil(IO_BLOCK_BYTES as u64);
            self.budget
                .metadata(source.size, 1 + blocks, writer.limits(), writer.checkpoint)?;
            Ok(payload.bytes.as_slice())
        })?;
        writer.end_source(projected)?;
        self.check_source(entry, &source, &payload.bytes)
            .map_err(|cause| {
                failure::report(self.family, "reconcile-entry", cause.as_ref());
                invalid(cause.to_string())
            })?;
        self.observation
            .origins
            .finish_entry(archive, entry, &input, writer)
            .map_err(|cause| {
                failure::report(self.family, "reconcile-entry-origins", cause.as_ref());
                invalid(cause.to_string())
            })?;
        Ok(raw)
    }

    fn check_source(&mut self, entry: &Entry, expected: &Source, bytes: &[u8]) -> TestResult {
        let mut evidence = self.observation.take_entry();
        let source = evidence.raw(expected)?;
        let mut rows = AnimationRows {
            evidence: &mut evidence,
            source: &source,
            schema: self.schema,
            totals: &mut self.totals,
        };
        rows.verify(self.family, entry.path(), bytes)?;
        evidence.finish_entry()?;
        self.totals.entries += 1;
        Ok(())
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
        if archive
            .issues()
            .iter()
            .any(|(_, issue)| issue.is_archive_wide())
        {
            return Err(invalid(
                "animation enumeration crossed an incomplete archive",
            ));
        }
        self.inventory.opened += u64::from(!lookup.chain.is_empty());
        for entry in archive.entries() {
            writer.checkpoint()?;
            self.inventory.entries += 1;
            self.inventory.matches += u64::from(matches_family(entry, self.family));
            let suffix = entry.path().rsplit('.').next().unwrap_or_default();
            self.inventory.containers += u64::from(
                suffix.eq_ignore_ascii_case("pak") || suffix.eq_ignore_ascii_case("socpak"),
            );
        }
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
        let selected = selection == EntrySelection::Included;
        if selected != matches_family(entry, self.family) {
            return Err(invalid(
                "animation filter disagrees with the original Entry suffix",
            ));
        }
        if !selected {
            return Ok(false);
        }
        let original = source::entry(lookup.root, lookup.index.archive, entry, lookup.chain)?;
        let inputs = lookup.input_state()?;
        if inputs.begin(entry.index(), writer)?.is_some() {
            return Err(invalid("animation Entry was visited repeatedly"));
        }
        let mut retained = None;
        let result = self.project(entry, lookup, original, writer).map(|raw| {
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

fn matches_family(entry: &Entry, family: Family) -> bool {
    entry
        .path()
        .rsplit_once('.')
        .is_some_and(|(_, suffix)| suffix.eq_ignore_ascii_case(family.suffix()))
}

struct Payload {
    bytes: Vec<u8>,
    digest: Digest,
}

impl Payload {
    fn read(
        archive: &Archive,
        entry: &Entry,
        budget: &InputBudget,
        writer: &mut MediaWriter<'_>,
    ) -> Result<Self, MediaError> {
        writer.checkpoint()?;
        let limits = writer.limits();
        let size = entry.uncompressed_size();
        budget.bytes(size, limits)?;
        if size > limits.entry_bytes {
            return Err(MediaError::Limit {
                budget: "entry_bytes",
                limit: limits.entry_bytes,
            });
        }
        writer.reserve_output(size)?;
        let length = usize::try_from(size)
            .map_err(|_| invalid("animation source exceeds native address width"))?;
        let mut bytes = Vec::new();
        bytes.try_reserve_exact(length).map_err(|cause| {
            error::io(
                archive.path(),
                io::Error::new(io::ErrorKind::OutOfMemory, cause),
            )
        })?;
        bytes.resize(length, 0);
        let mut reader = archive
            .open_entry_verified(entry)
            .map_err(|cause| MediaError::Archive(Box::new(cause)))?;
        let mut hash = Sha256::new();
        for block in bytes.chunks_mut(IO_BLOCK_BYTES) {
            writer.checkpoint()?;
            reader
                .read_exact(block)
                .map_err(|cause| error::io(archive.path(), cause))?;
            hash.update(block);
        }
        writer.checkpoint()?;
        if reader
            .read(&mut [0])
            .map_err(|cause| error::io(archive.path(), cause))?
            != 0
        {
            return Err(invalid(
                "verified original animation exceeded its directory length",
            ));
        }
        writer.checkpoint()?;
        Ok(Self {
            bytes,
            digest: Digest {
                algorithm: Algorithm::Sha256,
                scope: ByteScope::DecodedEntryPayload,
                hex: hashing::hex(hash.finalize().into()),
            },
        })
    }
}

fn invalid(message: impl Into<String>) -> MediaError {
    MediaError::Contract(message.into())
}
