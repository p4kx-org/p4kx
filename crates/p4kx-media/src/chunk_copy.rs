// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read, Write};
use std::path::Path;

use crate::checkpoint::IO_BLOCK_BYTES;
use crate::error;
use crate::hashing;
use crate::output_path;
use crate::temporary_file::TemporaryFile;
use crate::{Algorithm, ByteScope, Digest, MediaError, MediaWriter};
use sha2::{Digest as _, Sha256};

pub(crate) struct ChunkFile<'a> {
    pub(crate) temporary: TemporaryFile<'a>,
    pub(crate) digest: Digest,
}

pub(crate) fn copy<'a>(
    writer: &MediaWriter<'a>,
    directory: &Path,
    input: &mut dyn Read,
    size: u64,
) -> Result<ChunkFile<'a>, MediaError> {
    writer
        .namespace
        .ensure_namespace_writable(directory, writer.checkpoint.cancel.flag())?;
    if !output_path::directory(directory)? {
        return Err(MediaError::Destination(
            "chunk staging directory is missing".into(),
        ));
    }
    let mut temporary = TemporaryFile::create(directory, writer.checkpoint)?;
    match stream(writer, directory, input, size, &mut temporary) {
        Ok(digest) => Ok(ChunkFile { temporary, digest }),
        Err(primary) => temporary.finish(Err(primary)),
    }
}

fn stream(
    writer: &MediaWriter<'_>,
    directory: &Path,
    input: &mut dyn Read,
    size: u64,
    temporary: &mut TemporaryFile<'_>,
) -> Result<Digest, MediaError> {
    let mut remaining = size;
    let mut hash = Sha256::new();
    let mut buffer = vec![0; IO_BLOCK_BYTES];
    while remaining > 0 {
        writer.checkpoint()?;
        let capacity = usize::try_from(remaining)
            .unwrap_or(usize::MAX)
            .min(buffer.len());
        let bytes = buffer
            .get_mut(..capacity)
            .ok_or_else(|| MediaError::Contract("invalid chunk capacity".into()))?;
        let read = input
            .read(bytes)
            .map_err(|source| error::io(directory, source))?;
        if read == 0 {
            return Err(MediaError::Contract(
                "typed chunk payload is truncated".into(),
            ));
        }
        let bytes = bytes
            .get(..read)
            .ok_or_else(|| MediaError::Contract("invalid chunk reader result".into()))?;
        temporary
            .write_all(bytes)
            .map_err(|source| error::io(directory, source))?;
        hash.update(bytes);
        remaining -= read as u64;
    }
    writer.checkpoint()?;
    if input
        .read(&mut [0])
        .map_err(|source| error::io(directory, source))?
        != 0
    {
        return Err(MediaError::Contract(
            "typed chunk payload exceeds its declared count".into(),
        ));
    }
    temporary.sync_all()?;
    Ok(Digest {
        algorithm: Algorithm::Sha256,
        scope: ByteScope::ArtifactFile,
        hex: hashing::hex(hash.finalize().into()),
    })
}
