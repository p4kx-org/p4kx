// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;

use p4kx_extraction::model::LodSelection;
use p4kx_extraction::{
    AnimationChoice, AnimationSet, ArchiveTextures, CancelToken, ConversionError, GlbOptions,
    MaterialMode, ModelSource, PathIndex, TextureLoader, archive_error_in,
};
use p4kx_settings::ModelMaterials;

use super::{ModelExportFailure, ModelExportSummary};
use crate::model::describe;
use crate::{EntryIndex, NoEntries};

#[cfg(test)]
#[path = "job_tests.rs"]
mod tests;

/// What an Export as GLB writes and where.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Request {
    pub(crate) entry_path: String,
    pub(crate) file: PathBuf,
    pub(crate) lod: u32,
    pub(crate) materials: ModelMaterials,
    pub(crate) texture_mip: u32,
    pub(crate) animations: Option<AnimationChoice>,
    pub(crate) vertex_colours: bool,
    #[cfg(feature = "test-hooks")]
    pub(crate) pause: crate::hold::Pause,
}

impl Request {
    fn options(&self) -> GlbOptions {
        let mode = match self.materials {
            ModelMaterials::Colours => MaterialMode::Colours,
            ModelMaterials::Textures => MaterialMode::Textures,
        };
        let options = GlbOptions::default()
            .with_lod(self.lod)
            .with_materials(mode)
            .with_texture_mip(self.texture_mip);
        if self.vertex_colours {
            options
        } else {
            options.without_vertex_colours()
        }
    }
}

/// The step a running Export as GLB is at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelExportStage {
    /// The model and its companions are read from the Archive.
    Reading,
    /// The GLB file is written.
    Writing,
}

/// How far a running Export as GLB has come.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModelExportProgress {
    /// The step it is at.
    pub stage: ModelExportStage,
    /// The bytes of the file written so far.
    pub done: u64,
    /// The length of the file, known once writing starts.
    pub total: u64,
}

#[derive(Debug, Default)]
struct Shared {
    stage: AtomicU8,
    done: AtomicU64,
    total: AtomicU64,
}

impl Shared {
    fn progress(&self) -> ModelExportProgress {
        ModelExportProgress {
            stage: if self.stage.load(Ordering::Relaxed) == 0 {
                ModelExportStage::Reading
            } else {
                ModelExportStage::Writing
            },
            done: self.done.load(Ordering::Relaxed),
            total: self.total.load(Ordering::Relaxed),
        }
    }
}

/// Where an Export as GLB stands.
#[derive(Debug, Clone, Copy)]
pub enum ModelExportStatus<'a> {
    /// The export runs; the progress so far.
    Running(ModelExportProgress),
    /// The file and its origin receipt were published.
    Finished(&'a ModelExportSummary),
    /// The export was cancelled; no file was written.
    Cancelled,
    /// The GLB or its origin receipt could not be published.
    Failed(&'a ModelExportFailure),
}

enum Ended {
    Written(Box<ModelExportSummary>),
    Cancelled,
    Failed(ModelExportFailure),
}

#[derive(Debug)]
enum State {
    Running(Receiver<Ended>),
    Finished(Box<ModelExportSummary>),
    Cancelled,
    Failed(ModelExportFailure),
}

impl From<NoEntries> for Ended {
    fn from(error: NoEntries) -> Self {
        Self::Failed(error.into())
    }
}

impl std::fmt::Debug for Ended {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Ended").finish_non_exhaustive()
    }
}

/// An Export as GLB running on a background thread, or its outcome.
#[derive(Debug)]
pub struct ModelExport {
    id: u64,
    file: PathBuf,
    name: String,
    cancel: CancelToken,
    shared: Arc<Shared>,
    state: State,
}

impl ModelExport {
    /// Returns the number that tells this export apart from every other one of the process.
    #[must_use]
    pub const fn id(&self) -> u64 {
        self.id
    }

    /// Returns the GLB file the export writes.
    #[must_use]
    pub fn file(&self) -> &Path {
        &self.file
    }

    /// Returns the file name of the model that is exported.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns where the export stands.
    #[must_use]
    pub fn status(&self) -> ModelExportStatus<'_> {
        match &self.state {
            State::Running(_) => ModelExportStatus::Running(self.shared.progress()),
            State::Finished(summary) => ModelExportStatus::Finished(summary),
            State::Cancelled => ModelExportStatus::Cancelled,
            State::Failed(failure) => ModelExportStatus::Failed(failure),
        }
    }

    /// Returns whether the export still runs.
    #[must_use]
    pub const fn is_running(&self) -> bool {
        matches!(self.state, State::Running(_))
    }

    /// Returns whether the export was asked to stop.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.cancel.is_cancelled()
    }

    /// Asks the export to stop; the file is not written then.
    pub fn cancel(&self) {
        self.cancel.cancel();
    }

    /// Takes the outcome once the export ended; returns whether it ended.
    pub(crate) fn poll(&mut self) -> bool {
        let State::Running(result) = &self.state else {
            return false;
        };
        self.state = match result.try_recv() {
            Ok(ended) => settle(Some(ended)),
            Err(TryRecvError::Empty) => return false,
            Err(TryRecvError::Disconnected) => settle(None),
        };
        true
    }

    /// Cancels the export and waits until it ended, the partial file removed.
    pub(crate) fn stop(&mut self) {
        self.cancel.cancel();
        if let State::Running(result) = &self.state {
            self.state = settle(result.recv().ok());
        }
    }
}

fn settle(ended: Option<Ended>) -> State {
    match ended {
        Some(Ended::Written(summary)) => State::Finished(summary),
        Some(Ended::Cancelled) => State::Cancelled,
        Some(Ended::Failed(failure)) => State::Failed(failure),
        None => State::Failed(ModelExportFailure::Stopped),
    }
}

/// Starts exporting the model of `request` from `index` on a background thread and calls `wake`
/// from it once the export ended.
pub(crate) fn start(
    index: &Arc<EntryIndex>,
    request: Request,
    name: String,
    wake: impl Fn() + Send + 'static,
) -> ModelExport {
    static NEXT_ID: AtomicU64 = AtomicU64::new(0);
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let cancel = CancelToken::new();
    let shared = Arc::new(Shared::default());
    let file = request.file.clone();
    let (sender, result) = mpsc::channel();
    let thread_index = Arc::clone(index);
    let thread_cancel = cancel.clone();
    let thread_shared = Arc::clone(&shared);
    let spawned = thread::Builder::new()
        .name("model-export".to_owned())
        .spawn(move || {
            let ended = run(&thread_index, &request, &thread_cancel, &thread_shared);
            if sender.send(ended).is_ok() {
                wake();
            }
        });
    let state = match spawned {
        Ok(_handle) => State::Running(result),
        Err(error) => State::Failed(ModelExportFailure::Failed(describe(&error))),
    };
    ModelExport {
        id,
        file,
        name,
        cancel,
        shared,
        state,
    }
}

fn run(index: &EntryIndex, request: &Request, cancel: &CancelToken, shared: &Shared) -> Ended {
    let Some(paths) = index.path_index() else {
        return NoEntries.into();
    };
    let read = match read(index, &paths, request, cancel) {
        Ok(read) => read,
        Err(ended) => return ended,
    };
    write(index, &paths, request, &read, (cancel, shared))
}

/// The model and the animations an export reads before it writes the file.
struct Read {
    source: ModelSource,
    animations: AnimationSet,
}

fn read(
    index: &EntryIndex,
    paths: &PathIndex<'_>,
    request: &Request,
    cancel: &CancelToken,
) -> Result<Read, Ended> {
    if index.watch().check().is_err() {
        return Err(Ended::Failed(ModelExportFailure::ArchiveChanged));
    }
    let lods = LodSelection::Only(request.lod);
    let source = ModelSource::from_index_with(paths, &request.entry_path, lods)
        .map_err(|error| failed(index, &error))?;
    if cancel.is_cancelled() {
        return Err(Ended::Cancelled);
    }
    let animations = match &request.animations {
        Some(choice) => {
            AnimationSet::from_archive_cancellable(paths, &request.entry_path, choice, &[], cancel)
                .map_err(|error| match error {
                    ConversionError::Cancelled => Ended::Cancelled,
                    other => failed(index, &other),
                })?
        }
        None => AnimationSet::default(),
    };
    Ok(Read { source, animations })
}

fn write(
    index: &EntryIndex,
    paths: &PathIndex<'_>,
    request: &Request,
    read: &Read,
    (cancel, shared): (&CancelToken, &Shared),
) -> Ended {
    if index.watch().check().is_err() {
        return Ended::Failed(ModelExportFailure::ArchiveChanged);
    }
    shared.stage.store(1, Ordering::Relaxed);
    #[cfg(feature = "test-hooks")]
    request.pause.wait(|| cancel.is_cancelled());
    if cancel.is_cancelled() {
        return Ended::Cancelled;
    }
    let progress = |done: u64, total: u64| {
        shared.done.store(done, Ordering::Relaxed);
        shared.total.store(total, Ordering::Relaxed);
    };
    let textures = ArchiveTextures::new(paths);
    let loader = (request.materials == ModelMaterials::Textures).then_some(&textures);
    let publication = read.source.write_glb_with_geometry_receipt_observed(
        &read.animations,
        &request.options(),
        &request.file,
        loader.map(|textures| textures as &dyn TextureLoader),
        cancel,
        &progress,
    );
    match publication {
        Ok(publication) => Ended::Written(Box::new(ModelExportSummary::new(
            request.file.clone(),
            request.lod,
            read.source.model(),
            (publication, &read.animations),
            request.animations.is_some(),
        ))),
        Err(ConversionError::Cancelled) => Ended::Cancelled,
        Err(error) => failed(index, &error),
    }
}

fn failed(index: &EntryIndex, error: &ConversionError) -> Ended {
    if error.standalone_glb_was_committed() {
        return Ended::Failed(ModelExportFailure::Failed(describe(error)));
    }
    if index.watch().check().is_err() {
        return Ended::Failed(ModelExportFailure::ArchiveChanged);
    }
    if let Some(archive) = archive_error_in(error) {
        if archive.is_key_required() {
            return Ended::Failed(ModelExportFailure::KeyRequired);
        }
        if archive.is_wrong_key() {
            return Ended::Failed(ModelExportFailure::KeyRejected);
        }
    }
    Ended::Failed(match error {
        ConversionError::Installation(refused) => ModelExportFailure::Refused(refused.clone()),
        other => ModelExportFailure::Failed(describe(other)),
    })
}
