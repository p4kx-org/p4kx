// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde_json::Value;

use crate::archive_metadata_diagnostic::ArchiveMetadataDiagnostic;
use crate::archive_raw::ArchiveRawChunk;
use crate::archive_record::{ArchiveDirectoryRecord, ArchiveExtraData};
use crate::archive_section::ArchiveSectionData;
use crate::archive_table::{ArchiveFreeRange, ArchiveInstallation};
use crate::{
    MediaError, archive_extras_read, archive_metadata_diagnostic, archive_records_read,
    archive_retention, archive_sections_read, archive_tables_read, schema_merge,
};
use p4kx_extraction::ArchiveLayoutInfo;

pub(crate) fn bind_record_schema(mut base: Value) -> Result<Value, MediaError> {
    bind::<ArchiveLayoutInfo>(
        &mut base,
        crate::archive_layout_projection::LAYOUT_NAMESPACE,
        "ArchiveLayoutInfo",
    )?;
    bind::<ArchiveSectionData>(
        &mut base,
        archive_sections_read::SECTION_NAMESPACE,
        "ArchiveSectionData",
    )?;
    bind::<ArchiveRawChunk>(
        &mut base,
        archive_retention::RAW_NAMESPACE,
        "ArchiveRawChunk",
    )?;
    bind::<ArchiveDirectoryRecord>(
        &mut base,
        archive_records_read::RECORD_NAMESPACE,
        "ArchiveDirectoryRecord",
    )?;
    bind::<ArchiveExtraData>(
        &mut base,
        archive_extras_read::EXTRA_NAMESPACE,
        "ArchiveExtraData",
    )?;
    bind::<ArchiveInstallation>(
        &mut base,
        archive_tables_read::INSTALL_NAMESPACE,
        "ArchiveInstallation",
    )?;
    bind::<ArchiveFreeRange>(
        &mut base,
        archive_tables_read::FREE_NAMESPACE,
        "ArchiveFreeRange",
    )?;
    bind::<ArchiveMetadataDiagnostic>(
        &mut base,
        archive_metadata_diagnostic::DIAGNOSTIC_NAMESPACE,
        "ArchiveMetadataDiagnostic",
    )?;
    let base = crate::archive_layout_status_schema::bind(base)?;
    crate::archive_layout_consistency_schema::bind(base)
}

fn bind<T: JsonSchema>(base: &mut Value, namespace: &str, name: &str) -> Result<(), MediaError> {
    let mut payload = schema_merge::serialized_payload::<T>();
    if let Some(entry) = payload.pointer_mut("/$defs/EntryMetadataInfo") {
        let entry = entry.as_object_mut().ok_or_else(|| {
            MediaError::Contract("physical Entry metadata schema is not an object".into())
        })?;
        entry
            .entry("title")
            .or_insert_with(|| Value::String("EntryMetadataInfo".into()));
    }
    schema_merge::insert_payload(base, name, payload)?;
    let root = base
        .as_object_mut()
        .ok_or_else(|| MediaError::Contract("media record schema is not an object".into()))?;
    let constraints = root
        .entry("allOf")
        .or_insert_with(|| Value::Array(Vec::new()))
        .as_array_mut()
        .ok_or_else(|| MediaError::Contract("media record allOf is not an array".into()))?;
    constraints.push(serde_json::json!({
        "if": {"properties": {"kind": {"const": "archive"}, "subresource": {"properties": {"namespace": {"const": namespace}}, "required": ["namespace"]}}, "required": ["kind", "subresource"]},
        "then": {"properties": {"data": {"$ref": format!("#/$defs/{name}")}}, "required": ["data"]}
    }));
    Ok(())
}
