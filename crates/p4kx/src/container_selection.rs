// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use anyhow::Result;
use p4kx_assembly::ContainerAmbiguity;
use p4kx_extraction::{Archive, Entry, UnknownEntries};

/// Selects one physical container Entry at the given depth of the explicit route.
pub(crate) fn select<'archive>(
    archive: &'archive Archive,
    path: &str,
    depth: usize,
) -> Result<&'archive Entry> {
    let found = p4kx_extraction::select_entries(archive, &[path])?;
    match found.as_slice() {
        &[entry] => Ok(entry),
        [] => Err(UnknownEntries(vec![path.to_owned()]).into()),
        matches => Err(ContainerAmbiguity::new(path, depth, matches).into()),
    }
}
