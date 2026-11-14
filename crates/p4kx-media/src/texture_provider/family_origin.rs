// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, InputScope, SourceUseRole};

use crate::origin_capture::OriginSite;
use crate::origin_contract::OriginProfile;
use crate::origin_contributions::OriginContributions;
use crate::origin_input::MediaOriginInput;
use crate::origin_prepared::PreparedRecordOrigins;
use crate::origin_resolver::BuiltinOrigins;
use crate::{MediaError, MediaWriter, origin_error, origin_lookup};

pub(super) fn prepare(owner: &MediaOriginInput) -> Result<PreparedRecordOrigins, MediaError> {
    let mut prepared = PreparedRecordOrigins::new(&owner.owner);
    prepared.set_builtin(BuiltinOrigins {
        profile: OriginProfile::TextureFamily,
        input: owner.clone(),
        scope: InputScope::ArchiveDirectoryMetadata,
    })?;
    Ok(prepared)
}

pub(super) fn entry(
    input: &MediaOriginInput,
    index: u64,
    field: &str,
    role: SourceUseRole,
    writer: &MediaWriter<'_>,
) -> Result<OriginContributions, MediaError> {
    let mut capture = writer.capture_origins();
    capture.add(
        input,
        InputScope::ArchiveDirectoryMetadata,
        OriginSite::new("archive-entry", &[index], field),
        role,
        writer,
    )?;
    Ok(capture.finish())
}

pub(super) fn directory(
    archive: &Archive,
    writer: &MediaWriter<'_>,
) -> Result<OriginContributions, MediaError> {
    let owner = &writer.origins.owner;
    if let Some(range) = owner
        .observe_root_directory_paths(archive)
        .map_err(origin_error::evidence)?
    {
        return Ok(OriginContributions {
            uses: [range].into(),
            ..OriginContributions::independent(owner)
        });
    }
    origin_lookup::directory(writer.origins.directory(), writer)
}
