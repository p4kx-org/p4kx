// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Entry, PathIndex, PathIndexBuildError};

/// Returns every physical match for the first path with any matches.
pub(crate) fn first_matching<'archive, E>(
    index: &PathIndex<'archive>,
    paths: impl IntoIterator<Item = impl AsRef<str>>,
    mut check: impl FnMut() -> Result<(), E>,
) -> Result<Vec<&'archive Entry>, PathIndexBuildError<E>> {
    for path in paths {
        let matches = index
            .matching_entries_with(path.as_ref(), |_| check())?
            .collect::<Result<Vec<_>, _>>()?;
        if !matches.is_empty() {
            return Ok(matches);
        }
    }
    Ok(Vec::new())
}
