// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{ArchiveWatch, CancelToken};

use crate::MediaError;

pub(crate) const IO_BLOCK_BYTES: usize = 64 << 10;

#[derive(Clone, Copy)]
pub(crate) struct Checkpoint<'a> {
    pub(crate) cancel: &'a CancelToken,
    pub(crate) watch: &'a ArchiveWatch,
}

impl Checkpoint<'_> {
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
