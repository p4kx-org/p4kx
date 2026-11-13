// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    Archive, FieldOrigin, FieldOrigins, FieldPath, MetadataRange, SourceCoordinateOmission,
    SourceInventory, SourceLocation, SourceUse, SourceUseRole,
};
use schemars::JsonSchema;
use serde::Serialize;

pub(crate) use p4kx_extraction::{
    EntryDirectoryOrigins, EntryDirectoryOriginsError as EvidenceError,
};

use crate::content_origin::EntryPageSources;

pub(crate) fn directory_range(
    namespace: &str,
    field: &str,
    range: MetadataRange,
) -> SourceLocation {
    SourceLocation::Subresource {
        namespace: namespace.into(),
        indices: vec![range.offset(), range.offset() + range.len()],
        field: field.into(),
    }
}

pub(crate) fn put(
    fields: &mut FieldOrigins,
    address: &str,
    origin: FieldOrigin,
) -> Result<(), EvidenceError> {
    Ok(fields.insert(FieldPath::parse(address)?, origin)?)
}

#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct DirectoryOrigins {
    #[serde(rename = "fieldOrigins")]
    pub(crate) field_origins: FieldOrigins,
    pub(crate) provenance: SourceInventory,
}

#[derive(Clone)]
pub(crate) struct PathUses {
    pub(crate) sources: Vec<SourceUse>,
    pub(crate) complete: bool,
}

pub(crate) fn container_directory_paths(
    archive: &Archive,
    owner: &EntryPageSources,
) -> Result<PathUses, EvidenceError> {
    let mut sources = owner.path_uses.clone();
    let layout = archive.metadata().zip_directory();
    if let Some(layout) = layout {
        let payload = owner.payload_scope();
        for (name, extent) in [
            ("recordPaths", layout.records()),
            ("zipEnd", layout.end().fixed()),
        ]
        .into_iter()
        .chain(layout.zip64().map(|end| ("zip64End", end.fixed())))
        .chain(
            layout
                .locator()
                .map(|locator| ("zip64Locator", locator.range())),
        ) {
            sources.push(payload.at(
                directory_range("containerDirectory", name, extent),
                SourceUseRole::Selector,
            )?);
        }
    } else {
        sources.push(owner.payload_scope().at_opaque(
            SourceUseRole::Selector,
            SourceCoordinateOmission::Unavailable,
        )?);
    }
    Ok(PathUses {
        sources,
        complete: layout.is_some() && owner.complete(),
    })
}
