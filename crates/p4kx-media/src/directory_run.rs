// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::Cell;
use std::io;

use p4kx_extraction::{Archive, MetadataCost};

use crate::{DirectoryContext, DirectoryLookup, MediaError, MediaWriter, Provider};

pub(crate) fn run(
    archive: &Archive,
    lookup: &DirectoryLookup<'_>,
    providers: &[&dyn Provider],
    bytes: &mut u64,
    work: &Cell<u64>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let view = archive.metadata_view();
    let source = lookup.owning_source(view.source_len());
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
            work.get(),
            cost.work_items(),
            limits.metadata_work,
            "metadata_work",
        )
        .map_err(io::Error::other)?;
        *bytes += cost.read_bytes();
        work.set(work.get() + cost.work_items());
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
                    lookup,
                    access: &mut access,
                },
                writer,
            )
            .map_err(|cause| crate::error::at(&source, cause))?;
        writer.end_directory()?;
    }
    checkpoint.check()
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
