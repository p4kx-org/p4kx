// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_extraction::{ArchiveWatch, ArchiveWriteProtection, CancelToken};

use crate::MediaError;

pub(crate) const IO_BLOCK_BYTES: usize = 64 << 10;

#[derive(Clone, Copy)]
pub(crate) struct Checkpoint<'a> {
    pub(crate) cancel: &'a CancelToken,
    pub(crate) watch: &'a ArchiveWatch,
    pub(crate) protection: &'a ArchiveWriteProtection,
}

impl Checkpoint<'_> {
    pub(crate) fn writable(&self, path: &Path) -> Result<(), MediaError> {
        self.check()?;
        self.cleanup_writable(path)
    }

    pub(crate) fn cleanup_writable(&self, path: &Path) -> Result<(), MediaError> {
        self.protection.ensure_writable(path, self.cancel.flag())?;
        Ok(())
    }

    pub(crate) fn check(&self) -> Result<(), MediaError> {
        if self.cancel.is_cancelled() {
            return Err(MediaError::Cancelled);
        }
        self.watch.poll()?;
        Ok(())
    }

    pub(crate) fn finish(&self) -> Result<(), MediaError> {
        self.check()?;
        self.watch.check()?;
        Ok(())
    }
}
