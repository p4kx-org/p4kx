// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{FieldPath, InputScope, SourceUseRole};

use crate::origin_capture::OriginSite;
use crate::origin_contract::OriginProfile;
use crate::origin_contributions::OriginContributions;
use crate::origin_input::MediaOriginInput;
use crate::origin_prepared::PreparedRecordOrigins;
use crate::origin_resolver::BuiltinOrigins;
use crate::{MediaError, MediaWriter, origin_error};

pub(crate) fn prepare(input: &MediaOriginInput) -> Result<PreparedRecordOrigins, MediaError> {
    prepare_for(input, OriginProfile::AudioWem)
}

pub(crate) fn prepare_for(
    input: &MediaOriginInput,
    profile: OriginProfile,
) -> Result<PreparedRecordOrigins, MediaError> {
    let mut prepared = PreparedRecordOrigins::new(&input.owner);
    prepared.set_builtin(BuiltinOrigins {
        profile,
        input: input.clone(),
        scope: InputScope::Payload,
    })?;
    Ok(prepared)
}

pub(crate) fn capture<'site>(
    input: &MediaOriginInput,
    sites: impl IntoIterator<Item = (OriginSite<'site>, SourceUseRole)>,
    writer: &MediaWriter<'_>,
) -> Result<OriginContributions, MediaError> {
    let mut capture = writer.capture_origins();
    for (site, role) in sites {
        capture.add(input, InputScope::Payload, site, role, writer)?;
    }
    Ok(capture.finish())
}

pub(crate) fn field(
    prepared: &mut PreparedRecordOrigins,
    path: &str,
    evidence: OriginContributions,
    writer: &MediaWriter<'_>,
) -> Result<(), MediaError> {
    let path = FieldPath::parse(path).map_err(|cause| origin_error::address(&cause))?;
    prepared.insert(&path, evidence, writer)
}

pub(crate) fn subtree(
    prepared: &mut PreparedRecordOrigins,
    path: &str,
    evidence: OriginContributions,
    writer: &MediaWriter<'_>,
) -> Result<(), MediaError> {
    let path = FieldPath::parse(path).map_err(|cause| origin_error::address(&cause))?;
    prepared.insert_subtree(&path, evidence, writer)
}

pub(crate) fn indices(
    prepared: &mut PreparedRecordOrigins,
    evidence: OriginContributions,
    writer: &MediaWriter<'_>,
) -> Result<(), MediaError> {
    field(prepared, "/subresource", evidence.clone(), writer)?;
    subtree(prepared, "/subresource/indices", evidence, writer)
}

pub(crate) fn range(
    prepared: &mut PreparedRecordOrigins,
    path: &str,
    start: &OriginContributions,
    end: &OriginContributions,
    writer: &MediaWriter<'_>,
) -> Result<(), MediaError> {
    field(prepared, path, end.clone(), writer)?;
    field(prepared, &format!("{path}/start"), start.clone(), writer)?;
    field(
        prepared,
        &format!("{path}/end_exclusive"),
        end.clone(),
        writer,
    )
}
