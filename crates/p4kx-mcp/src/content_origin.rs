// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_extraction::cryxml::XmlWriteField;
use p4kx_extraction::{
    Archive, CryXmlFieldUses, Entry, EntryBytes, EntryOrigin, FieldOrigin, InputScope,
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
    pub(crate) path_uses: Vec<SourceUse>,
    payload: SourceScope,
    directory: SourceScope,
    complete: bool,
}

impl EntryPageSources {
    pub(crate) fn observe(archive: &Archive, entry: &Entry) -> Result<Self, SourceEvidenceError> {
        Self::observe_into(archive, entry, SourceUses::new())
    }

    pub(crate) fn observe_with_compact_use_refs(
        archive: &Archive,
        entry: &Entry,
    ) -> Result<Self, SourceEvidenceError> {
        Self::observe_into(archive, entry, SourceUses::with_compact_use_refs())
    }

    pub(crate) fn observe_into(
        archive: &Archive,
        entry: &Entry,
        uses: SourceUses,
    ) -> Result<Self, SourceEvidenceError> {
        let observed = archive.observed_entry(entry);
        let input = observed.map_or(SourceInput::Unknown, SourceInput::ArchiveView);
        let ancestors = observed.map_or(0, |entry| entry.ancestors().len());
        let complete = observed.is_some_and(|entry| entry.ancestors().all(|step| step.is_some()));
        Self::bind(uses, input, ancestors, complete)
    }

    pub(crate) fn from_origin(origin: Option<&EntryOrigin>) -> Result<Self, SourceEvidenceError> {
        Self::from_origin_into(origin, SourceUses::with_compact_use_refs())
    }

    pub(crate) fn from_origin_into(
        origin: Option<&EntryOrigin>,
        uses: SourceUses,
    ) -> Result<Self, SourceEvidenceError> {
        let input = origin.map_or(SourceInput::Unknown, SourceInput::Archive);
        let ancestors = origin.map_or(0, |entry| entry.ancestors().len());
        let complete = origin.is_some_and(|entry| entry.ancestors().all(|step| step.is_some()));
        Self::bind(uses, input, ancestors, complete)
    }

    fn bind(
        uses: SourceUses,
        input: SourceInput<'_>,
        ancestors: usize,
        complete: bool,
    ) -> Result<Self, SourceEvidenceError> {
        let payload = uses.observe(input, InputScope::Payload)?;
        let directory = payload.for_scope(InputScope::ArchiveDirectoryMetadata);
        let (path_uses, complete_path) =
            p4kx_extraction::directory_path_uses(&directory, ancestors, complete)?;
        let path = uses.origin(&path_uses, complete_path);
        Ok(Self {
            uses,
            path,
            path_uses,
            payload,
            directory,
            complete,
        })
    }

    pub(crate) const fn payload_scope(&self) -> &SourceScope {
        &self.payload
    }

    pub(crate) const fn complete(&self) -> bool {
        self.complete
    }

    pub(crate) fn directory_subresource(
        &self,
        location: SourceLocation,
        role: SourceUseRole,
    ) -> Result<SourceUse, SourceEvidenceError> {
        self.directory.at(location, role)
    }

    pub(crate) fn leaf_path(&self) -> Result<SourceUse, SourceEvidenceError> {
        self.directory
            .at(metadata("path"), SourceUseRole::Conversion)
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

    pub(crate) fn subresource(
        &self,
        location: SourceLocation,
        role: SourceUseRole,
    ) -> Result<SourceUse, SourceEvidenceError> {
        self.payload.at(location, role)
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

fn metadata(field: &str) -> SourceLocation {
    SourceLocation::Metadata {
        field: field.into(),
    }
}
