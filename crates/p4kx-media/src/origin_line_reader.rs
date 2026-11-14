// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs::File;
use std::io::{self, BufRead, BufReader, Read};
use std::path::{Component, Path, PathBuf};

use serde::de::DeserializeSeed;
use sha2::{Digest as _, Sha256};

use crate::checkpoint::{Checkpoint, IO_BLOCK_BYTES};
use crate::{Algorithm, Artifact, ByteScope, MediaError};
use crate::{error, hashing, output_path};

/// Reads bounded JSONL rows directly while verifying the complete original receipt.
pub(crate) struct OriginLines<'receipt, 'check> {
    reader: BufReader<File>,
    path: PathBuf,
    receipt: &'receipt Artifact,
    checkpoint: Checkpoint<'check>,
    bytes: u64,
    hash: Sha256,
    hash_buffer: Vec<u8>,
    line_bytes: u64,
}

impl<'receipt, 'check> OriginLines<'receipt, 'check> {
    pub(crate) fn open(
        root: &Path,
        receipt: &'receipt Artifact,
        checkpoint: Checkpoint<'check>,
        line_bytes: u64,
    ) -> Result<Self, MediaError> {
        checkpoint.check()?;
        let path = checked_path(root, receipt)?;
        let file = File::open(&path).map_err(|cause| error::io(&path, cause))?;
        Ok(Self {
            reader: BufReader::with_capacity(IO_BLOCK_BYTES, file),
            path,
            receipt,
            checkpoint,
            bytes: 0,
            hash: Sha256::new(),
            hash_buffer: Vec::with_capacity(IO_BLOCK_BYTES),
            line_bytes,
        })
    }

    pub(crate) fn next<T>(
        &mut self,
        seed: impl for<'de> DeserializeSeed<'de, Value = T>,
    ) -> Result<Option<T>, MediaError> {
        self.checkpoint.check()?;
        let buffer = self
            .reader
            .fill_buf()
            .map_err(|cause| error::io(&self.path, cause))?;
        if buffer.is_empty() {
            return Ok(None);
        }
        if let Some(newline) = buffer.iter().position(|byte| *byte == b'\n') {
            let count = newline + 1;
            let row = buffer
                .get(..count)
                .ok_or_else(|| MediaError::Contract("invalid buffered origin row".into()))?;
            self.bytes = checked_bytes(self.bytes, count, self.receipt.size)?;
            checked_bytes(0, count, self.line_bytes)?;
            let mut json = serde_json::Deserializer::from_slice(row);
            let value = seed.deserialize(&mut json)?;
            json.end()?;
            hash_bytes(&mut self.hash, &mut self.hash_buffer, row);
            self.reader.consume(count);
            self.checkpoint.check()?;
            return Ok(Some(value));
        }
        self.next_spanning(seed).map(Some)
    }

    fn next_spanning<T>(
        &mut self,
        seed: impl for<'de> DeserializeSeed<'de, Value = T>,
    ) -> Result<T, MediaError> {
        let mut line = Line {
            reader: &mut self.reader,
            path: &self.path,
            checkpoint: self.checkpoint,
            total: &mut self.bytes,
            hash: &mut self.hash,
            hash_buffer: &mut self.hash_buffer,
            maximum: self.receipt.size,
            row_limit: self.line_bytes,
            row_bytes: 0,
            unchecked: 0,
            newline: false,
            failure: None,
        };
        let parsed = {
            let mut json = serde_json::Deserializer::from_reader(&mut line);
            seed.deserialize(&mut json)
                .and_then(|value| json.end().map(|()| value))
        };
        if let Some(failure) = line.failure {
            return Err(failure);
        }
        let value = parsed?;
        if !line.newline {
            return Err(MediaError::Destination(
                "origin JSONL row lacks its newline".into(),
            ));
        }
        self.checkpoint.check()?;
        Ok(value)
    }

    pub(crate) fn finish(mut self) -> Result<(), MediaError> {
        self.checkpoint.check()?;
        self.hash.update(self.hash_buffer.as_slice());
        if !self
            .reader
            .fill_buf()
            .map_err(|cause| error::io(&self.path, cause))?
            .is_empty()
            || self.bytes != self.receipt.size
            || hashing::hex(self.hash.finalize().into()) != self.receipt.digest.hex
        {
            return Err(MediaError::Destination(
                "origin stream integrity mismatch".into(),
            ));
        }
        self.checkpoint.check()
    }
}

fn checked_path(root: &Path, receipt: &Artifact) -> Result<PathBuf, MediaError> {
    if receipt.digest.algorithm != Algorithm::Sha256
        || receipt.digest.scope != ByteScope::ArtifactFile
        || receipt.digest.hex.len() != 64
        || !receipt
            .digest
            .hex
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
        || receipt.path.is_empty()
    {
        return Err(MediaError::Destination("invalid origin receipt".into()));
    }
    let mut path = root.to_path_buf();
    for component in Path::new(&receipt.path).components() {
        if !matches!(component, Component::Normal(_)) {
            return Err(MediaError::Destination(
                "unsafe origin artifact path".into(),
            ));
        }
        path.push(component);
    }
    if !output_path::file(&path)? {
        return Err(MediaError::Destination("origin artifact is missing".into()));
    }
    Ok(path)
}

struct Line<'a, 'check> {
    reader: &'a mut BufReader<File>,
    path: &'a Path,
    checkpoint: Checkpoint<'check>,
    total: &'a mut u64,
    hash: &'a mut Sha256,
    hash_buffer: &'a mut Vec<u8>,
    maximum: u64,
    row_limit: u64,
    row_bytes: u64,
    unchecked: usize,
    newline: bool,
    failure: Option<MediaError>,
}

impl Read for Line<'_, '_> {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        if output.is_empty() || self.newline {
            return Ok(0);
        }
        match self.read_part(output) {
            Ok(count) => Ok(count),
            Err(error) => {
                self.failure = Some(error);
                Err(io::Error::other("origin stream read failed"))
            }
        }
    }
}

impl Line<'_, '_> {
    fn read_part(&mut self, output: &mut [u8]) -> Result<usize, MediaError> {
        if self.unchecked >= IO_BLOCK_BYTES {
            self.checkpoint.check()?;
            self.unchecked = 0;
        }
        let available = self
            .reader
            .fill_buf()
            .map_err(|cause| error::io(self.path, cause))?;
        let available = available
            .get(..available.len().min(output.len()))
            .ok_or_else(|| MediaError::Contract("invalid origin read prefix".into()))?;
        let count = available
            .iter()
            .position(|byte| *byte == b'\n')
            .map_or(available.len(), |index| index + 1);
        let bytes = available
            .get(..count)
            .ok_or_else(|| MediaError::Contract("invalid origin read span".into()))?;
        output
            .get_mut(..count)
            .ok_or_else(|| MediaError::Contract("invalid origin read buffer".into()))?
            .copy_from_slice(bytes);
        self.reader.consume(count);
        let bytes = output
            .get(..count)
            .ok_or_else(|| MediaError::Contract("invalid consumed origin read span".into()))?;
        self.observe(bytes)?;
        Ok(count)
    }
    fn observe(&mut self, bytes: &[u8]) -> Result<(), MediaError> {
        let count = bytes.len();
        *self.total = checked_bytes(*self.total, count, self.maximum)?;
        self.row_bytes = checked_bytes(self.row_bytes, count, self.row_limit)?;
        hash_bytes(self.hash, self.hash_buffer, bytes);
        self.newline = bytes.last() == Some(&b'\n');
        self.unchecked = self.unchecked.saturating_add(count);
        Ok(())
    }
}

fn checked_bytes(previous: u64, count: usize, maximum: u64) -> Result<u64, MediaError> {
    let total = u64::try_from(count)
        .ok()
        .and_then(|count| previous.checked_add(count))
        .ok_or_else(|| MediaError::Destination("origin stream byte count overflow".into()))?;
    if total > maximum {
        return Err(MediaError::Destination(
            "origin stream exceeds its reader byte policy".into(),
        ));
    }
    Ok(total)
}

fn hash_bytes(hash: &mut Sha256, buffer: &mut Vec<u8>, bytes: &[u8]) {
    if bytes.len() >= IO_BLOCK_BYTES - buffer.len() {
        hash.update(buffer.as_slice());
        hash.update(bytes);
        buffer.clear();
    } else {
        buffer.extend_from_slice(bytes);
    }
}

#[cfg(test)]
#[path = "origin_line_reader_tests.rs"]
mod tests;
