// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, Entry};

use crate::input_budget::InputBudget;
use crate::source;
use crate::{ContainerIdentity, Digest, MediaError, MediaWriter};

pub(crate) fn open(
    archive: &Archive,
    entry: &Entry,
    chain: &[ContainerIdentity],
    root: &Digest,
    budget: &InputBudget,
    writer: &MediaWriter<'_>,
) -> Result<Archive, MediaError> {
    writer.checkpoint()?;
    if chain.len() as u64 >= u64::from(writer.limits.depth) {
        return Err(MediaError::Limit {
            budget: "depth",
            limit: u64::from(writer.limits.depth),
        });
    }
    if entry.uncompressed_size() > writer.limits.entry_bytes {
        return Err(MediaError::Limit {
            budget: "entry_bytes",
            limit: writer.limits.entry_bytes,
        });
    }
    budget.bytes(entry.uncompressed_size(), writer.limits())?;
    let container = archive.open_container(entry).map_err(|cause| {
        let failure = MediaError::Archive(Box::new(cause));
        source::entry(root, archive, entry, chain)
            .map_or_else(|error| error, |entry| crate::error::at(&entry, failure))
    })?;
    writer.checkpoint()?;
    Ok(container)
}

pub(crate) fn is_container(path: &str) -> bool {
    path.rsplit('.').next().is_some_and(|extension| {
        extension.eq_ignore_ascii_case("pak") || extension.eq_ignore_ascii_case("socpak")
    })
}
