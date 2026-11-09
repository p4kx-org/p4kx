// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::checkpoint::Checkpoint;
use crate::origin_contract::{OriginScope, OriginStream, OriginStreamSummary};
use crate::{Index, MediaError, build_projection, origin_payload_read};

/// One canonical Build projection with no whole-Index-file hash claim.
pub(crate) struct IndexBuildPayload {
    bytes: Option<Vec<u8>>,
    fields: u64,
}

impl IndexBuildPayload {
    pub(crate) fn open(
        index: &Index,
        checkpoint: Checkpoint<'_>,
    ) -> Result<Option<Self>, MediaError> {
        let declared = index.field_origins.as_ref().is_some_and(|receipts| {
            receipts
                .streams
                .iter()
                .any(|summary| summary.stream == OriginStream::IndexBuild)
        });
        if !declared {
            return Ok(None);
        }
        if !index.source.build_present
            || index
                .source
                .build
                .as_ref()
                .is_some_and(|build| !build.canonical_shape)
        {
            return Err(invalid(
                "declared Index Build scope has missing or unknown actual payload nodes",
            ));
        }
        let bytes = build_projection::bytes(
            index.source.build.as_ref(),
            index.limits.record_bytes,
            checkpoint,
        )?;
        Ok(Some(Self {
            bytes: Some(bytes),
            fields: 0,
        }))
    }

    pub(crate) fn occurrence(
        &mut self,
        scope: &OriginScope,
        expected_hash: &str,
    ) -> Result<Vec<u8>, MediaError> {
        if scope.ordinal.get() != 0 {
            return Err(invalid("Index Build ordinal must be zero"));
        }
        let bytes = self
            .bytes
            .take()
            .ok_or_else(|| invalid("Index Build occurrence is duplicated"))?;
        origin_payload_read::verify_hash(&bytes, expected_hash)?;
        Ok(bytes)
    }

    pub(crate) fn fields(&mut self, count: u64) -> Result<(), MediaError> {
        self.fields = self
            .fields
            .checked_add(count)
            .ok_or_else(|| invalid("Index Build field count overflow"))?;
        Ok(())
    }

    pub(crate) fn finish(&self, summaries: &[OriginStreamSummary]) -> Result<(), MediaError> {
        let mut matches = summaries
            .iter()
            .filter(|summary| summary.stream == OriginStream::IndexBuild);
        let Some(summary) = matches.next() else {
            return Err(invalid("Index Build summary is absent"));
        };
        if self.bytes.is_some()
            || matches.next().is_some()
            || summary.occurrences.get() != 1
            || summary.fields.get() != self.fields
        {
            return Err(invalid(
                "Index Build summary disagrees with its actual projection",
            ));
        }
        Ok(())
    }
}

fn invalid(detail: &str) -> MediaError {
    MediaError::Destination(detail.into())
}
