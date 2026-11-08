// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::Write;

use serde::Serialize;

use crate::artifact::BoundedLine;
use crate::checkpoint::Checkpoint;
use crate::finite_serializer::Finite;
use crate::json_output;
use crate::observed_write::ObservedWrite;
use crate::{MediaError, MediaIssue, MediaKind, Source, Status, Subresource};

#[derive(Serialize)]
pub(crate) struct Record<'a, T> {
    pub(crate) kind: MediaKind,
    pub(crate) source: &'a Source,
    pub(crate) subresource: &'a Subresource,
    pub(crate) status: Status,
    pub(crate) data: &'a T,
    pub(crate) issues: &'a [MediaIssue],
}

pub(crate) fn line<T: Serialize>(
    record: &Record<'_, T>,
    limit: u64,
    depth: u32,
    checkpoint: &Checkpoint<'_>,
) -> Result<Vec<u8>, MediaError> {
    if record.status == Status::Failed
        && !record
            .issues
            .iter()
            .any(|issue| issue.severity == crate::Severity::Error)
    {
        return Err(MediaError::Contract(
            "failed record needs an error diagnosis".into(),
        ));
    }
    if record.status == Status::Partial
        && !record
            .issues
            .iter()
            .any(|issue| issue.severity != crate::Severity::Info)
    {
        return Err(MediaError::Contract(
            "partial record needs a projection-loss diagnosis".into(),
        ));
    }
    for issue in record.issues {
        if !namespaced(&issue.code) || issue.location.is_empty() || issue.detail.is_empty() {
            return Err(MediaError::Contract(
                "diagnosis needs a stable namespaced code, location and detail".into(),
            ));
        }
        if record.status == Status::Complete && issue.severity != crate::Severity::Info {
            return Err(MediaError::Contract(
                "complete record contains diagnosed projection loss".into(),
            ));
        }
    }
    let mut line = BoundedLine::new(limit);
    let serialised = record.serialize(Finite(
        &mut serde_json::Serializer::new(ObservedWrite::new(&mut line, checkpoint)),
        depth,
    ));
    json_output::finish(serialised, *checkpoint)?;
    line.write_all(b"\n").map_err(|_| MediaError::Limit {
        budget: "record_bytes",
        limit,
    })?;
    Ok(line.bytes)
}

pub(crate) fn namespaced(value: &str) -> bool {
    value.contains('.')
        && value.split('.').all(|part| {
            part.as_bytes().first().is_some_and(u8::is_ascii_lowercase)
                && part
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
                && !part.ends_with('-')
        })
}
