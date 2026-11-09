// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::Write;

use serde::Serialize;

use crate::MediaWriter;
use crate::media_record::{self, Record};
use crate::media_writer;
use crate::origin_prepared::PreparedRecordOrigins;
use crate::origin_resolver::BuiltinOrigins;
use crate::{MediaError, MediaIssue, Status, Subresource, error, output_operation};

impl MediaWriter<'_> {
    /// Writes one source-local record; source identity and kind are supplied centrally.
    ///
    /// Providers use `Float32`/`Float64` for every source float and preserve original ordinals.
    ///
    /// # Errors
    /// Returns budget, serialisation, cancellation or file failures.
    pub fn record<T: Serialize>(
        &mut self,
        subresource: &Subresource,
        status: Status,
        data: &T,
        issues: &[MediaIssue],
    ) -> Result<(), MediaError> {
        self.write_record(subresource, status, data, issues, None, None)
    }

    pub(crate) fn write_record<T: Serialize>(
        &mut self,
        subresource: &Subresource,
        status: Status,
        data: &T,
        issues: &[MediaIssue],
        builtin: Option<&BuiltinOrigins>,
        prepared: Option<&PreparedRecordOrigins>,
    ) -> Result<(), MediaError> {
        let active = self.admit_record(subresource, status, issues)?;
        let line = self.record_line(active, subresource, status, data, issues)?;
        self.reserve_output(line.len() as u64)?;
        self.emit_record_origin(active, subresource, &line, builtin, prepared)?;
        if let Some(prepared) = prepared {
            prepared.require_consumed()?;
        }
        let stream = self
            .kinds
            .get_mut(active)
            .ok_or_else(|| MediaError::Contract("unknown active provider".into()))?;
        let result = (|| {
            stream
                .stream
                .write_all(&line)
                .map_err(|source| error::io(&self.root, source))?;
            stream.counts.records.enumerated += 1;
            media_writer::observe(&mut stream.counts.records, status);
            self.records += 1;
            self.terminal = Some(media_writer::worst(
                self.terminal.unwrap_or(Status::Complete),
                status,
            ));
            Ok(())
        })();
        output_operation::checked(result, self.checkpoint())
    }

    fn admit_record(
        &self,
        subresource: &Subresource,
        status: Status,
        issues: &[MediaIssue],
    ) -> Result<usize, MediaError> {
        self.checkpoint()?;
        if self.records >= self.limits.records {
            return Err(MediaError::Limit {
                budget: "records",
                limit: self.limits.records,
            });
        }
        if subresource.namespace.is_empty() || (status != Status::Complete && issues.is_empty()) {
            return Err(MediaError::Contract(
                "record needs a namespace and diagnosed non-complete status".into(),
            ));
        }
        self.active
            .ok_or_else(|| MediaError::Contract("record outside provider callback".into()))
    }

    fn record_line<T: Serialize>(
        &self,
        active: usize,
        subresource: &Subresource,
        status: Status,
        data: &T,
        issues: &[MediaIssue],
    ) -> Result<Vec<u8>, MediaError> {
        let kind = self
            .kinds
            .get(active)
            .ok_or_else(|| MediaError::Contract("unknown active provider".into()))?
            .capability
            .kind;
        let source = self
            .source
            .as_ref()
            .ok_or_else(|| MediaError::Contract("record without source".into()))?;
        if status != Status::Complete && !self.diagnostic {
            return Err(MediaError::ProjectionLoss {
                kind,
                issues: issues.to_vec(),
            });
        }
        media_record::line(
            &Record {
                kind,
                source,
                subresource,
                status,
                data,
                issues,
            },
            self.limits.record_bytes,
            self.limits.value_depth,
            &self.checkpoint,
        )
    }
}
