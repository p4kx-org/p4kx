// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

mod failure;
mod package;
mod run;
pub(super) mod stop;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;

pub(crate) use failure::failure_of;
use p4kx_assembly::LoadoutChanges;
use p4kx_extraction::{CancelToken, GlbOptions, Guid, MaterialMode};
use p4kx_settings::ModelMaterials;
pub(crate) use package::{info_of, package_failure};
#[cfg(test)]
use stop::Observed;

use super::{AssemblyExportForm, AssemblyExportSummary};
use crate::model::describe;
use crate::{EntryIndex, ModelExportFailure, NoEntries};

/// What an Export as GLB writes the scene of.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Subject {
    /// An Entity with the changes made to its Loadout, and whether its Interior comes with it.
    Entity {
        guid: Guid,
        changes: LoadoutChanges,
        interior: bool,
    },
    /// The Interior of a socpak container on its own: the Entry paths from the Archive down to
    /// it, each in the Archive or container before it.
    Container { chain: Vec<String> },
}

/// What an Export as GLB writes and where.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Request {
    pub(crate) subject: Subject,
    pub(crate) name: String,
    pub(crate) form: AssemblyExportForm,
    pub(crate) file: PathBuf,
    pub(crate) lod: u32,
    pub(crate) materials: ModelMaterials,
    pub(crate) texture_mip: u32,
    pub(crate) vertex_colours: bool,
    #[cfg(feature = "test-hooks")]
    pub(crate) pause: crate::hold::Pause,
}

impl Request {
    pub(super) fn options(&self) -> GlbOptions {
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
pub enum AssemblyExportStage {
    /// The Loadout of the Entity is resolved.
    Resolving,
    /// The models of the parts are read from the Archive.
    Reading,
    /// The containers of the Interior are read and their models placed.
    Interior,
    /// The GLB file is written.
    Writing,
}

/// How far a running Export as GLB has come.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AssemblyExportProgress {
    /// The step it is at.
    pub stage: AssemblyExportStage,
    /// While writing, the bytes of the file written so far; while reading the Interior, the model
    /// files read of the container being read.
    pub done: u64,
    /// While reading the Interior, the model files the container being read places, else 0.
    pub total: u64,
    /// While reading the Interior of an Entity, the containers done before the one being read.
    pub containers_done: u64,
    /// While reading the Interior of an Entity, the containers it names; 0 for a container on
    /// its own.
    pub containers_total: u64,
}

#[derive(Debug, Default)]
pub(super) struct Shared {
    stage: AtomicU8,
    done: AtomicU64,
    total: AtomicU64,
    containers_done: AtomicU64,
    containers_total: AtomicU64,
}

impl Shared {
    pub(super) fn progress(&self) -> AssemblyExportProgress {
        AssemblyExportProgress {
            stage: match self.stage.load(Ordering::Relaxed) {
                0 => AssemblyExportStage::Resolving,
                1 => AssemblyExportStage::Reading,
                2 => AssemblyExportStage::Interior,
                _ => AssemblyExportStage::Writing,
            },
            done: self.done.load(Ordering::Relaxed),
            total: self.total.load(Ordering::Relaxed),
            containers_done: self.containers_done.load(Ordering::Relaxed),
            containers_total: self.containers_total.load(Ordering::Relaxed),
        }
    }

    fn enter(&self, stage: AssemblyExportStage) {
        let code = match stage {
            AssemblyExportStage::Resolving => 0,
            AssemblyExportStage::Reading => 1,
            AssemblyExportStage::Interior => 2,
            AssemblyExportStage::Writing => 3,
        };
        self.stage.store(code, Ordering::Relaxed);
        self.done.store(0, Ordering::Relaxed);
        self.total.store(0, Ordering::Relaxed);
        self.containers_done.store(0, Ordering::Relaxed);
        self.containers_total.store(0, Ordering::Relaxed);
    }

    fn containers(&self, done: usize, total: usize) {
        let count = |value: usize| u64::try_from(value).unwrap_or(u64::MAX);
        self.containers_done.store(count(done), Ordering::Relaxed);
        self.containers_total.store(count(total), Ordering::Relaxed);
        self.models(0, 0);
    }

    fn models(&self, done: usize, total: usize) {
        let count = |value: usize| u64::try_from(value).unwrap_or(u64::MAX);
        self.done.store(count(done), Ordering::Relaxed);
        self.total.store(count(total), Ordering::Relaxed);
    }
}

/// Where an Export of an Entity as GLB stands.
#[derive(Debug, Clone, Copy)]
pub enum AssemblyExportStatus<'a> {
    /// The export runs; the progress so far.
    Running(AssemblyExportProgress),
    /// The file was written.
    Finished(&'a AssemblyExportSummary),
    /// The export was cancelled; no file was written.
    Cancelled,
    /// No file was written, for the reason given.
    Failed(&'a ModelExportFailure),
}

enum Ended {
    Written(Box<AssemblyExportSummary>),
    Cancelled,
    Failed(ModelExportFailure),
}

#[derive(Debug)]
enum State {
    Running(Receiver<Ended>),
    Finished(Box<AssemblyExportSummary>),
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

/// An Export of an Entity as GLB running on a background thread, or its outcome.
#[derive(Debug)]
pub struct AssemblyExport {
    id: u64,
    file: PathBuf,
    name: String,
    container: bool,
    form: AssemblyExportForm,
    cancel: CancelToken,
    shared: Arc<Shared>,
    state: State,
}

impl AssemblyExport {
    /// Returns what the export writes: a GLB file or a Blender package folder.
    #[must_use]
    pub const fn form(&self) -> AssemblyExportForm {
        self.form
    }

    /// Returns whether the export writes the Interior of a container on its own rather than an
    /// Entity.
    #[must_use]
    pub const fn is_container(&self) -> bool {
        self.container
    }

    /// Returns the number that tells this export apart from every other one of the process.
    #[must_use]
    pub const fn id(&self) -> u64 {
        self.id
    }

    /// Returns the GLB file the export writes, or the package folder.
    #[must_use]
    pub fn file(&self) -> &Path {
        &self.file
    }

    /// Returns the class name of the Entity that is exported, or the name of the container.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns where the export stands.
    #[must_use]
    pub fn status(&self) -> AssemblyExportStatus<'_> {
        match &self.state {
            State::Running(_) => AssemblyExportStatus::Running(self.shared.progress()),
            State::Finished(summary) => AssemblyExportStatus::Finished(summary),
            State::Cancelled => AssemblyExportStatus::Cancelled,
            State::Failed(failure) => AssemblyExportStatus::Failed(failure),
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

/// Starts exporting the Entity of `request` from `index` on a background thread and calls `wake`
/// from it once the export ended.
pub(crate) fn start(
    index: &Arc<EntryIndex>,
    request: Request,
    wake: impl Fn() + Send + 'static,
) -> AssemblyExport {
    static NEXT_ID: AtomicU64 = AtomicU64::new(0);
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let cancel = CancelToken::new();
    let shared = Arc::new(Shared::default());
    let (file, name) = (request.file.clone(), request.name.clone());
    let container = matches!(request.subject, Subject::Container { .. });
    let form = request.form;
    let (sender, result) = mpsc::channel();
    let thread_index = Arc::clone(index);
    let thread_cancel = cancel.clone();
    let thread_shared = Arc::clone(&shared);
    let spawned = thread::Builder::new()
        .name("assembly-export".to_owned())
        .spawn(move || {
            let ended = run::run(&thread_index, &request, &thread_cancel, &thread_shared);
            if sender.send(ended).is_ok() {
                wake();
            }
        });
    let state = match spawned {
        Ok(_handle) => State::Running(result),
        Err(error) => State::Failed(ModelExportFailure::Failed(describe(&error))),
    };
    AssemblyExport {
        id,
        file,
        name,
        container,
        form,
        cancel,
        shared,
        state,
    }
}

#[cfg(test)]
impl AssemblyExport {
    /// Returns an export that stays running until the returned guard is dropped, in `stage`.
    pub(crate) fn pending(
        (name, form): (&str, AssemblyExportForm),
        stage: AssemblyExportStage,
    ) -> (Self, Box<dyn std::any::Any>) {
        let (sender, result) = mpsc::channel::<Ended>();
        let shared = Arc::new(Shared::default());
        shared.enter(stage);
        let export = Self {
            id: u64::MAX,
            file: PathBuf::from(name),
            name: name.to_owned(),
            container: false,
            form,
            cancel: CancelToken::new(),
            shared,
            state: State::Running(result),
        };
        (export, Box::new(sender))
    }
}

#[cfg(test)]
mod tests;
