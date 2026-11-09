// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;

use p4kx_extraction::{Archive, MetadataCost};

use crate::input_budget::InputBudget;

use crate::{DirectoryContext, DirectoryLookup, MediaError, MediaWriter, Provider};

pub(crate) fn run(
    archive: &Archive,
    lookup: &DirectoryLookup<'_>,
    providers: &[&dyn Provider],
    budget: &InputBudget,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let view = archive.metadata_view();
    let source = lookup.owning_source(view.source_len());
    let checkpoint = writer.checkpoint;
    let limits = writer.limits();
    let mut charge = |cost: MetadataCost| {
        budget
            .metadata(cost.read_bytes(), cost.work_items(), limits, checkpoint)
            .map_err(io::Error::other)?;
        Ok(())
    };
    let mut access = view.access(&mut charge, checkpoint.cancel.flag());
    for (index, provider) in providers.iter().enumerate() {
        let input = writer.origins.directory().cloned().ok_or_else(|| {
            MediaError::Contract("directory callback has no original input".into())
        })?;
        writer.begin_with_input(index, source.clone(), input)?;
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
