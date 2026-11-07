// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs::File;
use std::path::{Path, PathBuf};

use crate::MediaError;
use crate::error;
use crate::output_path;

pub(crate) struct GenerationLock {
    file: File,
    path: PathBuf,
}

impl GenerationLock {
    pub(crate) fn acquire(output: &Path) -> Result<Self, MediaError> {
        let path = output.join(".media.lock");
        p4kx_extraction::ensure_writable(&path)?;
        output_path::file(&path)?;
        let file = File::options()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)
            .map_err(|source| error::io(&path, source))?;
        file.try_lock()
            .map_err(|source| error::io(&path, source.into()))?;
        Ok(Self { file, path })
    }

    pub(crate) fn release(self) -> Result<(), MediaError> {
        self.file
            .unlock()
            .map_err(|source| error::io(&self.path, source))
    }
}
