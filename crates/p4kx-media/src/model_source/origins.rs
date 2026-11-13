// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_extraction::{FieldPath, InputScope, SourceUseRole};

use crate::origin_capture::OriginSite;
use crate::origin_contract::OriginProfile;
use crate::origin_contributions::OriginContributions;
use crate::origin_input::MediaOriginInput;
use crate::origin_prepared::PreparedRecordOrigins;
use crate::origin_resolver::BuiltinOrigins;
use crate::{MediaError, MediaWriter, origin_error};

pub(super) struct ModelOrigins<'a> {
    pub(super) input: &'a MediaOriginInput,
    pub(super) entry: usize,
}

impl ModelOrigins<'_> {
    pub(super) fn prepared(&self) -> Result<PreparedRecordOrigins, MediaError> {
        let mut prepared = PreparedRecordOrigins::new(&self.input.owner);
        prepared.set_builtin(BuiltinOrigins {
            profile: OriginProfile::Model,
            input: self.input.clone(),
            scope: InputScope::Payload,
        })?;
        Ok(prepared)
    }

    pub(super) fn payload(
        &self,
        range: Range<u64>,
        field: &str,
        role: SourceUseRole,
        writer: &MediaWriter<'_>,
    ) -> Result<OriginContributions, MediaError> {
        let mut capture = writer.capture_origins();
        capture.add(
            self.input,
            InputScope::Payload,
            OriginSite::new("model-byte-range", &[range.start, range.end], field),
            role,
            writer,
        )?;
        Ok(capture.finish())
    }

    pub(super) fn directory(
        &self,
        field: &str,
        writer: &MediaWriter<'_>,
    ) -> Result<OriginContributions, MediaError> {
        let mut capture = writer.capture_origins();
        capture.add(
            self.input,
            InputScope::ArchiveDirectoryMetadata,
            OriginSite::new("archive-entry", &[self.entry as u64], field),
            SourceUseRole::Selector,
            writer,
        )?;
        Ok(capture.finish())
    }

    pub(super) fn independent(&self) -> OriginContributions {
        OriginContributions::independent(&self.input.owner)
    }
}

pub(super) fn at(
    prepared: &mut PreparedRecordOrigins,
    path: &str,
    evidence: OriginContributions,
    writer: &MediaWriter<'_>,
) -> Result<(), MediaError> {
    let path = FieldPath::parse(path).map_err(|cause| origin_error::address(&cause))?;
    prepared.insert_subtree(&path, evidence, writer)
}

pub(super) struct ChunkExtent {
    pub(super) start: OriginContributions,
    pub(super) end: OriginContributions,
}
pub(super) fn chunk_extent(
    table: Range<u64>,
    offset: Range<u64>,
    size: Option<Range<u64>>,
    origins: &ModelOrigins<'_>,
    writer: &MediaWriter<'_>,
) -> Result<ChunkExtent, MediaError> {
    let start = origins.payload(offset, "modelChunkOffset", SourceUseRole::Payload, writer)?;
    let mut end = writer.capture_origins();
    end.include_original(&start)?;
    if let Some(size) = size {
        end.include_original(&origins.payload(
            size,
            "modelChunkSize",
            SourceUseRole::Payload,
            writer,
        )?)?;
    } else {
        for (range, label) in [
            (8..16, "modelChunkTableAddress"),
            (table, "modelChunkExtentTable"),
        ] {
            end.include_original(&origins.payload(
                range,
                label,
                SourceUseRole::Selector,
                writer,
            )?)?;
        }
        end.include_original(&origins.directory("modelDecodedEntryLength", writer)?)?;
    }
    Ok(ChunkExtent {
        start,
        end: end.finish(),
    })
}
