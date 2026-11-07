// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs::File;
use std::path::{Path, PathBuf};

use crate::MediaError;
use crate::checkpoint::Checkpoint;
use crate::error;
use crate::lock_namespace::LockNamespace;

pub(crate) struct GenerationLock {
    file: File,
    path: PathBuf,
    namespace: LockNamespace,
}

impl GenerationLock {
    pub(crate) fn acquire(output: &Path, checkpoint: &Checkpoint<'_>) -> Result<Self, MediaError> {
        let namespace = LockNamespace::bind(output, checkpoint)?;
        let path = namespace.path().to_path_buf();
        let file = namespace.open(checkpoint)?;
        namespace.validate(&file, checkpoint)?;
        file.try_lock()
            .map_err(|source| error::io(&path, source.into()))?;
        namespace.validate(&file, checkpoint)?;
        Ok(Self {
            file,
            path,
            namespace,
        })
    }

    pub(crate) fn check(&self, checkpoint: &Checkpoint<'_>) -> Result<(), MediaError> {
        self.namespace.validate(&self.file, checkpoint)
    }

    pub(crate) fn release(self) -> Result<(), MediaError> {
        self.file
            .unlock()
            .map_err(|source| error::io(&self.path, source))
    }
}
