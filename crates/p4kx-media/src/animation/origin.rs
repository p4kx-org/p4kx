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

pub(crate) struct AnimationOrigins<'a> {
    pub(crate) input: &'a MediaOriginInput,
    pub(crate) entry: usize,
}

impl AnimationOrigins<'_> {
    pub(crate) fn prepared(&self) -> Result<PreparedRecordOrigins, MediaError> {
        let mut prepared = PreparedRecordOrigins::new(&self.input.owner);
        prepared.set_builtin(BuiltinOrigins {
            profile: OriginProfile::Animation,
            input: self.input.clone(),
            scope: InputScope::Payload,
        })?;
        Ok(prepared)
    }

    pub(crate) fn payload(
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
            OriginSite::new("animation-byte-range", &[range.start, range.end], field),
            role,
            writer,
        )?;
        Ok(capture.finish())
    }

    pub(crate) fn extension(
        &self,
        writer: &MediaWriter<'_>,
    ) -> Result<OriginContributions, MediaError> {
        let mut capture = writer.capture_origins();
        capture.add(
            self.input,
            InputScope::ArchiveDirectoryMetadata,
            OriginSite::new("archive-entry", &[self.entry as u64], "animationExtension"),
            SourceUseRole::Selector,
            writer,
        )?;
        Ok(capture.finish())
    }

    pub(crate) fn decoded(
        &self,
        table: Range<u64>,
        range: Range<u64>,
        field: &str,
        writer: &MediaWriter<'_>,
    ) -> Result<OriginContributions, MediaError> {
        let mut capture = writer.capture_origins();
        for (range, label, role) in [
            (table, "channelDescriptor", SourceUseRole::Selector),
            (range, field, SourceUseRole::Conversion),
        ] {
            capture.add(
                self.input,
                InputScope::Payload,
                OriginSite::new("animation-byte-range", &[range.start, range.end], label),
                role,
                writer,
            )?;
        }
        Ok(capture.finish())
    }

    pub(crate) fn independent(&self) -> OriginContributions {
        OriginContributions::independent(&self.input.owner)
    }

    pub(crate) fn unknown(&self) -> OriginContributions {
        OriginContributions::unknown(&self.input.owner)
    }
}

pub(crate) fn at(
    prepared: &mut PreparedRecordOrigins,
    path: &str,
    evidence: OriginContributions,
    writer: &MediaWriter<'_>,
) -> Result<(), MediaError> {
    let path = FieldPath::parse(path).map_err(|cause| origin_error::address(&cause))?;
    prepared.insert_subtree(&path, evidence, writer)
}

pub(crate) fn control(
    prepared: &mut PreparedRecordOrigins,
    evidence: OriginContributions,
    writer: &MediaWriter<'_>,
) -> Result<(), MediaError> {
    at(prepared, "/subresource/indices", evidence, writer)
}
