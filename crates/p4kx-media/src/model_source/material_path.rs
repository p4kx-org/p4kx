// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::ArchiveMaterialPaths;

use super::path_reference::Observation;
use crate::{
    DirectoryLookup, MediaError, MediaKind, MediaWriter, OriginalIdentifier, ReferenceResolution,
    ReferenceUnavailable,
};

pub(super) fn observe(
    directory: &DirectoryLookup<'_>,
    original: OriginalIdentifier,
    paths: Result<ArchiveMaterialPaths, ReferenceUnavailable>,
    writer: &mut MediaWriter<'_>,
) -> Result<Observation, MediaError> {
    let paths = match paths {
        Ok(paths) => paths,
        Err(reason) => return Observation::unavailable(original, reason, writer),
    };
    let kind = Some(MediaKind::Material);
    if let Some(sibling) = paths.sibling {
        let observed = directory.lookup_input_observed(original.clone(), &sibling, kind, writer)?;
        if !matches!(
            observed.lookup.reference.resolution(),
            ReferenceResolution::Missing
        ) {
            return Ok(observed.into());
        }
    }
    let observed = directory.lookup_input_observed(original, &paths.data_root, kind, writer)?;
    Ok(observed.into())
}
