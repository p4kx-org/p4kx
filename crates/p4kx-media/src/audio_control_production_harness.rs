// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::Cell;
use std::io::{self, Read};
use std::path::Path;

use p4kx_extraction::sound::wwise_path;
use p4kx_extraction::{
    Archive, ArchiveWatch, ArchiveWriteProtection, CancelToken, Entry, Exclusion, Filter,
};
use sha2::{Digest as _, Sha256};

use crate::artifact::StreamObserver;
use crate::audio_control_projection::Projection as AtlProjection;
use crate::checkpoint::{Checkpoint, IO_BLOCK_BYTES};
use crate::directory_index::DirectoryIndex;
use crate::directory_inputs::DirectoryInputs;
use crate::directory_lookup::DirectoryLookup;
use crate::directory_run::{self, Phase};
use crate::input_budget::InputBudget;
use crate::media_writer::Finished;
use crate::source_selection::{EntrySelection, SourceSelection};
use crate::{
    Algorithm, ByteScope, Digest, Limits, MediaError, MediaWriter, Provider, error, hashing,
    raw_output, source,
};

use crate::audio_control_source::AudioControlSource;

use super::super::{AudioControlProvider, project_bytes};
use super::TestResult;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum StreamPhase {
    HashArchive,
    ProjectDocuments,
    ResolveReferences,
    FinalizeOrigins,
}

pub(super) fn observe(
    archive: &Archive,
    observer: &dyn StreamObserver,
    report: &mut impl FnMut(StreamPhase) -> TestResult,
) -> TestResult<Finished> {
    let cancel = CancelToken::default();
    let watch = ArchiveWatch::new(archive);
    let protection = ArchiveWriteProtection::new(archive.path(), cancel.flag())?;
    let checkpoint = Checkpoint {
        cancel: &cancel,
        watch: &watch,
        protection: &protection,
    };
    let limits = Limits::default();
    report(StreamPhase::HashArchive)?;
    let (digest, size) = hashing::file(
        archive.path(),
        ByteScope::RootArchiveFile,
        limits.root_bytes,
        &checkpoint,
    )?;
    checkpoint.finish()?;
    let mut root = source::root(archive, digest, size);
    let mut writer = MediaWriter::observed(
        Path::new("atl-observation"),
        vec![AudioControlProvider.capability()],
        limits,
        checkpoint,
        false,
        observer,
    )?;
    writer.origins.observe_root(archive, size)?;
    writer.bind_build(archive, &mut root)?;
    let include = Filter::parse(r"Data\Libs\GameAudio\*.xml")?;
    let exclude = Exclusion::parse(["*.pak", "*.socpak"])?;
    report(StreamPhase::ProjectDocuments)?;
    collect(
        archive,
        &root.digest,
        SourceSelection {
            include: &include,
            exclude: &exclude,
        },
        &mut writer,
        report,
    )?;
    report(StreamPhase::FinalizeOrigins)?;
    writer.index_build(&root)?;
    Ok(writer.finish()?)
}

fn collect(
    archive: &Archive,
    root: &Digest,
    selection: SourceSelection<'_>,
    writer: &mut MediaWriter<'_>,
    report: &mut impl FnMut(StreamPhase) -> TestResult,
) -> TestResult {
    if archive.entries().len() as u64 > writer.limits().sources {
        return Err(MediaError::Limit {
            budget: "sources",
            limit: writer.limits().sources,
        }
        .into());
    }
    let budget = InputBudget::default();
    let index_keys = Cell::new(0);
    let index = DirectoryIndex::new(archive, &index_keys);
    let inputs = DirectoryInputs::try_new(archive.entries().len(), &budget, writer)?;
    let lookup = DirectoryLookup::new_with_inputs(&index, root, &[], selection, &inputs);
    let providers: [&dyn Provider; 1] = [&AudioControlProvider];
    directory_run::run(archive, &lookup, &providers, &budget, Phase::Open, writer)?;
    for entry in archive.entries() {
        writer.checkpoint()?;
        if !wwise_path::is_atl_definition(entry.path())
            || selection.classify(&[], entry.path()) != EntrySelection::Included
        {
            continue;
        }
        inputs.admit(entry.index(), writer)?;
        observe_entry(&lookup, &budget, entry, writer)?;
    }
    report(StreamPhase::ResolveReferences)?;
    directory_run::run(
        archive,
        &lookup,
        &providers,
        &budget,
        Phase::Complete,
        writer,
    )?;
    Ok(())
}

fn observe_entry(
    lookup: &DirectoryLookup<'_>,
    budget: &InputBudget,
    entry: &Entry,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let archive = lookup.index.archive;
    let mut source = source::entry(lookup.root, archive, entry, lookup.chain)?;
    let bytes =
        read_payload(archive, entry, budget, writer).map_err(|cause| error::at(&source, cause))?;
    let input = writer.origins.input(archive, entry)?;
    let digest = Digest {
        algorithm: Algorithm::Sha256,
        scope: ByteScope::DecodedEntryPayload,
        hex: hashing::hex(Sha256::digest(&bytes).into()),
    };
    let raw = raw_output::receipt(digest, source.size);
    writer.reserve_output(raw.size)?;
    raw_output::record(&raw, &input, writer)?;
    source::retained(&mut source, entry, raw);
    writer.retained_with_input(&source, &input)?;
    writer.begin_with_input(0, source.clone(), input.clone())?;
    let projection = AtlProjection {
        kind: AudioControlSource::Atl,
        input: &input,
        budget,
        directory: lookup,
        entry: entry.index(),
    };
    let outcome =
        project_bytes(&projection, &bytes, writer).map_err(|cause| error::at(&source, cause))?;
    writer.end_source(outcome)
}

fn read_payload(
    archive: &Archive,
    entry: &Entry,
    budget: &InputBudget,
    writer: &MediaWriter<'_>,
) -> Result<Vec<u8>, MediaError> {
    let size = entry.uncompressed_size();
    let limits = writer.limits();
    if size > limits.entry_bytes {
        return Err(MediaError::Limit {
            budget: "entry_bytes",
            limit: limits.entry_bytes,
        });
    }
    budget.bytes(size, limits)?;
    let native_size = usize::try_from(size).map_err(|_| {
        MediaError::Contract("decoded ATL input exceeds native address width".into())
    })?;
    let path = Path::new(entry.path());
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(native_size)
        .map_err(|cause| error::io(path, io::Error::new(io::ErrorKind::OutOfMemory, cause)))?;
    bytes.resize(native_size, 0);
    let mut reader = archive
        .open_entry_verified(entry)
        .map_err(|cause| MediaError::Archive(Box::new(cause)))?;
    for chunk in bytes.chunks_mut(IO_BLOCK_BYTES) {
        writer.checkpoint()?;
        reader
            .read_exact(chunk)
            .map_err(|cause| error::io(path, cause))?;
    }
    writer.checkpoint()?;
    if reader
        .read(&mut [0])
        .map_err(|cause| error::io(path, cause))?
        != 0
    {
        return Err(MediaError::Contract(
            "decoded ATL length disagrees with its Entry".into(),
        ));
    }
    Ok(bytes)
}
