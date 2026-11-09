// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::origin_contract::OriginReceipts;
use crate::origin_input::MediaOriginInput;
use crate::origin_state::OriginState;
use crate::origin_stream::OriginTransport;

use std::path::PathBuf;

use p4kx_extraction::OwnedDirectoryPermit;

use crate::artifact::Stream;
use crate::checkpoint::Checkpoint;
use crate::contract::Tally;
use crate::{
    Artifact, Capability, Counts, KindSummary, Limits, MediaError, MediaKind, Source, Status,
};

pub(crate) struct KindStream<'a> {
    pub(crate) capability: Capability,
    pub(crate) counts: Counts,
    pub(crate) stream: Stream<'a>,
}

pub(crate) struct Finished {
    pub(crate) kinds: Vec<KindSummary>,
    pub(crate) sources: Artifact,
    pub(crate) artifacts: Artifact,
    pub(crate) origins: OriginReceipts,
}

/// Central bounded record and artifact writer used by every provider.
pub struct MediaWriter<'a> {
    pub(crate) root: PathBuf,
    pub(crate) checkpoint: Checkpoint<'a>,
    pub(crate) namespace: &'a OwnedDirectoryPermit,
    pub(crate) limits: Limits,
    pub(crate) kinds: Vec<KindStream<'a>>,
    pub(crate) manifest: Stream<'a>,
    pub(crate) sources: Stream<'a>,
    pub(crate) active: Option<usize>,
    pub(crate) source: Option<Source>,
    pub(crate) terminal: Option<Status>,
    pub(crate) records: u64,
    pub(crate) output_bytes: u64,
    pub(crate) artifacts: u64,
    pub(crate) diagnostic: bool,
    pub(crate) reference_queries: u64,
    pub(crate) origins: OriginState,
    pub(crate) origin_transport: OriginTransport<'a>,
    pub(crate) source_origin: Option<MediaOriginInput>,
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
        let origin_transport = OriginTransport::create(
            root,
            capabilities.iter().map(|capability| capability.kind),
            checkpoint,
            limits.value_depth,
        )?;
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
            origins: OriginState::new(limits.record_bytes),
            origin_transport,
            source_origin: None,
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

    pub(crate) fn begin(&mut self, active: usize, source: Source) -> Result<(), MediaError> {
        self.checkpoint()?;
        self.active = Some(active);
        self.source = Some(source);
        self.source_origin = None;
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
        self.source_origin = None;
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

    pub(crate) fn require_output_callback(&self) -> Result<(), MediaError> {
        self.checkpoint()?;
        if self.active.is_none() || self.source.is_none() {
            return Err(MediaError::Contract(
                "converted artifact outside provider callback".into(),
            ));
        }
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

    pub(crate) const fn remaining_output_bytes(&self) -> u64 {
        self.limits.output_bytes.saturating_sub(self.output_bytes)
    }

    pub(crate) const fn reserve_output(&mut self, bytes: u64) -> Result<(), MediaError> {
        if bytes > self.remaining_output_bytes() {
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
        let origins = self
            .origin_transport
            .finish(&self.root, &self.origins.owner)?;
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
            origins,
        })
    }
}

#[cfg(test)]
#[path = "artifact_accounting_tests.rs"]
mod artifact_tests;

pub(crate) const fn observe(tally: &mut Tally, status: Status) {
    match status {
        Status::Complete => tally.written += 1,
        Status::Partial => tally.partial += 1,
        Status::Failed => tally.failed += 1,
    }
}

pub(crate) const fn worst(left: Status, right: Status) -> Status {
    match (left, right) {
        (Status::Failed, _) | (_, Status::Failed) => Status::Failed,
        (Status::Partial, _) | (_, Status::Partial) => Status::Partial,
        _ => Status::Complete,
    }
}
