// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;
use std::sync::atomic::{AtomicU8, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;

use p4kx_assembly::{
    ArchiveModels, Assembly, AssemblyError, AssemblyOptions, CachedModels, EntityInterior,
    EntityNode, ForgePalettes, InteriorForge, InteriorOptions, InteriorProgress, InteriorWatch,
    ModelCache, ModelFailure, ModelProvider, ProvidedModel,
};
use p4kx_extraction::model::{LayerBudget, MaterialStatus};
use p4kx_extraction::{CancelToken, ConversionError, DataForge};
use p4kx_settings::ModelMaterials;

use crate::assembly_export::{Stoppable, StoppableTextures, failure_of};
use crate::forge_cache::ForgeSource;
use crate::model::describe;
use crate::{
    AssemblyScene, BuildError, BuildOptions, EntryIndex, Hooks, ModelExportFailure, NoEntries,
    Progress,
};

/// The step a building view is at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewStage {
    /// The models of the parts are read from the Archive.
    Reading,
    /// The models are flattened into shared meshes.
    Flattening,
    /// The textures of the materials are read.
    Textures,
    /// The containers of the Interior are read; `done` and `total` count the containers.
    Interior,
}

/// How far a building view has come.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ViewProgress {
    /// The step it is at.
    pub stage: ViewStage,
    /// What is done of the step.
    pub done: u64,
    /// What the step has to do, 0 while unknown.
    pub total: u64,
    /// The model files read of those the container being read places, in the Interior step.
    pub models_done: u64,
    /// The model files the container being read places, in the Interior step.
    pub models_total: u64,
}

#[derive(Debug, Default)]
pub(super) struct Shared {
    stage: AtomicU8,
    done: AtomicU64,
    total: AtomicU64,
    models_done: AtomicU64,
    models_total: AtomicU64,
}

impl Shared {
    fn progress(&self) -> ViewProgress {
        ViewProgress {
            stage: match self.stage.load(Ordering::Relaxed) {
                0 => ViewStage::Reading,
                1 => ViewStage::Flattening,
                2 => ViewStage::Textures,
                _ => ViewStage::Interior,
            },
            done: self.done.load(Ordering::Relaxed),
            total: self.total.load(Ordering::Relaxed),
            models_done: self.models_done.load(Ordering::Relaxed),
            models_total: self.models_total.load(Ordering::Relaxed),
        }
    }

    fn set(&self, stage: u8, done: u64, total: u64) {
        self.stage.store(stage, Ordering::Relaxed);
        self.done.store(done, Ordering::Relaxed);
        self.total.store(total, Ordering::Relaxed);
    }

    fn interior(&self, progress: InteriorProgress) {
        let count = |value: usize| u64::try_from(value).unwrap_or(u64::MAX);
        match progress {
            InteriorProgress::Container { done, total } => {
                self.set(3, count(done) + 1, count(total));
                self.models_done.store(0, Ordering::Relaxed);
                self.models_total.store(0, Ordering::Relaxed);
            }
            InteriorProgress::Models { done, total } => {
                self.models_done.store(count(done), Ordering::Relaxed);
                self.models_total.store(count(total), Ordering::Relaxed);
            }
            _ => {}
        }
    }
}

/// What the view is built from.
pub(super) struct Request {
    pub(super) tree: Arc<EntityNode>,
    pub(super) lod: u32,
    pub(super) materials: ModelMaterials,
    pub(super) interior: bool,
    pub(super) models: Arc<ModelCache>,
    pub(super) interior_models: Arc<ModelCache>,
    #[cfg(feature = "test-hooks")]
    pub(super) gate: Option<Arc<crate::hold::Gate>>,
}

/// A built view and the number of parts the Assembly could not resolve, those of the Loadout and
/// those of the Interior apart.
pub(super) struct Built {
    pub(super) scene: AssemblyScene,
    pub(super) issues: usize,
    pub(super) interior_issues: usize,
    pub(super) interior: bool,
}

pub(super) enum Ended {
    Built(Box<Built>),
    Cancelled,
    Empty,
    Failed(ModelExportFailure),
}

/// A view being built on a background thread; dropping it stops the work.
pub(super) struct Job {
    result: Receiver<Ended>,
    cancel: CancelToken,
    shared: Arc<Shared>,
}

impl Job {
    pub(super) fn progress(&self) -> ViewProgress {
        self.shared.progress()
    }

    pub(super) fn cancel(&self) {
        self.cancel.cancel();
    }

    pub(super) fn poll(&self) -> Option<Ended> {
        match self.result.try_recv() {
            Ok(ended) => Some(ended),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => Some(Ended::Failed(ModelExportFailure::Stopped)),
        }
    }
}

impl Drop for Job {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}

struct Counting<'a> {
    inner: &'a dyn ModelProvider,
    shared: &'a Shared,
    cancel: &'a CancelToken,
}

impl ModelProvider for Counting<'_> {
    fn model(&self, path: &str, lod: u32) -> Result<ProvidedModel, ModelFailure> {
        if self.cancel.is_cancelled() {
            return Err(ModelFailure::Fatal(Box::new(ConversionError::Cancelled)));
        }
        let model = self.inner.model(path, lod);
        let done = self.shared.done.fetch_add(1, Ordering::Relaxed) + 1;
        self.shared.total.store(done, Ordering::Relaxed);
        model
    }

    fn library(&self, entry: &str, name: &str) -> MaterialStatus {
        self.inner.library(entry, name)
    }

    fn layer_budget(&self) -> LayerBudget {
        self.inner.layer_budget()
    }

    fn layer_library(&self, path: &str, budget: &mut LayerBudget) -> Arc<MaterialStatus> {
        self.inner.layer_library(path, budget)
    }
}

/// Starts building the view of `request` from `index`, calling `wake` from the thread once it
/// ended.
pub(super) fn start(
    index: &Arc<EntryIndex>,
    request: Request,
    wake: Arc<dyn Fn() + Send + Sync>,
) -> Result<Job, String> {
    let (sender, result) = mpsc::channel();
    let cancel = CancelToken::new();
    let shared = Arc::new(Shared::default());
    let thread_index = Arc::clone(index);
    let (thread_cancel, thread_shared) = (cancel.clone(), Arc::clone(&shared));
    thread::Builder::new()
        .name("assembly-view".to_owned())
        .spawn(move || {
            let ended = run(&thread_index, &request, &thread_cancel, &thread_shared);
            if sender.send(ended).is_ok() {
                wake();
            }
        })
        .map_err(|error| describe(&error))?;
    Ok(Job {
        result,
        cancel,
        shared,
    })
}

fn scene_of(
    assembly: &Assembly,
    (models, materials): (&ArchiveModels<'_>, ModelMaterials),
    cancel: &CancelToken,
    shared: &Shared,
) -> Result<AssemblyScene, BuildError> {
    let archive_textures = models.textures();
    let textures = StoppableTextures {
        textures: &archive_textures,
        cancel,
    };
    let count = |value: usize| u64::try_from(value).unwrap_or(u64::MAX);
    AssemblyScene::build(
        assembly,
        &BuildOptions {
            materials,
            textures: Some(&textures),
        },
        &Hooks {
            cancelled: &|| cancel.is_cancelled(),
            progress: &|progress| match progress {
                Progress::Flattening { done, total } => shared.set(1, count(done), count(total)),
                Progress::Textures { done, total } => shared.set(2, count(done), count(total)),
            },
        },
    )
}

/// Adds the Interior of `request` to `assembly` when it asks for one.
fn with_interior(
    index: &EntryIndex,
    assembly: Assembly,
    request: &Request,
    (cancel, shared): (&CancelToken, &Shared),
    (forge, models): (&DataForge<'_>, &dyn ModelProvider),
) -> Result<Assembly, Ended> {
    if !request.interior {
        return Ok(assembly);
    }
    shared.set(3, 0, 0);
    #[cfg(feature = "test-hooks")]
    if let Some(gate) = &request.gate {
        gate.wait(|| cancel.is_cancelled());
    }
    let watch = InteriorWatch {
        cancelled: &|| cancel.is_cancelled(),
        progress: &|progress| shared.interior(progress),
    };
    let archive = index
        .entries_archive()
        .map_err(|error| Ended::Failed(error.into()))?;
    let options = InteriorOptions::default().with_lod(request.lod);
    let classes = InteriorForge::new(forge);
    match EntityInterior::build_with_models(
        archive,
        &assembly,
        &request.tree,
        &options,
        &watch,
        (Some(&classes), Some(models)),
    ) {
        Ok(interior) => Ok(assembly.with_interior(interior)),
        Err(AssemblyError::Cancelled) => Err(Ended::Cancelled),
        Err(_) if cancel.is_cancelled() => Err(Ended::Cancelled),
        Err(AssemblyError::Provider(error)) => Err(Ended::Failed(failure_of(index, &*error))),
        Err(other) => Err(Ended::Failed(failure_of(index, &other))),
    }
}

/// Returns the DataForge of `index`, or why the build ends without it.
fn read_forge(index: &EntryIndex) -> Result<Arc<ForgeSource>, Ended> {
    if index.watch().check().is_err() {
        return Err(Ended::Failed(ModelExportFailure::ArchiveChanged));
    }
    index
        .forge()
        .read(index, || {})
        .map_err(|failure| Ended::Failed(failure.into_export()))
}

fn run(index: &EntryIndex, request: &Request, cancel: &CancelToken, shared: &Shared) -> Ended {
    let source = match read_forge(index) {
        Ok(source) => source,
        Err(ended) => return ended,
    };
    let forge = match source.entry.open() {
        Ok(forge) => forge,
        Err(error) => return Ended::Failed(ModelExportFailure::Failed(describe(&error))),
    };
    let palettes = ForgePalettes::new(&forge);
    let Ok(archive) = index.entries_archive() else {
        return Ended::Failed(NoEntries.into());
    };
    let archive_models = ArchiveModels::new(archive);
    let stoppable = Stoppable {
        models: &archive_models,
        cancel,
    };
    let cached = CachedModels::new(&stoppable, &request.models);
    let models = Counting {
        inner: &cached,
        shared,
        cancel,
    };
    let options = AssemblyOptions::default().with_lod(request.lod);
    let assembly = match Assembly::build_with_palettes(&request.tree, &models, &palettes, &options)
    {
        Ok(assembly) => assembly,
        Err(_) if cancel.is_cancelled() => return Ended::Cancelled,
        Err(AssemblyError::Provider(error)) => {
            return Ended::Failed(failure_of(index, &*error));
        }
        Err(other) => return Ended::Failed(failure_of(index, &other)),
    };
    let interior_cached = CachedModels::new(&stoppable, &request.interior_models);
    let with = (&forge, &interior_cached as &dyn ModelProvider);
    let assembly = match with_interior(index, assembly, request, (cancel, shared), with) {
        Ok(assembly) => assembly,
        Err(ended) => return ended,
    };
    let built = scene_of(
        &assembly,
        (&archive_models, request.materials),
        cancel,
        shared,
    );
    if index.watch().check().is_err() {
        return Ended::Failed(ModelExportFailure::ArchiveChanged);
    }
    match built {
        Ok(scene) => Ended::Built(Box::new(built_of(scene, &assembly, request.interior))),
        Err(BuildError::Cancelled) => Ended::Cancelled,
        Err(BuildError::Empty) => Ended::Empty,
    }
}

fn built_of(scene: AssemblyScene, assembly: &Assembly, interior: bool) -> Built {
    let interior_issues = assembly
        .interior()
        .map_or(0, |interior| interior.issues().len());
    Built {
        scene,
        issues: assembly.issues().len().saturating_sub(interior_issues),
        interior_issues,
        interior,
    }
}
