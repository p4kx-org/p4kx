// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_extraction::cryxml::XmlWriteField;
use p4kx_extraction::{
    Archive, CryXmlFieldUses, Entry, EntryBytes, FieldOrigin, InputScope, ObservedEntry,
    SourceEvidenceError, SourceInput, SourceInventory, SourceLocation, SourceScope, SourceUse,
    SourceUseRole, SourceUses,
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
        let sources = EntryPageSources::observe(archive, entry)?;
        let size = sources.stored_size()?;
        let stored_size = sources.origin(std::slice::from_ref(&size));
        let end = range.offset + range.bytes.len() as u64;
        let mut window = vec![size];
        window.extend(sources.decoded_range(range.offset..end, "bytes", SourceUseRole::Payload)?);
        let selected = sources.origin(&window);
        let local = sources.local();
        let fields = Self {
            archive: local.clone(),
            path: sources.path,
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
        Ok((fields, sources.uses.freeze()?))
    }
}

pub(crate) struct EntryPageSources {
    pub(crate) uses: SourceUses,
    pub(crate) path: FieldOrigin,
    payload: SourceScope,
    directory: SourceScope,
    complete: bool,
}

impl EntryPageSources {
    pub(crate) fn observe(archive: &Archive, entry: &Entry) -> Result<Self, SourceEvidenceError> {
        let uses = SourceUses::new();
        let observed = archive.observed_entry(entry);
        let input = observed.map_or(SourceInput::Unknown, SourceInput::ArchiveView);
        let payload = uses.observe(input, InputScope::Payload)?;
        let complete = observed.is_some_and(|entry| entry.ancestors().all(|step| step.is_some()));
        let directory = payload.for_scope(InputScope::ArchiveDirectoryMetadata);
        let path = path_origin(&uses, &directory, observed.as_ref(), complete)?;
        Ok(Self {
            uses,
            path,
            payload,
            directory,
            complete,
        })
    }

    pub(crate) fn stored_size(&self) -> Result<SourceUse, SourceEvidenceError> {
        self.directory
            .at(metadata("uncompressed_size"), SourceUseRole::Payload)
    }

    pub(crate) fn decoded_range(
        &self,
        range: Range<u64>,
        field: &str,
        role: SourceUseRole,
    ) -> Result<Option<SourceUse>, SourceEvidenceError> {
        if range.is_empty() {
            return Ok(None);
        }
        self.payload
            .at(
                SourceLocation::Subresource {
                    namespace: "archiveEntryDecodedBytes".into(),
                    indices: vec![range.start, range.end],
                    field: field.into(),
                },
                role,
            )
            .map(Some)
    }

    pub(crate) fn cryxml_field(
        &self,
        field: &XmlWriteField,
    ) -> Result<Option<CryXmlFieldUses>, SourceEvidenceError> {
        self.payload.cryxml_field(field)
    }

    pub(crate) fn origin(&self, actual: &[SourceUse]) -> FieldOrigin {
        self.origin_when(actual, true)
    }

    pub(crate) fn origin_when(&self, actual: &[SourceUse], complete: bool) -> FieldOrigin {
        self.uses.origin(actual, self.complete && complete)
    }

    pub(crate) fn local(&self) -> FieldOrigin {
        self.uses.origin(&[], true)
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
