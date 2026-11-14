// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, Entry, EntryMetadataInfo, FieldOrigin, FieldOrigins, SourceUses};

use crate::content_origin::EntryPageSources;
use crate::entry_directory_origins::{
    self, DirectoryOrigins, EntryDirectoryOrigins, EvidenceError,
};

pub(crate) fn capture<'entry>(
    archive: &Archive,
    rows: impl IntoIterator<Item = (&'entry Entry, &'entry EntryMetadataInfo)>,
    has_cursor: bool,
) -> Result<DirectoryOrigins, EvidenceError> {
    let owner = archive.container_input_origin();
    let uses = SourceUses::with_compact_use_refs();
    let survey = if let Some(owner) = owner.as_ref() {
        let owner = EntryPageSources::from_origin_into(Some(owner), uses.clone())?;
        Survey::container(archive, owner)?
    } else {
        Survey::root(archive, &uses)?
    };
    let mut fields = FieldOrigins::new();
    let local = uses.origin(&[], true);
    put(&mut fields, "archive", local.clone())?;
    put(&mut fields, "filter", local)?;
    if let Some(container) = survey.container {
        put(&mut fields, "container", container)?;
    }
    put(&mut fields, "total", survey.count.clone())?;
    if has_cursor {
        put(&mut fields, "next_cursor", survey.count)?;
    }
    let mut contributors = Vec::new();
    let mut complete = true;
    for (ordinal, (entry, metadata)) in rows.into_iter().enumerate() {
        let row = EntryDirectoryOrigins::observe(archive, entry, uses.clone())?;
        let pointer = format!("/structuredContent/entries/{ordinal}");
        contributors.extend(row.write(&mut fields, &pointer, metadata)?);
        complete &= row.complete();
    }
    put(&mut fields, "entries", uses.origin(&contributors, complete))?;
    Ok(DirectoryOrigins {
        field_origins: fields,
        provenance: uses.freeze()?,
    })
}

struct Survey {
    count: FieldOrigin,
    container: Option<FieldOrigin>,
}

impl Survey {
    fn container(archive: &Archive, owner: EntryPageSources) -> Result<Self, EvidenceError> {
        let paths = entry_directory_origins::container_directory_paths(archive, &owner)?;
        Ok(Self {
            count: owner.origin_when(&paths.sources, paths.complete),
            container: Some(owner.path),
        })
    }

    fn root(archive: &Archive, uses: &SourceUses) -> Result<Self, EvidenceError> {
        let consulted = uses.observe_root_directory_paths(archive)?;
        Ok(Self {
            count: uses.origin(consulted.as_slice(), consulted.is_some()),
            container: None,
        })
    }
}

fn put(fields: &mut FieldOrigins, name: &str, origin: FieldOrigin) -> Result<(), EvidenceError> {
    entry_directory_origins::put(fields, &format!("/structuredContent/{name}"), origin)
}
