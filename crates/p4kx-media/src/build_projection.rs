// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Write};

use sha2::{Digest as _, Sha256};

use crate::checkpoint::Checkpoint;
use crate::observed_write::ObservedWrite;
use crate::{BuildProvenance, MediaError, json_output};

/// Hashes only the canonical nullable Build projection, without a trailing newline.
pub(crate) fn fingerprint(
    build: Option<&BuildProvenance>,
    limit: u64,
    checkpoint: Checkpoint<'_>,
) -> Result<Option<[u8; 32]>, MediaError> {
    Ok(encode(build, limit, checkpoint, Hash(Sha256::new()))?
        .map(|output| output.0.finalize().into()))
}

pub(crate) fn bytes(
    build: Option<&BuildProvenance>,
    limit: u64,
    checkpoint: Checkpoint<'_>,
) -> Result<Vec<u8>, MediaError> {
    encode(build, limit, checkpoint, Vec::new())?.ok_or(MediaError::Limit {
        budget: "index_bytes",
        limit,
    })
}

fn encode<W: Write>(
    build: Option<&BuildProvenance>,
    limit: u64,
    checkpoint: Checkpoint<'_>,
    output: W,
) -> Result<Option<W>, MediaError> {
    let mut output = Bounded {
        output,
        limit,
        written: 0,
        exceeded: false,
    };
    let result = serde_json::to_writer(ObservedWrite::new(&mut output, &checkpoint), &build);
    if output.exceeded {
        checkpoint.check()?;
        return Ok(None);
    }
    json_output::finish(result, checkpoint)?;
    Ok(Some(output.output))
}

struct Bounded<W> {
    output: W,
    limit: u64,
    written: u64,
    exceeded: bool,
}

impl<W: Write> Write for Bounded<W> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let length = u64::try_from(bytes.len()).map_err(io::Error::other)?;
        if length > self.limit.saturating_sub(self.written) {
            self.exceeded = true;
            return Err(io::Error::other(
                "Build projection exceeds its owning record bound",
            ));
        }
        let written = self.output.write(bytes)?;
        self.written += u64::try_from(written).map_err(io::Error::other)?;
        Ok(written)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.output.flush()
    }
}

struct Hash(Sha256);
impl Write for Hash {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.update(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
