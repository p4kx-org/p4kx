// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};

use p4kx_extraction::OwnedFileToken;
use sha2::{Digest as _, Sha256};

use crate::checkpoint::Checkpoint;
use crate::error;
use crate::hashing;
use crate::output_path;
use crate::{Algorithm, Artifact, ByteScope, Digest, MediaError};

pub(crate) struct Stream<'a> {
    path: PathBuf,
    relative: String,
    file: BufWriter<File>,
    hash: Sha256,
    size: u64,
    token: OwnedFileToken,
    checkpoint: Checkpoint<'a>,
}

impl<'a> Stream<'a> {
    pub(crate) fn create(
        root: &Path,
        relative: String,
        checkpoint: &Checkpoint<'a>,
    ) -> Result<Self, MediaError> {
        let path = root.join(&relative);
        checkpoint.check()?;
        output_path::file(&path)?;
        let (file, token) = p4kx_extraction::create_output_file(
            &path,
            Some(checkpoint.protection),
            checkpoint.cancel.flag(),
        )?
        .into_parts();
        Ok(Self {
            path,
            relative,
            file: BufWriter::new(file),
            hash: Sha256::new(),
            size: 0,
            token,
            checkpoint: *checkpoint,
        })
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }
    pub(crate) fn finish(mut self) -> Result<Artifact, MediaError> {
        self.checkpoint.check()?;
        self.token
            .ensure_writable(&self.path, self.checkpoint.cancel.flag())?;
        self.file
            .flush()
            .map_err(|source| error::io(&self.path, source))?;
        self.file
            .get_ref()
            .sync_all()
            .map_err(|source| error::io(&self.path, source))?;
        Ok(Artifact {
            path: self.relative,
            size: self.size,
            digest: Digest {
                algorithm: Algorithm::Sha256,
                scope: ByteScope::ArtifactFile,
                hex: hashing::hex(self.hash.finalize().into()),
            },
        })
    }
}

impl Write for Stream<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let written = self.file.write(bytes)?;
        self.hash.update(
            bytes.get(..written).ok_or_else(|| {
                io::Error::other("artifact writer reported an invalid byte count")
            })?,
        );
        self.size = self
            .size
            .checked_add(written as u64)
            .ok_or_else(|| io::Error::other("actual artifact size exceeds its wire width"))?;
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.file.flush()
    }
}

pub(crate) struct BoundedLine {
    pub(crate) bytes: Vec<u8>,
    limit: u64,
}

impl BoundedLine {
    pub(crate) const fn new(limit: u64) -> Self {
        Self {
            bytes: Vec::new(),
            limit,
        }
    }
}

impl Write for BoundedLine {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() as u64 > self.limit.saturating_sub(self.bytes.len() as u64) {
            return Err(io::Error::other(MediaError::Limit {
                budget: "record_bytes",
                limit: self.limit,
            }));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
