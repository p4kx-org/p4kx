// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
#[cfg(unix)]
use std::fs::File;
use std::path::{Path, PathBuf};

use p4kx_extraction::ConflictPolicy;
use tempfile::TempDir;

use crate::checkpoint::Checkpoint;
use crate::error;
use crate::generation_lock::GenerationLock;
use crate::generation_validation;
use crate::index_file;
use crate::output_path;
use crate::{Index, Limits, MediaError};

pub(crate) struct Generation {
    pub(crate) output: PathBuf,
    pub(crate) stage: TempDir,
    lock: GenerationLock,
    pub(crate) previous: Option<Index>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Boundary {
    ArtifactsComplete,
    GenerationPlaced,
    IndexSynced,
    BeforeIndexReplace,
}

impl Generation {
    pub(crate) fn new(
        output: &Path,
        limits: Limits,
        checkpoint: &Checkpoint<'_>,
    ) -> Result<Self, MediaError> {
        Self::with_stage(output, limits, checkpoint, |output| {
            tempfile::Builder::new()
                .prefix(".media-stage-")
                .tempdir_in(output)
        })
    }

    pub(crate) fn with_stage(
        output: &Path,
        limits: Limits,
        checkpoint: &Checkpoint<'_>,
        stage: impl FnOnce(&Path) -> std::io::Result<TempDir>,
    ) -> Result<Self, MediaError> {
        checkpoint.check()?;
        let output = if output.is_absolute() {
            output.into()
        } else {
            std::env::current_dir()
                .map_err(|source| error::io(output, source))?
                .join(output)
        };
        output_path::ensure_directory(&output)?;
        let lock = GenerationLock::acquire(&output)?;
        let stage = stage(&output).map_err(|source| error::io(&output, source))?;
        let mut generation = Self {
            output,
            stage,
            lock,
            previous: None,
        };
        generation.previous =
            generation_validation::existing(&generation.output, limits, checkpoint)?;
        output_path::ensure_directory(&generation.stage.path().join("raw"))?;
        output_path::ensure_directory(&generation.stage.path().join("chunks"))?;
        Ok(generation)
    }

    pub(crate) fn publish(
        &self,
        index: &mut Index,
        policy: ConflictPolicy,
        checkpoint: &Checkpoint<'_>,
    ) -> Result<bool, MediaError> {
        self.publish_observed(index, policy, checkpoint, |_| Ok(()))
    }

    pub(crate) fn publish_observed(
        &self,
        index: &mut Index,
        policy: ConflictPolicy,
        checkpoint: &Checkpoint<'_>,
        observe: impl Fn(Boundary) -> Result<(), MediaError>,
    ) -> Result<bool, MediaError> {
        checkpoint.finish()?;
        generation_validation::artifacts(self.stage.path(), index, index.limits, checkpoint)?;
        #[cfg(unix)]
        {
            sync_directory(&self.stage.path().join("raw"))?;
            sync_directory(&self.stage.path().join("chunks"))?;
            sync_directory(self.stage.path())?;
        }
        observe(Boundary::ArtifactsComplete)?;
        if policy == ConflictPolicy::IfChanged
            && let Some(previous) = self
                .previous
                .as_ref()
                .filter(|previous| previous.generation == index.generation)
        {
            index.artifact_base.clone_from(&previous.artifact_base);
            return Ok(false);
        }
        let index_path = self.place(index, checkpoint)?;
        observe(Boundary::GenerationPlaced)?;
        index_file::write(index, &index_path, checkpoint, &observe)?;
        Ok(true)
    }

    fn place(&self, index: &mut Index, checkpoint: &Checkpoint<'_>) -> Result<PathBuf, MediaError> {
        let base = if index.complete {
            "generations"
        } else {
            "diagnostics"
        };
        let relative = format!("{base}/{}", index.generation);
        let parent = self.output.join(base);
        output_path::ensure_directory(&parent)?;
        let destination = self.output.join(&relative);
        if output_path::directory(&destination)? {
            generation_validation::artifacts(&destination, index, index.limits, checkpoint)?;
        } else {
            fs::rename(self.stage.path(), &destination)
                .map_err(|source| error::io(&destination, source))?;
            #[cfg(unix)]
            sync_directory(&parent)?;
        }
        #[cfg(unix)]
        sync_directory(&self.output)?;
        index.artifact_base = if index.complete { relative } else { ".".into() };
        let index_path = if index.complete {
            self.output.join("media-index.json")
        } else {
            destination.join("media-index.json")
        };
        Ok(index_path)
    }

    pub(crate) fn release(self) -> Result<(), MediaError> {
        self.lock.release()
    }
}

#[cfg(unix)]
pub(crate) fn sync_directory(path: &Path) -> Result<(), MediaError> {
    File::open(path)
        .and_then(|file| file.sync_all())
        .map_err(|source| error::io(path, source))?;
    Ok(())
}
