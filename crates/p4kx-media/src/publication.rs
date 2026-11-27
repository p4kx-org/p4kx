// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_extraction::OwnedDirectoryPermit;

use crate::MediaError;
use crate::checkpoint::Checkpoint;
use crate::index_destination::IndexDestination;

#[derive(Clone, Copy)]
pub(crate) enum Publication<'a> {
    Namespace(&'a OwnedDirectoryPermit),
    RootIndex(&'a IndexDestination),
}

impl Publication<'_> {
    pub(crate) fn check(&self, path: &Path, checkpoint: &Checkpoint<'_>) -> Result<(), MediaError> {
        match self {
            Self::Namespace(permit) => {
                checkpoint.check()?;
                permit.ensure_namespace_writable(path, checkpoint.cancel.flag())?;
                Ok(())
            }
            Self::RootIndex(destination) => destination.check(path, checkpoint),
        }
    }
}
