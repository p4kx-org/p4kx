// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs::{File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};

use p4kx_extraction::OwnedDirectoryPermit;

use crate::checkpoint::Checkpoint;
use crate::{MediaError, error, output_path};

pub(crate) struct LockNamespace {
    permit: OwnedDirectoryPermit,
    path: PathBuf,
}

impl LockNamespace {
    pub(crate) fn bind(output: &Path, checkpoint: &Checkpoint<'_>) -> Result<Self, MediaError> {
        let permit = p4kx_extraction::validated_output_directory(
            output,
            Some(checkpoint.protection),
            checkpoint.cancel.flag(),
            |root| {
                output_path::file(&root.join(".media.lock"))
                    .map(|_| ())
                    .map_err(io::Error::other)
            },
        )?;
        let path = permit.path().join(".media.lock");
        Ok(Self { permit, path })
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn check(&self, checkpoint: &Checkpoint<'_>) -> Result<(), MediaError> {
        checkpoint.check()?;
        self.permit
            .ensure_namespace_writable(&self.path, checkpoint.cancel.flag())?;
        output_path::file(&self.path)?;
        Ok(())
    }

    pub(crate) fn open(&self, checkpoint: &Checkpoint<'_>) -> Result<File, MediaError> {
        self.check(checkpoint)?;
        match File::open(&self.path) {
            Ok(file) => Ok(file),
            Err(source) if source.kind() == io::ErrorKind::NotFound => {
                self.check(checkpoint)?;
                match OpenOptions::new()
                    .read(true)
                    .write(true)
                    .create_new(true)
                    .open(&self.path)
                {
                    Ok(file) => Ok(file),
                    Err(source) if source.kind() == io::ErrorKind::AlreadyExists => {
                        File::open(&self.path).map_err(|source| error::io(&self.path, source))
                    }
                    Err(source) => Err(error::io(&self.path, source)),
                }
            }
            Err(source) => Err(error::io(&self.path, source)),
        }
    }

    pub(crate) fn validate(
        &self,
        file: &File,
        checkpoint: &Checkpoint<'_>,
    ) -> Result<(), MediaError> {
        self.check(checkpoint)?;
        let metadata = file
            .metadata()
            .map_err(|source| error::io(&self.path, source))?;
        if !metadata.is_file() || metadata.len() != 0 {
            return Err(MediaError::Destination(
                ".media.lock is not an empty regular file".into(),
            ));
        }
        self.check(checkpoint)
    }
}
