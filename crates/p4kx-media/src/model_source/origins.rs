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
