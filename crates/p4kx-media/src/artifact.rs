// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};

#[cfg(test)]
use p4kx_extraction::FieldOrigin;
use p4kx_extraction::OwnedFileToken;
use sha2::{Digest as _, Sha256};

use crate::checkpoint::Checkpoint;
use crate::error;
use crate::hashing;
#[cfg(test)]
use crate::origin_contract::use_sets::SetId;
use crate::output_path;
use crate::{Algorithm, Artifact, ByteScope, Digest, MediaError};

#[cfg(test)]
pub(crate) trait StreamObserver {
    fn write(&self, relative: &str, bytes: &[u8]) -> io::Result<()>;

    fn finish(&self, _relative: &str) -> io::Result<()> {
        Ok(())
    }

    fn origin_field(&self, _origin: &FieldOrigin, _shared: Option<SetId>) -> io::Result<()> {
        Ok(())
    }

    /// Returns whether the complete typed chunk is new to the observed destination.
    fn typed_chunk(&self, _artifact: &Artifact) -> io::Result<bool> {
        Err(io::Error::other("typed chunk has no observation sink"))
    }
}

#[derive(Clone, Copy)]
pub(crate) struct StreamDestination<'a> {
    checkpoint: Checkpoint<'a>,
    #[cfg(test)]
    observer: Option<&'a dyn StreamObserver>,
}

impl<'a> StreamDestination<'a> {
    pub(crate) const fn file(checkpoint: Checkpoint<'a>) -> Self {
        Self {
            checkpoint,
            #[cfg(test)]
            observer: None,
        }
    }

    #[cfg(test)]
    pub(crate) const fn observed(
        checkpoint: Checkpoint<'a>,
        observer: &'a dyn StreamObserver,
    ) -> Self {
        Self {
            checkpoint,
            observer: Some(observer),
        }
    }

    pub(crate) const fn checkpoint(self) -> Checkpoint<'a> {
        self.checkpoint
    }

    #[cfg(test)]
    pub(crate) const fn observer(self) -> Option<&'a dyn StreamObserver> {
        self.observer
    }

    pub(crate) fn create(self, root: &Path, relative: String) -> Result<Stream<'a>, MediaError> {
        #[cfg(test)]
        if let Some(observer) = self.observer {
            self.checkpoint.check()?;
            return Ok(Stream::new(
                root.join(&relative),
                relative,
                StreamBackend::Observed {
                    observer,
                    checkpoint: self.checkpoint,
                },
            ));
        }
        Stream::create(root, relative, &self.checkpoint)
    }
}

enum StreamBackend<'a> {
    File {
        file: BufWriter<File>,
        token: OwnedFileToken,
        checkpoint: Checkpoint<'a>,
    },
    #[cfg(test)]
    Observed {
        observer: &'a dyn StreamObserver,
        checkpoint: Checkpoint<'a>,
    },
}

pub(crate) struct Stream<'a> {
    path: PathBuf,
    relative: String,
    backend: StreamBackend<'a>,
    hash: Sha256,
    size: u64,
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
        Ok(Self::new(
            path,
            relative,
            StreamBackend::File {
                file: BufWriter::new(file),
                token,
                checkpoint: *checkpoint,
            },
        ))
    }

    fn new(path: PathBuf, relative: String, backend: StreamBackend<'a>) -> Self {
        Self {
            path,
            relative,
            backend,
            hash: Sha256::new(),
            size: 0,
        }
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn finish(mut self) -> Result<Artifact, MediaError> {
        match &mut self.backend {
            StreamBackend::File {
                file,
                token,
                checkpoint,
            } => {
                checkpoint.check()?;
                token.ensure_writable(&self.path, checkpoint.cancel.flag())?;
                file.flush()
                    .map_err(|source| error::io(&self.path, source))?;
                file.get_ref()
                    .sync_all()
                    .map_err(|source| error::io(&self.path, source))?;
            }
            #[cfg(test)]
            StreamBackend::Observed {
                observer,
                checkpoint,
            } => {
                checkpoint.check()?;
                observer
                    .finish(&self.relative)
                    .map_err(|source| error::io(&self.path, source))?;
            }
        }
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
        let written = match &mut self.backend {
            StreamBackend::File { file, .. } => file.write(bytes)?,
            #[cfg(test)]
            StreamBackend::Observed { observer, .. } => {
                observer.write(&self.relative, bytes)?;
                bytes.len()
            }
        };
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
        match &mut self.backend {
            StreamBackend::File { file, .. } => file.flush(),
            #[cfg(test)]
            StreamBackend::Observed { .. } => Ok(()),
        }
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
