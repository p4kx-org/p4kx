// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;
use std::path::Path;

use p4kx_extraction::{Archive, Filter};

use crate::archive_provider;
use crate::checkpoint::Checkpoint;
use crate::hashing;
use crate::source;
use crate::traversal::Traversal;
use crate::{
    ByteScope, ExportOptions, Index, MediaError, MediaWriter, Provider, RootSource, Tally,
};

pub(crate) fn build(
    archive: &Archive,
    stage: &Path,
    options: &ExportOptions,
    providers: &[&dyn Provider],
    checkpoint: Checkpoint<'_>,
) -> Result<Index, MediaError> {
    let (digest, size) = hashing::file(
        archive.path(),
        ByteScope::RootArchiveFile,
        options.limits.root_bytes,
        &checkpoint,
    )?;
    checkpoint.finish()?;
    let root = source::root(archive, digest, size);
    let capabilities = providers
        .iter()
        .map(|provider| provider.capability())
        .collect();
    let mut writer = MediaWriter::new(
        stage,
        capabilities,
        options.limits,
        checkpoint,
        options.diagnostic,
    )?;
    for (index, provider) in providers.iter().enumerate() {
        writer.begin(index, archive_provider::root_source(&root))?;
        provider.root(&root, &mut writer)?;
    }
    let entries = collect(archive, options, &root, providers, &mut writer)?;
    let finished = writer.finish()?;
    let complete = root.enumeration_complete
        && finished
            .kinds
            .iter()
            .all(|kind| kind.counts.records.failed == 0 && kind.counts.records.partial == 0);
    let mut index = Index {
        format: crate::MediaFormat::P4kxMedia,
        format_version: crate::MediaVersion::V1,
        complete,
        enumeration_complete: root.enumeration_complete,
        source: root,
        generation: String::new(),
        artifact_base: String::new(),
        limits: options.limits,
        source_entries: entries,
        kinds: finished.kinds,
        source_manifest: finished.sources,
        artifact_manifest: finished.artifacts,
    };
    index.generation = hashing::generation(&mut index, &checkpoint)?;
    Ok(index)
}

fn collect(
    archive: &Archive,
    options: &ExportOptions,
    root: &RootSource,
    providers: &[&dyn Provider],
    writer: &mut MediaWriter<'_>,
) -> Result<Tally, MediaError> {
    let include = match &options.include {
        Some(include) => Cow::Borrowed(include),
        None => {
            Cow::Owned(Filter::parse("*").map_err(|error| MediaError::Contract(error.to_string()))?)
        }
    };
    let mut traversal = Traversal {
        root: &root.digest,
        providers,
        include: &include,
        exclude: &options.exclude,
        entries: Tally::default(),
        bytes: 0,
    };
    traversal.walk(archive, &mut Vec::new(), writer)?;
    Ok(traversal.entries)
}
