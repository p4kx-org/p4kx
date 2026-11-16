// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;
use std::collections::{HashSet, VecDeque};
use std::io;

use p4kx_extraction::FieldOrigin;
use sha2::{Digest as _, Sha256};

use crate::artifact::StreamObserver;
use crate::origin_contract::use_sets::SetId;
use crate::{Algorithm, Artifact, ByteScope, MediaKind, MediaRecord, Source, Tally, hashing};

use super::origin_batches::OriginBatches;

use super::rows::Evidence;
use super::{TestResult, require};

#[derive(Default)]
pub(super) struct Observation {
    entry: RefCell<EntryEvidence>,
    seen: RefCell<HashSet<[u8; 32]>>,
    pub(super) origins: OriginBatches,
}

impl Observation {
    pub(super) fn take_entry(&self) -> EntryEvidence {
        self.entry.take()
    }

    pub(super) fn finish_census(&self, converted: &Tally) -> TestResult {
        self.entry.borrow().finish_entry()?;
        let distinct = u64::try_from(self.seen.borrow().len())?;
        require(
            converted.written == distinct
                && converted.enumerated.checked_sub(distinct) == Some(converted.skipped),
            "converted animation tally agrees with exact typed artifact deduplication",
        )
    }
}

impl StreamObserver for Observation {
    fn write(&self, relative: &str, bytes: &[u8]) -> io::Result<()> {
        let mut entry = self.entry.try_borrow_mut().map_err(io::Error::other)?;
        match relative {
            "sources.jsonl" => {
                if entry.source.is_some() {
                    return Err(io::Error::other("animation source receipt already held"));
                }
                entry.source = Some(serde_json::from_slice(bytes)?);
            }
            "artifacts.jsonl" => {
                entry.artifacts.try_reserve(1).map_err(io::Error::other)?;
                entry.artifacts.push_back(serde_json::from_slice(bytes)?);
            }
            "animation.jsonl" => {
                entry.records.try_reserve(1).map_err(io::Error::other)?;
                entry.records.push_back(serde_json::from_slice(bytes)?);
            }
            _ => {}
        }
        Ok(())
    }

    fn origin_field(&self, origin: &FieldOrigin, shared: Option<SetId>) -> io::Result<()> {
        self.origins.field(origin, shared)
    }

    fn typed_chunk(&self, artifact: &Artifact) -> io::Result<bool> {
        let mut entry = self.entry.try_borrow_mut().map_err(io::Error::other)?;
        entry.chunks.try_reserve(1).map_err(io::Error::other)?;
        entry.chunks.push_back(artifact.clone());
        let packed = packed_digest(artifact.digest.hex.as_bytes())?;
        let mut seen = self.seen.try_borrow_mut().map_err(io::Error::other)?;
        seen.try_reserve(1).map_err(io::Error::other)?;
        Ok(seen.insert(packed))
    }
}

#[derive(Default)]
pub(super) struct EntryEvidence {
    source: Option<Source>,
    artifacts: VecDeque<Artifact>,
    chunks: VecDeque<Artifact>,
    records: VecDeque<MediaRecord>,
}

impl EntryEvidence {
    pub(super) fn raw(&mut self, expected: &Source) -> TestResult<Source> {
        let source = self
            .source
            .take()
            .ok_or("one physical animation source receipt")?;
        require(
            serde_json::to_value(&source)? == serde_json::to_value(expected)?,
            "animation source receipt differs from verified Archive metadata",
        )?;
        let raw = source
            .raw
            .as_ref()
            .ok_or("verified raw animation receipt")?;
        let artifact = self
            .artifacts
            .pop_front()
            .ok_or("raw animation artifact receipt")?;
        require(
            artifact.path == raw.path && artifact.size == raw.size && artifact.digest == raw.digest,
            "raw animation receipt has the verified payload size and SHA-256",
        )?;
        Ok(source)
    }

    pub(super) fn finish_entry(&self) -> TestResult {
        require(
            self.source.is_none()
                && self.artifacts.is_empty()
                && self.chunks.is_empty()
                && self.records.is_empty(),
            "unmatched animation source, record or artifact receipts",
        )
    }
}

impl Evidence for EntryEvidence {
    fn record(&mut self) -> TestResult<Option<MediaRecord>> {
        let record = self.records.pop_front();
        require(
            record
                .as_ref()
                .is_none_or(|record| record.kind == MediaKind::Animation),
            "record belongs to the original animation provider",
        )?;
        Ok(record)
    }

    fn sample(&mut self, artifact: &Artifact, expected: &[u8]) -> TestResult {
        let chunk = self
            .chunks
            .pop_front()
            .ok_or("actual typed animation bytes")?;
        let manifested = self
            .artifacts
            .pop_front()
            .ok_or("typed animation manifest receipt")?;
        require(
            same_artifact(artifact, &chunk) && same_artifact(artifact, &manifested),
            "animation sample references the actual typed chunk and manifest receipt",
        )?;
        require(
            artifact.size == u64::try_from(expected.len())?
                && artifact.digest.algorithm == Algorithm::Sha256
                && artifact.digest.scope == ByteScope::ArtifactFile
                && artifact.digest.hex == hashing::hex(Sha256::digest(expected).into()),
            "typed animation size and SHA-256 agree with the parser's exact decoded bytes",
        )
    }
}

fn same_artifact(left: &Artifact, right: &Artifact) -> bool {
    left.size == right.size && left.path == right.path && left.digest == right.digest
}

fn packed_digest(text: &[u8]) -> io::Result<[u8; 32]> {
    let (pairs, rest) = text.as_chunks::<2>();
    let mut digest = [0; 32];
    if pairs.len() != digest.len() || !rest.is_empty() {
        return Err(io::Error::other("incomplete typed chunk SHA-256"));
    }
    for (target, digits) in digest.iter_mut().zip(pairs) {
        let digits = std::str::from_utf8(digits).map_err(io::Error::other)?;
        *target = u8::from_str_radix(digits, 16).map_err(io::Error::other)?;
    }
    Ok(digest)
}
