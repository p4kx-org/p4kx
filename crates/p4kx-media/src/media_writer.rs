// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::Write;
use std::path::PathBuf;

use p4kx_extraction::OwnedDirectoryPermit;

use serde::Serialize;

use crate::artifact::{BoundedLine, Stream};
use crate::checkpoint::Checkpoint;
use crate::contract::Tally;
use crate::error;
use crate::media_record;
use crate::media_record::Record;
use crate::{
    Artifact, Capability, Counts, KindSummary, Limits, MediaError, MediaIssue, MediaKind, Source,
    Status, Subresource,
};

struct KindStream<'a> {
    capability: Capability,
    counts: Counts,
    stream: Stream<'a>,
}

pub(crate) struct Finished {
    pub(crate) kinds: Vec<KindSummary>,
    pub(crate) sources: Artifact,
    pub(crate) artifacts: Artifact,
}

/// Central bounded record and artifact writer used by every provider.
pub struct MediaWriter<'a> {
    pub(crate) root: PathBuf,
    pub(crate) checkpoint: Checkpoint<'a>,
    pub(crate) namespace: &'a OwnedDirectoryPermit,
    pub(crate) limits: Limits,
    kinds: Vec<KindStream<'a>>,
    manifest: Stream<'a>,
    sources: Stream<'a>,
    active: Option<usize>,
    source: Option<Source>,
    terminal: Option<Status>,
    records: u64,
    output_bytes: u64,
    artifacts: u64,
    diagnostic: bool,
    reference_queries: u64,
}

impl<'a> MediaWriter<'a> {
    pub(crate) fn new(
        namespace: &'a OwnedDirectoryPermit,
        capabilities: Vec<Capability>,
        limits: Limits,
        checkpoint: Checkpoint<'a>,
        diagnostic: bool,
    ) -> Result<Self, MediaError> {
        let root = namespace.path();
        let mut kinds = Vec::new();
        for capability in capabilities {
            let stream = Stream::create(root, format!("{}.jsonl", capability.kind), &checkpoint)?;
            kinds.push(KindStream {
                capability,
                counts: Counts::default(),
                stream,
            });
        }
        Ok(Self {
            root: root.into(),
            checkpoint,
            namespace,
            limits,
            kinds,
            manifest: Stream::create(root, "artifacts.jsonl".into(), &checkpoint)?,
            sources: Stream::create(root, "sources.jsonl".into(), &checkpoint)?,
            active: None,
            source: None,
            terminal: None,
            records: 0,
            output_bytes: 0,
            artifacts: 0,
            diagnostic,
            reference_queries: 0,
        })
    }

    /// Checks cancellation and Archive changes before provider work.
    ///
    /// # Errors
    /// Returns cancellation or source-change failure.
    pub fn checkpoint(&self) -> Result<(), MediaError> {
        self.checkpoint.check()
    }

    /// Returns the limits that provider-specific bounded readers must respect.
    #[must_use]
    pub const fn limits(&self) -> Limits {
        self.limits
    }

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
        let active = self
            .active
            .ok_or_else(|| MediaError::Contract("record outside provider callback".into()))?;
        let line = self.record_line(active, subresource, status, data, issues)?;
        self.reserve_output(line.len() as u64)?;
        let stream = self
            .kinds
            .get_mut(active)
            .ok_or_else(|| MediaError::Contract("unknown active provider".into()))?;
        stream
            .stream
            .write_all(&line)
            .map_err(|source| error::io(&self.root, source))?;
        stream.counts.records.enumerated += 1;
        observe(&mut stream.counts.records, status);
        self.records += 1;
        self.terminal = Some(worst(self.terminal.unwrap_or(Status::Complete), status));
        Ok(())
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

    pub(crate) fn begin(&mut self, active: usize, source: Source) -> Result<(), MediaError> {
        self.checkpoint()?;
        self.active = Some(active);
        self.source = Some(source);
        self.terminal = None;
        Ok(())
    }

    pub(crate) fn start_reference(&mut self) -> Result<(), MediaError> {
        self.checkpoint()?;
        if self.source.is_none() {
            return Err(MediaError::Contract(
                "reference outside provider callback".into(),
            ));
        }
        if self.reference_queries >= self.limits.references {
            return Err(MediaError::Limit {
                budget: "references",
                limit: self.limits.references,
            });
        }
        self.reference_queries += 1;
        Ok(())
    }

    pub(crate) fn end_directory(&mut self) -> Result<(), MediaError> {
        self.active = None;
        self.source = None;
        self.terminal = None;
        self.checkpoint()
    }

    pub(crate) fn has_kind(&self, kind: MediaKind) -> bool {
        self.kinds
            .iter()
            .any(|stream| stream.capability.kind == kind)
    }

    pub(crate) fn require_source(&self, expected: &Source) -> Result<(), MediaError> {
        self.checkpoint()?;
        if self.active.is_none()
            || !self.source.as_ref().is_some_and(|source| {
                source.root == expected.root && source.identity == expected.identity
            })
        {
            return Err(MediaError::Contract(
                "required input outside its active source callback".into(),
            ));
        }
        Ok(())
    }

    pub(crate) fn reference_root(&self) -> Result<&crate::Digest, MediaError> {
        self.source
            .as_ref()
            .map(|source| &source.root)
            .ok_or_else(|| MediaError::Contract("reference without source provenance".into()))
    }

    pub(crate) fn end_source(&mut self, projected: crate::Projection) -> Result<(), MediaError> {
        let active = self
            .active
            .ok_or_else(|| MediaError::Contract("missing active provider".into()))?;
        let stream = self
            .kinds
            .get_mut(active)
            .ok_or_else(|| MediaError::Contract("unknown active provider".into()))?;
        stream.counts.source_entries.enumerated += 1;
        match (projected, self.terminal) {
            (crate::Projection::Written, Some(status)) => {
                observe(&mut stream.counts.source_entries, status);
            }
            (crate::Projection::Skipped, None) => stream.counts.source_entries.skipped += 1,
            _ => {
                return Err(MediaError::Contract(
                    "provider result contradicts its actual written records".into(),
                ));
            }
        }
        self.checkpoint()
    }

    pub(crate) fn artifact(&mut self, artifact: &Artifact) -> Result<(), MediaError> {
        self.checkpoint()?;
        if self.artifacts >= self.limits.sources.saturating_add(self.limits.records) {
            return Err(MediaError::Limit {
                budget: "artifacts",
                limit: self.limits.sources.saturating_add(self.limits.records),
            });
        }
        let mut line = BoundedLine::new(self.limits.record_bytes);
        serde_json::to_writer(&mut line, artifact)?;
        line.write_all(b"\n")
            .map_err(|source| error::io(&self.root, source))?;
        self.reserve_output(line.bytes.len() as u64)?;
        self.manifest
            .write_all(&line.bytes)
            .map_err(|source| error::io(&self.root, source))?;
        self.artifacts += 1;
        Ok(())
    }

    pub(crate) fn retained(&mut self, source: &Source) -> Result<(), MediaError> {
        self.checkpoint()?;
        let mut line = BoundedLine::new(self.limits.record_bytes);
        let serialised = serde_json::to_writer(
            crate::observed_write::ObservedWrite::new(&mut line, &self.checkpoint),
            source,
        );
        self.checkpoint()?;
        serialised?;
        line.write_all(b"\n").map_err(|_| MediaError::Limit {
            budget: "record_bytes",
            limit: self.limits.record_bytes,
        })?;
        self.reserve_output(line.bytes.len() as u64)?;
        self.sources
            .write_all(&line.bytes)
            .map_err(|cause| error::io(&self.root, cause))?;
        Ok(())
    }

    pub(crate) fn converted_output(&mut self, created: bool) -> Result<(), MediaError> {
        let active = self
            .active
            .ok_or_else(|| MediaError::Contract("chunk outside provider callback".into()))?;
        let counts = &mut self
            .kinds
            .get_mut(active)
            .ok_or_else(|| MediaError::Contract("unknown active provider".into()))?
            .counts
            .converted_outputs;
        counts.enumerated += 1;
        if created {
            counts.written += 1;
        } else {
            counts.skipped += 1;
        }
        Ok(())
    }

    pub(crate) const fn reserve_output(&mut self, bytes: u64) -> Result<(), MediaError> {
        if bytes > self.limits.output_bytes.saturating_sub(self.output_bytes) {
            return Err(MediaError::Limit {
                budget: "output_bytes",
                limit: self.limits.output_bytes,
            });
        }
        self.output_bytes += bytes;
        Ok(())
    }

    pub(crate) fn finish(self) -> Result<Finished, MediaError> {
        self.checkpoint.finish()?;
        let mut kinds = Vec::new();
        for stream in self.kinds {
            kinds.push(KindSummary {
                provider: stream.capability,
                counts: stream.counts,
                artifact: stream.stream.finish()?,
            });
        }
        Ok(Finished {
            kinds,
            sources: self.sources.finish()?,
            artifacts: self.manifest.finish()?,
        })
    }
}

pub(crate) const fn observe(tally: &mut Tally, status: Status) {
    match status {
        Status::Complete => tally.written += 1,
        Status::Partial => tally.partial += 1,
        Status::Failed => tally.failed += 1,
    }
}

const fn worst(left: Status, right: Status) -> Status {
    match (left, right) {
        (Status::Failed, _) | (_, Status::Failed) => Status::Failed,
        (Status::Partial, _) | (_, Status::Partial) => Status::Partial,
        _ => Status::Complete,
    }
}
