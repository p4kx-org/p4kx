// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use thiserror::Error;

use crate::texture_preview::PreviewHeaderEvidence;
use crate::{
    DdsHeaderField, DdsSourceInfo, FieldOrigin, FieldOrigins, FieldOriginsError, FieldPath,
    FieldPathError, SourceEvidenceError, SourceUseRole, SourceUses,
};

#[path = "dds_info_origin_words.rs"]
mod words;

/// Why an exact DDS metadata field origin could not be recorded.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum DdsInfoOriginError {
    /// The original source-use registry rejected the observation.
    #[error(transparent)]
    Evidence(#[from] SourceEvidenceError),
    /// The caller's output field address was invalid.
    #[error(transparent)]
    Address(#[from] FieldPathError),
    /// The caller assigned duplicate origin ownership.
    #[error(transparent)]
    Field(#[from] FieldOriginsError),
}

/// Adds exact original DDS metadata field origins below the caller's output address.
///
/// # Errors
/// Returns original source registry, field-address or duplicate-field errors.
pub fn dds_info_origins(
    fields: &mut FieldOrigins,
    base: &FieldPath,
    dds: &DdsSourceInfo,
    header: &PreviewHeaderEvidence,
    uses: &SourceUses,
) -> Result<(), DdsInfoOriginError> {
    let base = base.as_str();
    words::capture(fields, dds, header, uses, base)?;
    normalized(fields, dds, header, uses, base)?;
    if dds.transfer.is_some() {
        for (field, words) in crate::dds_source_info::DDS_TRANSFER_ORIGIN_FIELDS {
            let mut contributors =
                header.header_uses(&[DdsHeaderField::Prefix], SourceUseRole::Selector)?;
            contributors.extend(header.header_uses(words, SourceUseRole::Conversion)?);
            insert(
                fields,
                &format!("{base}/transfer/{field}"),
                uses.origin(&contributors, true),
            )?;
        }
    }
    insert(
        fields,
        &format!("{base}/header_source_bytes"),
        uses.origin(&[header.size_use()?], true),
    )?;
    insert(
        fields,
        &format!("{base}/schema_version"),
        uses.origin(&[], true),
    )
}

fn normalized(
    fields: &mut FieldOrigins,
    dds: &DdsSourceInfo,
    header: &PreviewHeaderEvidence,
    uses: &SourceUses,
    base: &str,
) -> Result<(), DdsInfoOriginError> {
    use DdsHeaderField::{ArraySize, Depth, Faces, Format, Height, MipCount, Prefix, Width};
    for (path, field) in [
        ("width", Width),
        ("height", Height),
        ("depth", Depth),
        ("mip_count", MipCount),
        ("faces", Faces),
        ("cubemap", Faces),
        ("array_size", ArraySize),
        ("format", Format),
        ("reader_high_range", Format),
        ("block_layout", Format),
    ] {
        let contributors = header.header_uses(&[Prefix, field], SourceUseRole::Conversion)?;
        let origin = uses.origin(&contributors, true);
        let path = format!("{base}/normalized/{path}");
        if path.ends_with("/block_layout") && dds.normalized.block_layout.is_some() {
            for field in ["block_width", "block_height", "bytes_per_block"] {
                insert(fields, &format!("{path}/{field}"), origin.clone())?;
            }
        }
        insert(fields, &path, origin)?;
    }
    Ok(())
}

fn insert(
    fields: &mut FieldOrigins,
    path: &str,
    origin: FieldOrigin,
) -> Result<(), DdsInfoOriginError> {
    Ok(fields.insert(FieldPath::parse(path)?, origin)?)
}
