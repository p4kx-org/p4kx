// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs::File;
use std::io::{self, Read, Write};
use std::path::Path;

use p4kx_extraction::DecimalU64;
use serde::Serialize;

use crate::checkpoint::Checkpoint;
use crate::observed_write::ObservedWrite;
use crate::origin_contract::{OriginProtocol, OriginReceipts, OriginStream, OriginStreamSummary};
use crate::{
    Algorithm, Artifact, ByteScope, Digest, Index, MediaError, MediaKind, error, json_output,
};

const SOURCES_PATH: &str = "origin-sources.jsonl";
const USES_PATH: &str = "origin-uses.jsonl";
const FIELDS_PATH: &str = "field-origins.jsonl";
const SHA256_HEX_BYTES: usize = 64;
#[derive(Clone, Copy)]
pub(crate) enum Encoding {
    Compact,
    Pretty,
}

#[derive(Clone, Copy)]
pub(crate) struct Budget {
    base: u64,
    control: u64,
    bound: u64,
}

impl Budget {
    pub(crate) fn for_index(index: &Index, encoding: Encoding) -> Result<Self, MediaError> {
        Self::new(
            index.limits.record_bytes,
            index.field_origins.as_ref(),
            encoding,
        )
    }

    fn new(
        base: u64,
        receipts: Option<&OriginReceipts>,
        encoding: Encoding,
    ) -> Result<Self, MediaError> {
        if let Some(receipts) = receipts {
            validate_receipts(receipts)?;
        }
        let control = insertion(receipts, encoding)?;
        Ok(Self {
            base,
            control,
            bound: base.saturating_add(control),
        })
    }

    pub(crate) fn admit(self, written: u64, bytes: usize) -> Result<u64, MediaError> {
        let next = add_bytes(written, bytes)?;
        if next > self.bound {
            return Err(limit(self.base));
        }
        Ok(next)
    }

    pub(crate) fn finish(self, written: u64) -> Result<(), MediaError> {
        let base = written
            .checked_sub(self.control)
            .ok_or_else(|| invalid("index is shorter than its actual control insertion"))?;
        if written > self.bound || base > self.base {
            return Err(limit(self.base));
        }
        Ok(())
    }
}

pub(crate) struct Buffer {
    pub(crate) bytes: Vec<u8>,
    budget: Budget,
}

impl Buffer {
    pub(crate) const fn new(budget: Budget) -> Self {
        Self {
            bytes: Vec::new(),
            budget,
        }
    }

    pub(crate) fn finish(&self) -> Result<(), MediaError> {
        self.budget.finish(byte_len(self.bytes.len())?)
    }
}

impl Write for Buffer {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.budget
            .admit(
                byte_len(self.bytes.len()).map_err(io::Error::other)?,
                bytes.len(),
            )
            .map_err(io::Error::other)?;
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub(crate) fn read(
    path: &Path,
    base: u64,
    checkpoint: &Checkpoint<'_>,
) -> Result<Index, MediaError> {
    checkpoint.check()?;
    let bound = read_bound(base)?;
    let input = File::open(path).map_err(|cause| error::io(path, cause))?;
    let mut bytes = Vec::new();
    input
        .take(bound.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|cause| error::io(path, cause))?;
    let actual = byte_len(bytes.len())?;
    if actual > bound {
        return Err(limit(base));
    }
    let index: Index = serde_json::from_slice(&bytes)?;
    validate_read(&index, actual, base, Some(checkpoint))?;
    checkpoint.check()?;
    Ok(index)
}

fn validate_read(
    index: &Index,
    actual: u64,
    base: u64,
    checkpoint: Option<&Checkpoint<'_>>,
) -> Result<(), MediaError> {
    let Some(receipts) = index.field_origins.as_ref() else {
        return if actual <= base {
            Ok(())
        } else {
            Err(limit(base))
        };
    };
    let budget = Budget::new(base, Some(receipts), Encoding::Compact)?;
    budget.finish(measure(index, Encoding::Compact, checkpoint)?)
}

fn read_bound(base: u64) -> Result<u64, MediaError> {
    let receipts = maximum_receipts();
    let compact = insertion(Some(&receipts), Encoding::Compact)?;
    let pretty = insertion(Some(&receipts), Encoding::Pretty)?;
    Ok(base.saturating_add(compact.max(pretty)))
}

#[derive(Serialize)]
struct Envelope<'a> {
    existing: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    field_origins: Option<&'a OriginReceipts>,
}

fn insertion(receipts: Option<&OriginReceipts>, encoding: Encoding) -> Result<u64, MediaError> {
    let old = measure(
        &Envelope {
            existing: true,
            field_origins: None,
        },
        encoding,
        None,
    )?;
    let full = measure(
        &Envelope {
            existing: true,
            field_origins: receipts,
        },
        encoding,
        None,
    )?;
    full.checked_sub(old)
        .ok_or_else(|| invalid("origin control insertion underflow"))
}

fn measure<T: Serialize>(
    value: &T,
    encoding: Encoding,
    checkpoint: Option<&Checkpoint<'_>>,
) -> Result<u64, MediaError> {
    let mut count = Counter(0);
    let result = match checkpoint {
        Some(checkpoint) => encode(value, encoding, ObservedWrite::new(&mut count, checkpoint)),
        None => encode(value, encoding, &mut count),
    };
    if let Some(checkpoint) = checkpoint {
        json_output::finish(result, *checkpoint)?;
    } else {
        result?;
    }
    Ok(count.0)
}

fn encode<T: Serialize, W: Write>(
    value: &T,
    encoding: Encoding,
    writer: W,
) -> Result<(), serde_json::Error> {
    match encoding {
        Encoding::Compact => serde_json::to_writer(writer, value),
        Encoding::Pretty => serde_json::to_writer_pretty(writer, value),
    }
}

struct Counter(u64);

impl Write for Counter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0 = add_bytes(self.0, bytes.len()).map_err(io::Error::other)?;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn add_bytes(written: u64, bytes: usize) -> Result<u64, MediaError> {
    written
        .checked_add(byte_len(bytes)?)
        .ok_or_else(|| invalid("index byte counter overflow"))
}

fn byte_len(bytes: usize) -> Result<u64, MediaError> {
    u64::try_from(bytes).map_err(|_| invalid("index byte count is not representable"))
}

pub(crate) fn validate_receipts(receipts: &OriginReceipts) -> Result<(), MediaError> {
    if receipts.protocol != OriginProtocol::V1 {
        return Err(invalid("unsupported origin protocol"));
    }
    for (artifact, path) in [
        (&receipts.sources, SOURCES_PATH),
        (&receipts.uses, USES_PATH),
        (&receipts.fields, FIELDS_PATH),
    ] {
        if artifact.path != path
            || artifact.digest.algorithm != Algorithm::Sha256
            || artifact.digest.scope != ByteScope::ArtifactFile
            || artifact.digest.hex.len() != SHA256_HEX_BYTES
            || !artifact
                .digest
                .hex
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
        {
            return Err(invalid(
                "origin receipt violates its fixed artifact contract",
            ));
        }
    }
    if receipts.streams.len() > streams().count() {
        return Err(invalid("too many origin stream summaries"));
    }
    let mut seen = Vec::new();
    for summary in &receipts.streams {
        if seen.contains(&summary.stream) || !streams().any(|stream| stream == summary.stream) {
            return Err(invalid(
                "origin stream summaries are duplicated or unsupported",
            ));
        }
        seen.push(summary.stream);
    }
    if !seen.contains(&OriginStream::Sources) || !seen.contains(&OriginStream::Artifacts) {
        return Err(invalid("origin stream summaries omit a mandatory stream"));
    }
    Ok(())
}

fn streams() -> impl Iterator<Item = OriginStream> {
    [
        MediaKind::Archive,
        MediaKind::Model,
        MediaKind::Material,
        MediaKind::Texture,
        MediaKind::Shader,
        MediaKind::Animation,
        MediaKind::AnimationDefinition,
        MediaKind::Audio,
        MediaKind::AudioControl,
    ]
    .into_iter()
    .map(|kind| OriginStream::Records { kind })
    .chain([OriginStream::Sources, OriginStream::Artifacts])
}

fn maximum_receipts() -> OriginReceipts {
    let artifact = |path: &str| Artifact {
        path: path.into(),
        size: u64::MAX,
        digest: Digest {
            algorithm: Algorithm::Sha256,
            scope: ByteScope::ArtifactFile,
            hex: "f".repeat(SHA256_HEX_BYTES),
        },
    };
    OriginReceipts {
        protocol: OriginProtocol::V1,
        sources: artifact(SOURCES_PATH),
        uses: artifact(USES_PATH),
        fields: artifact(FIELDS_PATH),
        streams: streams()
            .map(|stream| OriginStreamSummary {
                stream,
                occurrences: DecimalU64::new(u64::MAX),
                fields: DecimalU64::new(u64::MAX),
            })
            .collect(),
    }
}

const fn limit(base: u64) -> MediaError {
    MediaError::Limit {
        budget: "index_bytes",
        limit: base,
    }
}
fn invalid(message: &str) -> MediaError {
    MediaError::Destination(message.into())
}

#[cfg(test)]
#[path = "origin_index_tests.rs"]
mod tests;
