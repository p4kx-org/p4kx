// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{FieldPath, InputScope, SourceUseRole};

use crate::origin_capture::OriginSite;
use crate::origin_contract::OriginProfile;
use crate::origin_prepared::PreparedRecordOrigins;
use crate::origin_resolver::BuiltinOrigins;
use crate::{MediaError, MediaWriter, SourceContext, Subresource, origin_error};

const ENTRY_FIELDS: &[&str] = &[
    "metadata",
    "compressed_size",
    "uncompressed_size",
    "declared_compression",
    "encrypted",
    "installed_bytes",
    "stored_sha256",
    "stored_sha256_checked_by_reader",
];

pub(crate) fn entry(
    context: &SourceContext<'_>,
    subresource: &Subresource,
    writer: &MediaWriter<'_>,
) -> Result<PreparedRecordOrigins, MediaError> {
    let input = context.origin_input(writer)?;
    let mut fields = writer.record_origins();
    let mut aggregate = writer.capture_origins();
    for field in ENTRY_FIELDS {
        let mut capture = writer.capture_origins();
        capture.add(
            &input,
            InputScope::ArchiveDirectoryMetadata,
            OriginSite::new(&subresource.namespace, &subresource.indices, field),
            SourceUseRole::Payload,
            writer,
        )?;
        let contributions = capture.finish();
        aggregate.include(&contributions, writer)?;
        let mut path = FieldPath::parse("/data").map_err(|error| origin_error::address(&error))?;
        path.push_field(field)
            .map_err(|error| origin_error::address(&error))?;
        fields.insert(&path, contributions, writer)?;
    }
    fields.insert(
        &FieldPath::parse("/data").map_err(|error| origin_error::address(&error))?,
        aggregate.finish(),
        writer,
    )?;
    fields.set_builtin(BuiltinOrigins {
        profile: OriginProfile::ArchiveEntry,
        input,
        scope: InputScope::ArchiveDirectoryMetadata,
    })?;
    Ok(fields)
}
