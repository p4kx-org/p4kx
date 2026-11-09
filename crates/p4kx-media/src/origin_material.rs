// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::model::MaterialLibrary;
use p4kx_extraction::{FieldPath, InputScope, SourceUseRole};

use crate::material_payload::MaterialFailureReason;
use crate::material_read_origins::MaterialReadOrigins;
use crate::origin_capture::OriginSite;
use crate::origin_contract::OriginProfile;
use crate::origin_prepared::PreparedRecordOrigins;
use crate::origin_resolver::BuiltinOrigins;
use crate::{MediaError, MediaWriter, SourceContext, origin_error};

pub(crate) fn library(
    context: &SourceContext<'_>,
    library: &MaterialLibrary,
    reads: MaterialReadOrigins,
    writer: &MediaWriter<'_>,
) -> Result<PreparedRecordOrigins, MediaError> {
    let input = context.origin_input(writer)?;
    let mut prepared = reads.finish(&input, writer)?;
    let mut palette = writer.capture_origins();
    let role = if library.default_palette.is_some() {
        SourceUseRole::Payload
    } else {
        SourceUseRole::Selector
    };
    palette.add(
        &input,
        InputScope::Payload,
        OriginSite::new("material.xml", &[], "DefaultPalette"),
        role,
        writer,
    )?;
    let path = FieldPath::parse("/data/payload/default_palette")
        .map_err(|cause| origin_error::address(&cause))?;
    prepared.insert(&path, palette.finish(), writer)?;
    prepared.set_builtin(BuiltinOrigins {
        profile: OriginProfile::MaterialLibrary,
        input,
        scope: InputScope::Payload,
    })?;
    Ok(prepared)
}

pub(crate) fn failure(
    context: &SourceContext<'_>,
    reason: &MaterialFailureReason,
    writer: &MediaWriter<'_>,
) -> Result<PreparedRecordOrigins, MediaError> {
    let mut prepared = writer.record_origins();
    let profile = match reason {
        MaterialFailureReason::InvalidLimits => OriginProfile::MaterialConfigurationFailure,
        MaterialFailureReason::SourceBytes { .. } => OriginProfile::MaterialSourceLimit,
        MaterialFailureReason::UnsupportedReader => return Ok(prepared),
        MaterialFailureReason::TextEncoding
        | MaterialFailureReason::Xml
        | MaterialFailureReason::CryXml
        | MaterialFailureReason::NotMaterial
        | MaterialFailureReason::XmlDepth
        | MaterialFailureReason::TypedLimit { .. }
        | MaterialFailureReason::SourceSpan => OriginProfile::MaterialFailure,
    };
    let scope = if profile == OriginProfile::MaterialFailure {
        InputScope::Payload
    } else {
        InputScope::ArchiveDirectoryMetadata
    };
    prepared.set_builtin(BuiltinOrigins {
        profile,
        input: context.origin_input(writer)?,
        scope,
    })?;
    Ok(prepared)
}
