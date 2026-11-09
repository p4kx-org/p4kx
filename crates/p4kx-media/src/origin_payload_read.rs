// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::path::Path;

use sha2::{Digest as _, Sha256};

use crate::checkpoint::Checkpoint;
use crate::origin_contract::{OriginScope, OriginStream, OriginStreamSummary};
use crate::origin_index_build_read::IndexBuildPayload;
use crate::{Artifact, Index, MediaError, error, hashing};

struct PayloadStream {
    stream: OriginStream,
    input: BufReader<File>,
    occurrences: u64,
    fields: u64,
    declared_records: Option<u64>,
}

/// Independent cursors preserve every actual payload-row occurrence.
pub(crate) struct PayloadStreams {
    streams: Vec<PayloadStream>,
    record_bytes: u64,
    index_build: Option<IndexBuildPayload>,
}

impl PayloadStreams {
    pub(crate) fn open(
        root: &Path,
        index: &Index,
        checkpoint: Checkpoint<'_>,
    ) -> Result<Self, MediaError> {
        let mut streams = Vec::new();
        for kind in &index.kinds {
            let stream = OriginStream::Records {
                kind: kind.provider.kind,
            };
            if streams
                .iter()
                .any(|existing: &PayloadStream| existing.stream == stream)
            {
                return Err(invalid("duplicate payload kind stream"));
            }
            streams.push(open_stream(
                root,
                stream,
                &kind.artifact,
                Some(kind.counts.records.enumerated),
            )?);
        }
        streams.push(open_stream(
            root,
            OriginStream::Sources,
            &index.source_manifest,
            None,
        )?);
        streams.push(open_stream(
            root,
            OriginStream::Artifacts,
            &index.artifact_manifest,
            None,
        )?);
        Ok(Self {
            streams,
            record_bytes: index.limits.record_bytes,
            index_build: IndexBuildPayload::open(index, checkpoint)?,
        })
    }

    pub(crate) fn occurrence(
        &mut self,
        scope: &OriginScope,
        expected_hash: &str,
        checkpoint: Checkpoint<'_>,
    ) -> Result<Vec<u8>, MediaError> {
        checkpoint.check()?;
        if scope.stream == OriginStream::IndexBuild {
            return self
                .index_build
                .as_mut()
                .ok_or_else(|| invalid("Index Build scope is not declared"))?
                .occurrence(scope, expected_hash);
        }
        let record_bytes = self.record_bytes;
        let stream = self.find(scope.stream)?;
        if scope.ordinal.get() != stream.occurrences {
            return Err(invalid("origin occurrence ordinal is not contiguous"));
        }
        let mut bytes = Vec::new();
        stream
            .input
            .by_ref()
            .take(record_bytes.saturating_add(1))
            .read_until(b'\n', &mut bytes)
            .map_err(|cause| error::io(Path::new("media payload stream"), cause))?;
        if bytes.is_empty() || bytes.last() != Some(&b'\n') || bytes.len() as u64 > record_bytes {
            return Err(invalid("origin occurrence has no admitted payload row"));
        }
        verify_hash(&bytes, expected_hash)?;
        stream.occurrences = stream
            .occurrences
            .checked_add(1)
            .ok_or_else(|| invalid("payload occurrence count overflow"))?;
        checkpoint.check()?;
        Ok(bytes)
    }

    pub(crate) fn fields(&mut self, stream: OriginStream, count: u64) -> Result<(), MediaError> {
        if stream == OriginStream::IndexBuild {
            return self
                .index_build
                .as_mut()
                .ok_or_else(|| invalid("Index Build scope is not declared"))?
                .fields(count);
        }
        let stream = self.find(stream)?;
        stream.fields = stream
            .fields
            .checked_add(count)
            .ok_or_else(|| invalid("payload field count overflow"))?;
        Ok(())
    }

    pub(crate) fn finish(
        &mut self,
        summaries: &[OriginStreamSummary],
        checkpoint: Checkpoint<'_>,
    ) -> Result<(), MediaError> {
        if summaries.len() != self.streams.len() + usize::from(self.index_build.is_some()) {
            return Err(invalid(
                "origin summary does not cover every payload stream",
            ));
        }
        for stream in &mut self.streams {
            checkpoint.check()?;
            if !stream
                .input
                .fill_buf()
                .map_err(|cause| error::io(Path::new("media payload stream"), cause))?
                .is_empty()
            {
                return Err(invalid("payload row lacks an origin occurrence"));
            }
            let mut matches = summaries
                .iter()
                .filter(|summary| summary.stream == stream.stream);
            let Some(summary) = matches.next() else {
                return Err(invalid("payload stream lacks its origin summary"));
            };
            if matches.next().is_some()
                || summary.occurrences.get() != stream.occurrences
                || summary.fields.get() != stream.fields
                || stream
                    .declared_records
                    .is_some_and(|count| count != stream.occurrences)
            {
                return Err(invalid(
                    "origin stream summary disagrees with actual payload occurrences",
                ));
            }
        }
        if let Some(index_build) = &self.index_build {
            index_build.finish(summaries)?;
        }
        Ok(())
    }

    fn find(&mut self, stream: OriginStream) -> Result<&mut PayloadStream, MediaError> {
        self.streams
            .iter_mut()
            .find(|existing| existing.stream == stream)
            .ok_or_else(|| invalid("origin scope names an unknown payload stream"))
    }
}

fn open_stream(
    root: &Path,
    stream: OriginStream,
    artifact: &Artifact,
    declared_records: Option<u64>,
) -> Result<PayloadStream, MediaError> {
    let path = root.join(&artifact.path);
    let input = File::open(&path).map_err(|cause| error::io(&path, cause))?;
    Ok(PayloadStream {
        stream,
        input: BufReader::new(input),
        occurrences: 0,
        fields: 0,
        declared_records,
    })
}

fn invalid(message: &str) -> MediaError {
    MediaError::Destination(message.to_owned())
}

pub(crate) fn verify_hash(bytes: &[u8], expected_hash: &str) -> Result<(), MediaError> {
    if expected_hash.len() != 64 || hashing::hex(Sha256::digest(bytes).into()) != expected_hash {
        return Err(invalid("origin occurrence payload hash mismatch"));
    }
    Ok(())
}
