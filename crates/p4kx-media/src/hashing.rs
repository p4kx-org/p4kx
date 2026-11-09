// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs::File;
use std::io::{self, Read, Write};
use std::path::Path;

use p4kx_extraction::Checksum;
use sha2::{Digest as _, Sha256};

use crate::checkpoint::{Checkpoint, IO_BLOCK_BYTES};
use crate::error;
use crate::json_output;
use crate::observed_write::ObservedWrite;
use crate::origin_index::{Budget, Encoding};
use crate::{Algorithm, ByteScope, Digest, MediaError};

pub(crate) fn file(
    path: &Path,
    scope: ByteScope,
    limit: u64,
    checkpoint: &Checkpoint<'_>,
) -> Result<(Digest, u64), MediaError> {
    checkpoint.check()?;
    let mut input = File::open(path).map_err(|source| error::io(path, source))?;
    if input
        .metadata()
        .map_err(|source| error::io(path, source))?
        .len()
        > limit
    {
        return Err(MediaError::Limit {
            budget: "hash_bytes",
            limit,
        });
    }
    let mut hash = Sha256::new();
    let mut size = 0_u64;
    let mut buffer = vec![0; IO_BLOCK_BYTES];
    loop {
        checkpoint.check()?;
        let read = input
            .read(&mut buffer)
            .map_err(|source| error::io(path, source))?;
        if read == 0 {
            break;
        }
        size = size.checked_add(read as u64).ok_or(MediaError::Limit {
            budget: "hash_bytes",
            limit,
        })?;
        if size > limit {
            return Err(MediaError::Limit {
                budget: "hash_bytes",
                limit,
            });
        }
        hash.update(buffer.get(..read).ok_or_else(|| {
            MediaError::Contract("hash reader reported an invalid byte count".into())
        })?);
    }
    Ok((
        Digest {
            algorithm: Algorithm::Sha256,
            scope,
            hex: hex(hash.finalize().into()),
        },
        size,
    ))
}

pub(crate) fn generation(
    index: &mut crate::Index,
    checkpoint: &Checkpoint<'_>,
) -> Result<String, MediaError> {
    checkpoint.check()?;
    let budget = Budget::for_index(index, Encoding::Compact)?;
    let generation = std::mem::take(&mut index.generation);
    let base = std::mem::take(&mut index.artifact_base);
    let mut writer = HashWriter {
        hash: Sha256::new(),
        budget,
        written: 0,
    };
    let serialised = serde_json::to_writer(ObservedWrite::new(&mut writer, checkpoint), &*index);
    index.generation = generation;
    index.artifact_base = base;
    json_output::finish(serialised, *checkpoint)?;
    budget.finish(writer.written)?;
    Ok(hex(writer.hash.finalize().into()))
}

struct HashWriter {
    hash: Sha256,
    budget: Budget,
    written: u64,
}

impl Write for HashWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let next = self
            .budget
            .admit(self.written, bytes.len())
            .map_err(io::Error::other)?;
        self.hash.update(bytes);
        self.written = next;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Formats a SHA-256 word in its canonical lower-case hexadecimal spelling.
pub(crate) fn hex(bytes: [u8; 32]) -> String {
    Checksum::Sha256(bytes).hex()
}
