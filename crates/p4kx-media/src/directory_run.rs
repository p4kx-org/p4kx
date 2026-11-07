// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;

use p4kx_extraction::{Archive, MetadataCost};

use crate::{
    ContainerIdentity, Digest, DirectoryContext, Identity, MediaError, MediaWriter, Provider,
    Source, Verification,
};

pub(crate) fn run(
    archive: &Archive,
    root: &Digest,
    chain: &[ContainerIdentity],
    providers: &[&dyn Provider],
    bytes: &mut u64,
    work: &mut u64,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let view = archive.metadata_view();
    let source = owning_source(root, chain, view.source_len());
    let checkpoint = writer.checkpoint;
    let limits = writer.limits();
    let mut charge = |cost: MetadataCost| {
        checkpoint.check().map_err(io::Error::other)?;
        admit(
            *bytes,
            cost.read_bytes(),
            limits.source_bytes,
            "source_bytes",
        )
        .map_err(io::Error::other)?;
        admit(
            *work,
            cost.work_items(),
            limits.metadata_work,
            "metadata_work",
        )
        .map_err(io::Error::other)?;
        *bytes += cost.read_bytes();
        *work += cost.work_items();
        Ok(())
    };
    let mut access = view.access(&mut charge, checkpoint.cancel.flag());
    for (index, provider) in providers.iter().enumerate() {
        writer.begin(index, source.clone())?;
        provider
            .directory(
                &mut DirectoryContext {
                    source: &source,
                    view,
                    entries: archive.entries(),
                    access: &mut access,
                },
                writer,
            )
            .map_err(|cause| crate::error::at(&source, cause))?;
        writer.end_directory()?;
    }
    checkpoint.check()
}

fn owning_source(root: &Digest, chain: &[ContainerIdentity], size: u64) -> Source {
    let identity = match chain.split_last() {
        Some((entry, containers)) => Identity::Entry {
            containers: containers.to_vec(),
            entry: entry.clone(),
        },
        None => Identity::Root,
    };
    Source {
        root: root.clone(),
        identity,
        size,
        declared_integrity: None,
        verification: Verification::NotAvailable,
        raw: None,
    }
}

const fn admit(
    current: u64,
    added: u64,
    limit: u64,
    budget: &'static str,
) -> Result<(), MediaError> {
    if added > limit.saturating_sub(current) {
        return Err(MediaError::Limit { budget, limit });
    }
    Ok(())
}
