// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::VecDeque;
use std::io::Read;

use p4kx_extraction::model::{
    MaterialError, MaterialLibrary, MaterialReadError, MaterialSourceLimits,
};
use p4kx_extraction::{Archive, ArchiveWatch, Entry};
use sha2::{Digest as _, Sha256};

use crate::checkpoint::IO_BLOCK_BYTES;
use crate::directory_lookup::DirectoryLookup;
use crate::directory_run::{self, Phase};
use crate::input_budget::InputBudget;
use crate::source_selection::EntrySelection;
use crate::traversal_visit::TraversalVisit;
use crate::{
    Algorithm, ByteScope, ContainerIdentity, Digest, Identity, MediaError, MediaWriter, Provider,
    SourceContext, error, hashing, material_projection, raw_output, source, traversal_container,
};

use super::sink::Sink;
use super::{TestResult, require};

#[derive(Default)]
pub(super) struct Inputs {
    pub(super) enumerated: u64,
    pub(super) selected: VecDeque<Identity>,
}

impl Inputs {
    pub(super) fn collect(archive: &Archive, watch: &ArchiveWatch) -> TestResult<Self> {
        let mut inputs = Self::default();
        inputs.walk(archive, watch, &mut Vec::new())?;
        Ok(inputs)
    }

    fn walk(
        &mut self,
        archive: &Archive,
        watch: &ArchiveWatch,
        chain: &mut Vec<ContainerIdentity>,
    ) -> TestResult {
        require(
            !archive
                .issues()
                .iter()
                .any(|(_, issue)| issue.is_archive_wide()),
            "material enumeration crossed an incomplete archive",
        )?;
        for entry in archive.entries() {
            watch.poll()?;
            self.enumerated += 1;
            let extension = entry
                .path()
                .rsplit_once('.')
                .map(|(_, extension)| extension);
            if extension.is_some_and(|ext| ext.eq_ignore_ascii_case("mtl")) {
                self.selected.push_back(Identity::Entry {
                    containers: chain.clone(),
                    entry: ContainerIdentity::try_new_with_entry(archive, entry)?,
                });
            }
            if traversal_container::is_container(entry.path()) {
                let nested = archive.open_container(entry)?;
                chain.push(ContainerIdentity::try_new_with_entry(archive, entry)?);
                self.walk(&nested, watch, chain)?;
                let _parent = chain.pop();
            }
        }
        Ok(())
    }
}

pub(super) struct Visit<'a> {
    providers: &'a [&'a dyn Provider],
    budget: &'a InputBudget,
    sink: &'a Sink,
    read_buffer: Vec<u8>,
}

impl<'a> Visit<'a> {
    pub(super) fn new(
        providers: &'a [&'a dyn Provider],
        budget: &'a InputBudget,
        sink: &'a Sink,
    ) -> Self {
        Self {
            providers,
            budget,
            sink,
            read_buffer: vec![0; IO_BLOCK_BYTES],
        }
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
        if selection != EntrySelection::Included {
            return Ok(false);
        }
        let declared = source::entry(lookup.root, lookup.index.archive, entry, lookup.chain)?;
        let inputs = lookup.input_state()?;
        if inputs.begin(entry.index(), writer)?.is_some() {
            return Err(invalid("material payload already retained"));
        }
        let result = self.project(entry, lookup, declared, writer);
        let raw = result.as_ref().ok().cloned();
        error::finish(result.map(|_| true), inputs.end(entry.index(), raw))
    }

    fn excluded_container(&mut self) {}
}

impl Visit<'_> {
    fn project(
        &mut self,
        entry: &Entry,
        lookup: &DirectoryLookup<'_>,
        mut source: crate::Source,
        writer: &mut MediaWriter<'_>,
    ) -> Result<crate::RawReference, MediaError> {
        let archive = lookup.index.archive;
        let limits = MaterialSourceLimits::default();
        let observed = Payload::read(
            archive,
            entry,
            limits,
            self.budget,
            writer,
            &mut self.read_buffer,
        )?;
        let expected = observed.parse(entry, limits, writer)?;
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
        let projected = material_projection::project_with(
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
        writer.end_source(projected)?;
        self.sink
            .check_entry(&source, &expected, entry.uncompressed_size())?;
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
