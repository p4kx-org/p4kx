// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_extraction::SourceUseRole;
use p4kx_extraction::model::PhysicsFloat;

use crate::origin_contributions::OriginContributions;
use crate::origin_prepared::PreparedRecordOrigins;
use crate::{MediaError, MediaWriter, OffsetBasis, SourceRange};

use super::super::origins::{self, ModelOrigins};

pub(super) struct PhysicsOrigins<'origin, 'input> {
    pub(super) model: &'origin ModelOrigins<'input>,
    pub(super) base: u64,
    pub(super) extent: &'origin OriginContributions,
}

impl PhysicsOrigins<'_, '_> {
    pub(super) const fn range(&self, local: Range<u64>) -> SourceRange {
        let global = self.global(local);
        SourceRange::new(OffsetBasis::Entry, global.start, global.end)
    }

    pub(super) const fn global(&self, local: Range<u64>) -> Range<u64> {
        self.base + local.start..self.base + local.end
    }

    pub(super) fn use_bytes(
        &self,
        local: Range<u64>,
        word: &str,
        writer: &MediaWriter<'_>,
    ) -> Result<OriginContributions, MediaError> {
        self.model
            .payload(self.global(local), word, SourceUseRole::Payload, writer)
    }

    pub(super) fn bind_range(
        &self,
        field: &str,
        start: OriginContributions,
        end: OriginContributions,
        prepared: &mut PreparedRecordOrigins,
        writer: &MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        let union = merge(writer, [&start, &end])?;
        bind(prepared, field, union, writer)?;
        bind(prepared, &format!("{field}/start"), start, writer)?;
        bind(prepared, &format!("{field}/end_exclusive"), end, writer)?;
        bind(
            prepared,
            &format!("{field}/basis"),
            self.model.independent(),
            writer,
        )
    }

    pub(super) fn vector(
        &self,
        field: &str,
        values: [PhysicsFloat; 3],
        prepared: &mut PreparedRecordOrigins,
        writer: &MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        let range = values[0].range().start..values[2].range().end;
        bind(
            prepared,
            field,
            self.use_bytes(range, "physicsFloatVector", writer)?,
            writer,
        )?;
        for (index, value) in values.into_iter().enumerate() {
            self.scalar(&format!("{field}/{index}"), value, prepared, writer)?;
        }
        Ok(())
    }

    pub(super) fn scalar(
        &self,
        field: &str,
        value: PhysicsFloat,
        prepared: &mut PreparedRecordOrigins,
        writer: &MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        bind(
            prepared,
            field,
            self.use_bytes(value.range(), "physicsFloatWord", writer)?,
            writer,
        )
    }
}

pub(super) fn bind(
    prepared: &mut PreparedRecordOrigins,
    field: &str,
    evidence: OriginContributions,
    writer: &MediaWriter<'_>,
) -> Result<(), MediaError> {
    origins::at(
        prepared,
        &format!("/data/payload/{field}"),
        evidence,
        writer,
    )
}

pub(super) fn merge<'a>(
    writer: &MediaWriter<'_>,
    inputs: impl IntoIterator<Item = &'a OriginContributions>,
) -> Result<OriginContributions, MediaError> {
    let mut capture = writer.capture_origins();
    for input in inputs {
        capture.include_original(input)?;
    }
    Ok(capture.finish())
}
