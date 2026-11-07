// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use p4kx_assembly::{ArchiveModels, Armour};
use p4kx_blender::PackageSummary;
use p4kx_extraction::{
    ArchiveWriteProtection, CancelToken, ConversionError, GlbOptions, GlbSummary, GltfError,
    MaterialMode, write_replacing_protected,
};
use p4kx_settings::ModelMaterials;

use super::package;
use super::task::{Polled, Task};
use crate::assembly_export::{Observed, PartIssue, StoppableTextures, failure_of};
use crate::{AssemblyExportForm, EntryIndex, ModelExportFailure, NoEntries};

/// Where the Character comes from, as the manifest of a package records it.
#[derive(Debug, Clone, Default)]
pub(super) struct Origin {
    pub(super) chf: Option<String>,
    pub(super) loadout: Option<String>,
    pub(super) npc_folder: Option<String>,
}

/// What the Export of a Character writes and where.
#[derive(Debug, Clone)]
pub(super) struct Request {
    pub(super) armour: Arc<Armour>,
    pub(super) name: String,
    pub(super) form: AssemblyExportForm,
    pub(super) file: PathBuf,
    pub(super) materials: ModelMaterials,
    pub(super) texture_mip: u32,
    pub(super) vertex_colours: bool,
    pub(super) origin: Origin,
    pub(super) animations: bool,
    #[cfg(feature = "test-hooks")]
    pub(super) pause: crate::hold::Pause,
}

impl Request {
    pub(super) fn options(&self) -> GlbOptions {
        let mode = match self.materials {
            ModelMaterials::Colours => MaterialMode::Colours,
            ModelMaterials::Textures => MaterialMode::Textures,
        };
        let options = GlbOptions::default()
            .with_lod(self.armour.lod())
            .with_materials(mode)
            .with_texture_mip(self.texture_mip);
        if self.vertex_colours {
            options
        } else {
            options.without_vertex_colours()
        }
    }
}

/// What a finished Export of a Character wrote and what it left out.
#[derive(Debug, Clone, PartialEq)]
pub struct CharacterExportSummary {
    pub(super) file: PathBuf,
    pub(super) name: String,
    pub(super) parts: usize,
    pub(super) lod: u32,
    pub(super) written: GlbSummary,
    pub(super) issues: Vec<PartIssue>,
    pub(super) package: Option<PackageSummary>,
}

impl CharacterExportSummary {
    /// Returns what the Blender package holds, or `None` when the export wrote a GLB file.
    #[must_use]
    pub const fn package(&self) -> Option<&PackageSummary> {
        self.package.as_ref()
    }

    /// Returns the GLB file or the package folder that was written.
    #[must_use]
    pub fn file(&self) -> &Path {
        &self.file
    }

    /// Returns the name of the Character.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns how many parts the file holds.
    #[must_use]
    pub const fn parts(&self) -> usize {
        self.parts
    }

    /// Returns the level of detail that was exported.
    #[must_use]
    pub const fn lod(&self) -> u32 {
        self.lod
    }

    /// Returns what the writer counted.
    #[must_use]
    pub const fn written(&self) -> &GlbSummary {
        &self.written
    }

    /// Returns what could not be done in full, in the order of the pieces.
    #[must_use]
    pub fn issues(&self) -> &[PartIssue] {
        &self.issues
    }

    /// Returns how many pieces the report lists as left out of the file.
    #[must_use]
    pub fn skipped(&self) -> usize {
        self.issues.iter().filter(|issue| issue.skipped).count()
    }
}

/// Where an Export of a Character stands.
#[derive(Debug, Clone, Copy)]
pub enum CharacterExportStatus<'a> {
    /// The export runs; the bytes written so far, always 0 for a package.
    Running(u64),
    /// The file or package was written.
    Finished(&'a CharacterExportSummary),
    /// The export was cancelled; nothing was written.
    Cancelled,
    /// Nothing was written, for the reason given.
    Failed(&'a ModelExportFailure),
}

pub(super) enum Ended {
    Written(Box<CharacterExportSummary>),
    Cancelled,
    Failed(ModelExportFailure),
}

impl From<NoEntries> for Ended {
    fn from(error: NoEntries) -> Self {
        Self::Failed(error.into())
    }
}

#[derive(Debug)]
enum State {
    Running,
    Finished(Box<CharacterExportSummary>),
    Cancelled,
    Failed(ModelExportFailure),
}

/// An Export of a Character running on a background thread, or its outcome.
pub struct CharacterExport {
    id: u64,
    file: PathBuf,
    name: String,
    form: AssemblyExportForm,
    written: Arc<AtomicU64>,
    task: Option<Task<Ended>>,
    state: State,
}

impl std::fmt::Debug for CharacterExport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CharacterExport")
            .field("id", &self.id)
            .field("file", &self.file)
            .field("state", &self.state)
            .finish_non_exhaustive()
    }
}

impl CharacterExport {
    /// Returns the number that tells this export apart from every other one of the process.
    #[must_use]
    pub const fn id(&self) -> u64 {
        self.id
    }

    /// Returns what the export writes: a GLB file or a Blender package folder.
    #[must_use]
    pub const fn form(&self) -> AssemblyExportForm {
        self.form
    }

    /// Returns the GLB file the export writes, or the package folder.
    #[must_use]
    pub fn file(&self) -> &Path {
        &self.file
    }

    /// Returns the name of the Character that is exported.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns where the export stands.
    #[must_use]
    pub fn status(&self) -> CharacterExportStatus<'_> {
        match &self.state {
            State::Running => CharacterExportStatus::Running(self.written.load(Ordering::Relaxed)),
            State::Finished(summary) => CharacterExportStatus::Finished(summary),
            State::Cancelled => CharacterExportStatus::Cancelled,
            State::Failed(failure) => CharacterExportStatus::Failed(failure),
        }
    }

    /// Returns whether the export still runs.
    #[must_use]
    pub const fn is_running(&self) -> bool {
        matches!(self.state, State::Running)
    }

    /// Returns whether the export was asked to stop.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.task.as_ref().is_some_and(Task::is_cancelled)
    }

    /// Asks the export to stop; the file is not written then.
    pub fn cancel(&self) {
        if let Some(task) = &self.task {
            task.cancel();
        }
    }

    pub(super) fn poll(&mut self) -> bool {
        let Some(task) = &self.task else {
            return false;
        };
        let ended = match task.poll() {
            Polled::Pending => return false,
            Polled::Ready(ended) => Some(ended),
            Polled::Lost => None,
        };
        self.settle(ended);
        true
    }

    pub(super) fn stop(&mut self) {
        let Some(task) = &self.task else {
            return;
        };
        task.cancel();
        let ended = task.wait();
        self.settle(ended);
    }

    fn settle(&mut self, ended: Option<Ended>) {
        self.task = None;
        self.state = match ended {
            Some(Ended::Written(summary)) => State::Finished(summary),
            Some(Ended::Cancelled) => State::Cancelled,
            Some(Ended::Failed(failure)) => State::Failed(failure),
            None => State::Failed(ModelExportFailure::Stopped),
        };
    }
}

fn next_id() -> u64 {
    static NEXT_ID: AtomicU64 = AtomicU64::new(0);
    NEXT_ID.fetch_add(1, Ordering::Relaxed)
}

pub(super) fn start(
    index: &Arc<EntryIndex>,
    request: Request,
    wake: Arc<dyn Fn() + Send + Sync>,
) -> CharacterExport {
    let id = next_id();
    let written = Arc::new(AtomicU64::new(0));
    let (file, name) = (request.file.clone(), request.name.clone());
    let form = request.form;
    let thread_index = Arc::clone(index);
    let thread_written = Arc::clone(&written);
    let spawned = Task::spawn("character-export", wake, move |cancel| {
        run(&thread_index, &request, cancel, &thread_written)
    });
    let (task, state) = match spawned {
        Ok(task) => (Some(task), State::Running),
        Err(reason) => (None, State::Failed(ModelExportFailure::Failed(reason))),
    };
    CharacterExport {
        id,
        file,
        name,
        form,
        written,
        task,
        state,
    }
}

fn run(index: &EntryIndex, request: &Request, cancel: &CancelToken, written: &AtomicU64) -> Ended {
    if index.watch().check().is_err() {
        return Ended::Failed(ModelExportFailure::ArchiveChanged);
    }
    #[cfg(feature = "test-hooks")]
    request.pause.wait(|| cancel.is_cancelled());
    if request.form == AssemblyExportForm::Package {
        return package::write(index, request, cancel);
    }
    let Ok(archive) = index.entries_archive() else {
        return NoEntries.into();
    };
    let models = ArchiveModels::new(archive);
    let archive_textures = models.textures();
    let textures = StoppableTextures {
        textures: &archive_textures,
        cancel,
    };
    let options = request.options();
    let protection = match ArchiveWriteProtection::new(index.path(), cancel.flag()) {
        Ok(protection) => protection,
        Err(error) if error.is_cancelled() => return Ended::Cancelled,
        Err(error) => return Ended::Failed(ModelExportFailure::Refused(error)),
    };
    let outcome =
        write_replacing_protected(&request.file, Some(&protection), Some(cancel), |writer| {
            let mut observed = Observed {
                inner: writer,
                cancel,
                written,
            };
            request
                .armour
                .write_glb_with(&options, &textures, &mut observed)
                .map_err(|error| match error {
                    GltfError::Write(_) if cancel.is_cancelled() => ConversionError::Cancelled,
                    GltfError::Write(source) => ConversionError::Write {
                        path: request.file.clone(),
                        source,
                    },
                    source => ConversionError::Glb {
                        file: request.name.clone(),
                        source,
                    },
                })
        });
    match outcome {
        Ok(summary) => Ended::Written(Box::new(CharacterExportSummary {
            file: request.file.clone(),
            name: request.name.clone(),
            parts: request.armour.parts().len(),
            lod: request.armour.lod(),
            written: summary,
            issues: request.armour.issues().iter().map(PartIssue::of).collect(),
            package: None,
        })),
        Err(ConversionError::Cancelled) => Ended::Cancelled,
        Err(error) => Ended::Failed(failure_of(index, &error)),
    }
}
