// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
#[cfg(unix)]
use std::fs::File;
use std::path::{Path, PathBuf};

use p4kx_extraction::{ConflictPolicy, OwnedDirectoryPermit};

use crate::checkpoint::Checkpoint;
use crate::error;
use crate::generation_lock::GenerationLock;
use crate::generation_validation;
use crate::index_destination::IndexDestination;
use crate::index_file;
use crate::output_path;
use crate::publication::Publication;
use crate::staged_directory::StagedDirectory;
use crate::{Index, Limits, MediaError};

pub(crate) struct Generation<'a> {
    pub(crate) output: PathBuf,
    pub(crate) stage: StagedDirectory<'a>,
    lock: GenerationLock,
    index_destination: IndexDestination,
    pub(crate) previous: Option<Index>,
}

struct Placement {
    index: PathBuf,
    namespace: Option<OwnedDirectoryPermit>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Boundary {
    ArtifactsComplete,
    GenerationPlaced,
    IndexSynced,
    BeforeIndexReplace,
}

impl<'a> Generation<'a> {
    pub(crate) fn new(
        output: &Path,
        limits: Limits,
        checkpoint: &Checkpoint<'a>,
    ) -> Result<Self, MediaError> {
        Self::with_stage(output, limits, checkpoint, |output, checkpoint| {
            StagedDirectory::create(output, ".media-stage-", checkpoint)
        })
    }

    pub(crate) fn with_stage(
        output: &Path,
        limits: Limits,
        checkpoint: &Checkpoint<'a>,
        stage: impl FnOnce(&Path, Checkpoint<'a>) -> Result<StagedDirectory<'a>, MediaError>,
    ) -> Result<Self, MediaError> {
        checkpoint.check()?;
        let output = if output.is_absolute() {
            output.into()
        } else {
            std::env::current_dir()
                .map_err(|source| error::io(output, source))?
                .join(output)
        };
        output_path::ensure_directory(&output, checkpoint)?;
        let lock = GenerationLock::acquire(&output, checkpoint)?;
        let bound = (|| {
            let destination = IndexDestination::bind(&output, limits, checkpoint)?;
            lock.check(checkpoint)?;
            Ok(destination)
        })();
        let (index_destination, previous) = match bound {
            Ok(destination) => destination,
            Err(primary) => return error::finish(Err(primary), lock.release()),
        };
        let stage = match stage(&output, *checkpoint) {
            Ok(stage) => stage,
            Err(primary) => return error::finish(Err(primary), lock.release()),
        };
        let generation = Self {
            output,
            stage,
            lock,
            index_destination,
            previous,
        };
        let prepared = (|| {
            output_path::ensure_directory(&generation.stage.path().join("raw"), checkpoint)?;
            output_path::ensure_directory(&generation.stage.path().join("chunks"), checkpoint)
        })();
        if let Err(primary) = prepared {
            return generation.finish(Err(primary));
        }
        Ok(generation)
    }

    pub(crate) fn publish(
        &mut self,
        index: &mut Index,
        policy: ConflictPolicy,
        checkpoint: &Checkpoint<'_>,
    ) -> Result<bool, MediaError> {
        self.publish_observed(index, policy, checkpoint, |_| Ok(()))
    }

    pub(crate) fn publish_observed(
        &mut self,
        index: &mut Index,
        policy: ConflictPolicy,
        checkpoint: &Checkpoint<'_>,
        observe: impl Fn(Boundary) -> Result<(), MediaError>,
    ) -> Result<bool, MediaError> {
        checkpoint.finish()?;
        self.lock.check(checkpoint)?;
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
        let placed = self.place(index, checkpoint)?;
        observe(Boundary::GenerationPlaced)?;
        let publication = if index.complete {
            Publication::RootIndex(&self.index_destination)
        } else {
            Publication::Namespace(
                placed
                    .namespace
                    .as_ref()
                    .unwrap_or_else(|| self.stage.permit()),
            )
        };
        index_file::write(index, &placed.index, publication, checkpoint, &observe)?;
        Ok(true)
    }

    fn place(
        &mut self,
        index: &mut Index,
        checkpoint: &Checkpoint<'_>,
    ) -> Result<Placement, MediaError> {
        let base = if index.complete {
            "generations"
        } else {
            "diagnostics"
        };
        let relative = format!("{base}/{}", index.generation);
        let parent = self.output.join(base);
        output_path::ensure_directory(&parent, checkpoint)?;
        let destination = self.output.join(&relative);
        checkpoint.writable(&destination)?;
        let namespace = if output_path::directory(&destination)? {
            if index.complete {
                generation_validation::artifacts(&destination, index, index.limits, checkpoint)?;
                None
            } else {
                Some(p4kx_extraction::validated_output_directory(
                    &destination,
                    Some(checkpoint.protection),
                    checkpoint.cancel.flag(),
                    |root| {
                        generation_validation::artifacts(root, index, index.limits, checkpoint)
                            .map_err(std::io::Error::other)
                    },
                )?)
            }
        } else {
            self.stage
                .permit()
                .ensure_namespace_writable(self.stage.path(), checkpoint.cancel.flag())?;
            fs::rename(self.stage.path(), &destination)
                .map_err(|source| error::io(&destination, source))?;
            self.stage.published_at(&destination)?;
            #[cfg(unix)]
            sync_directory(&parent)?;
            None
        };
        #[cfg(unix)]
        sync_directory(&self.output)?;
        index.artifact_base = if index.complete { relative } else { ".".into() };
        let index_path = if index.complete {
            self.output.join("media-index.json")
        } else {
            destination.join("media-index.json")
        };
        Ok(Placement {
            index: index_path,
            namespace,
        })
    }

    pub(crate) fn finish<T>(self, result: Result<T, MediaError>) -> Result<T, MediaError> {
        let cleanup = error::finish(self.stage.close(), self.lock.release());
        error::finish(result, cleanup)
    }
}

#[cfg(unix)]
pub(crate) fn sync_directory(path: &Path) -> Result<(), MediaError> {
    File::open(path)
        .and_then(|file| file.sync_all())
        .map_err(|source| error::io(path, source))?;
    Ok(())
}
