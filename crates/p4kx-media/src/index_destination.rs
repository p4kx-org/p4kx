// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;
use std::path::{Path, PathBuf};

use p4kx_extraction::OwnedDirectoryPermit;

use crate::checkpoint::Checkpoint;
use crate::generation_validation;
use crate::{Index, Limits, MediaError};

pub(crate) struct IndexDestination {
    permit: OwnedDirectoryPermit,
    path: PathBuf,
}

impl IndexDestination {
    pub(crate) fn bind(
        output: &Path,
        limits: Limits,
        checkpoint: &Checkpoint<'_>,
    ) -> Result<(Self, Option<Index>), MediaError> {
        let mut previous = None;
        let permit = p4kx_extraction::validated_output_directory(
            output,
            Some(checkpoint.protection),
            checkpoint.cancel.flag(),
            |root| {
                previous = generation_validation::existing(root, limits, checkpoint)
                    .map_err(io::Error::other)?;
                Ok(())
            },
        )?;
        let path = permit.path().join("media-index.json");
        Ok((Self { permit, path }, previous))
    }

    pub(crate) fn check(&self, path: &Path, checkpoint: &Checkpoint<'_>) -> Result<(), MediaError> {
        if path != self.path {
            return Err(MediaError::Contract(
                "root index namespace does not authorize another output".into(),
            ));
        }
        checkpoint.check()?;
        self.permit
            .ensure_namespace_writable(path, checkpoint.cancel.flag())?;
        Ok(())
    }
}
