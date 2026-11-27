// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Reading the files inside a container (`.socpak`).

use std::path::Path;

use p4kx_extraction::{
    Archive, Entry, InputScope, SourceInput, SourceLocation, SourceUse, SourceUseRole, SourceUses,
};

use crate::{CatalogError, ExportAccountingError};

/// The largest `.soc` file that is read, 64 MiB.
pub(crate) const MAX_SOC_LEN: u64 = 64 << 20;

/// The extension of the entity file of a container.
const SOC_EXTENSION: &str = "soc";

/// Returns the `.soc` files of `container`, in stored order.
pub(crate) fn soc_entries(container: &Archive) -> impl Iterator<Item = &Entry> {
    container.entries().iter().filter(|inner| {
        Path::new(inner.path())
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case(SOC_EXTENSION))
    })
}

/// Records the complete nested list used to admit physical SOC members.
pub(crate) fn observe_container_directory(
    archive: &Archive,
    sources: &SourceUses,
) -> Result<Option<SourceUse>, CatalogError> {
    if crate::export_archive_health::inventory(archive).is_some() {
        return Ok(None);
    }
    let entry = archive
        .container_input_origin()
        .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
    let count = u64::try_from(archive.entries().len())
        .map_err(|_| ExportAccountingError::CounterOverflow)?;
    let scope = sources.observe(SourceInput::Archive(&entry), InputScope::Payload)?;
    Ok(Some(scope.at(
        SourceLocation::Subresource {
            namespace: "socArchiveDirectory".to_owned(),
            indices: vec![0, count],
            field: "entryPaths".to_owned(),
        },
        SourceUseRole::Selector,
    )?))
}
