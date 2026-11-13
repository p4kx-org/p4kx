// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, Filter, OwnedDirectoryPermit};
use std::borrow::Cow;
use std::cell::Cell;

use crate::archive_provider;
use crate::checkpoint::Checkpoint;
use crate::hashing;
use crate::input_budget::InputBudget;
use crate::source;
use crate::traversal::Traversal;
use crate::{
    ByteScope, ExportOptions, Index, MediaError, MediaWriter, Provider, RootSource, Tally,
};

pub(crate) fn build(
    archive: &Archive,
    stage: &OwnedDirectoryPermit,
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
    let mut root = source::root(archive, digest, size);
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
    writer.audio_output = options.audio_output;
    writer.texture_mip = options.texture_mip;
    writer
        .animation_skeleton
        .clone_from(&options.animation_skeleton);
    writer.origins.observe_root(archive, size)?;
    writer.bind_build(archive, &mut root)?;
    root_callbacks(&root, providers, &mut writer)?;
    let entries = collect(archive, options, &root, providers, &mut writer)?;
    writer.index_build(&root)?;
    root.build_origin = None;
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
        field_origins: Some(finished.origins),
    };
    index.generation = hashing::generation(&mut index, &checkpoint)?;
    Ok(index)
}

fn root_callbacks(
    root: &RootSource,
    providers: &[&dyn Provider],
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    for (index, provider) in providers.iter().enumerate() {
        let input = writer.origins.root.clone().ok_or_else(|| {
            MediaError::Contract("completed root read lost its original evidence".into())
        })?;
        writer.begin_with_input(index, archive_provider::root_source(root), input)?;
        provider.root(root, writer)?;
    }
    Ok(())
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
    let budget = InputBudget::default();
    let index_keys = Cell::new(0);
    let shader_lookup = crate::shader_lookup::ShaderLookup::new(
        archive,
        &root.digest,
        crate::source_selection::SourceSelection {
            include: &include,
            exclude: &options.exclude,
        },
        &budget,
    );
    let mut traversal = Traversal {
        root: &root.digest,
        providers,
        include: &include,
        exclude: &options.exclude,
        entries: Tally::default(),
        budget: &budget,
        index_keys: &index_keys,
        shaders: Some(&shader_lookup),
    };
    traversal.walk(archive, &mut Vec::new(), writer)?;
    Ok(traversal.entries)
}
