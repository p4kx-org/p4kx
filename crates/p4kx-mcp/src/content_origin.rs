// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    Archive, Entry, EntryBytes, FieldOrigin, InputScope, ObservedEntry, SourceEvidenceError,
    SourceInput, SourceInventory, SourceLocation, SourceScope, SourceUseRole, SourceUses,
};
use schemars::JsonSchema;
use serde::Serialize;

#[derive(Debug, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct BytesFieldOrigins {
    #[serde(rename = "/structuredContent/archive")]
    archive: FieldOrigin,
    #[serde(rename = "/structuredContent/path")]
    path: FieldOrigin,
    #[serde(rename = "/structuredContent/size")]
    size: FieldOrigin,
    #[serde(rename = "/structuredContent/offset")]
    offset: FieldOrigin,
    #[serde(rename = "/structuredContent/length")]
    length: FieldOrigin,
    #[serde(rename = "/structuredContent/encoding")]
    encoding: FieldOrigin,
    #[serde(rename = "/structuredContent/data")]
    data: FieldOrigin,
    #[serde(rename = "/structuredContent/more")]
    more: FieldOrigin,
    #[serde(
        rename = "/structuredContent/next_offset",
        skip_serializing_if = "Option::is_none"
    )]
    next_offset: Option<FieldOrigin>,
    #[serde(rename = "/structuredContent/clamped")]
    clamped: FieldOrigin,
    #[serde(rename = "/structuredContent/max_bytes")]
    max_bytes: FieldOrigin,
}

impl BytesFieldOrigins {
    pub(crate) fn capture(
        archive: &Archive,
        entry: &Entry,
        range: &EntryBytes,
    ) -> Result<(Self, SourceInventory), SourceEvidenceError> {
        let uses = SourceUses::new();
        let observed = archive.observed_entry(entry);
        let input = observed.map_or(SourceInput::Unknown, SourceInput::ArchiveView);
        let scope = uses.observe(input, InputScope::Payload)?;
        let complete = observed.is_some_and(|entry| entry.ancestors().all(|step| step.is_some()));
        let directory = scope.for_scope(InputScope::ArchiveDirectoryMetadata);
        let path = path_origin(&uses, &directory, observed.as_ref(), complete)?;
        let size = directory.at(metadata("uncompressed_size"), SourceUseRole::Payload)?;
        let stored_size = uses.origin(std::slice::from_ref(&size), complete);
        let mut window = vec![size];
        if !range.bytes.is_empty() {
            let end = range.offset + range.bytes.len() as u64;
            window.push(scope.at(
                SourceLocation::Subresource {
                    namespace: "archiveEntryDecodedBytes".into(),
                    indices: vec![range.offset, end],
                    field: "bytes".into(),
                },
                SourceUseRole::Payload,
            )?);
        }
        let selected = uses.origin(&window, complete);
        let local = uses.origin(&[], true);
        let fields = Self {
            archive: local.clone(),
            path,
            size: stored_size,
            offset: local.clone(),
            length: selected.clone(),
            encoding: local.clone(),
            data: selected.clone(),
            more: selected.clone(),
            next_offset: range.next_offset().map(|_| selected),
            clamped: local.clone(),
            max_bytes: local,
        };
        Ok((fields, uses.freeze()?))
    }
}

fn path_origin(
    uses: &SourceUses,
    directory: &SourceScope,
    observed: Option<&ObservedEntry<'_>>,
    mut complete: bool,
) -> Result<FieldOrigin, SourceEvidenceError> {
    let mut path = Vec::new();
    for index in 0..observed.map_or(0, |entry| entry.ancestors().len()) {
        if let Some(ancestor) = directory.ancestor(index)? {
            path.push(ancestor.at(metadata("path"), SourceUseRole::Payload)?);
        } else {
            complete = false;
        }
    }
    path.push(directory.at(metadata("path"), SourceUseRole::Payload)?);
    Ok(uses.origin(&path, complete))
}

fn metadata(field: &str) -> SourceLocation {
    SourceLocation::Metadata {
        field: field.into(),
    }
}
