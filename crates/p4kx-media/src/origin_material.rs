// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::InputScope;

use crate::material_payload::MaterialFailureReason;

use crate::origin_contract::OriginProfile;
use crate::origin_prepared::PreparedRecordOrigins;
use crate::origin_resolver::BuiltinOrigins;
use crate::{MediaError, MediaWriter, SourceContext};

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
