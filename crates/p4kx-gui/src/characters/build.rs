// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;
use std::sync::atomic::{AtomicU8, AtomicU64, Ordering};

use p4kx_assembly::character::Character;
use p4kx_assembly::{
    ArchiveFiles, ArchiveModels, Armour, AssemblyError, AssemblyOptions, BodyVariant, Expressions,
    ForgeItems, ForgePalettes, Head, LoadoutResolver, ModelFailure, ModelProvider, Piece,
    ProvidedModel, Replaced,
};
use p4kx_extraction::DataForge;
use p4kx_extraction::model::{LayerBudget, MaterialStatus};
use p4kx_extraction::{CancelToken, Guid};

use crate::assembly_export::{Stoppable, failure_of};
use crate::forge_cache::ForgeSource;
use crate::model::describe;
use crate::{
    EntryIndex, ModelExportFailure, MorphSpan, NoEntries, Scene, SceneError, ViewProgress,
    ViewStage,
};

/// What the Armour of the Character builder is built from.
#[derive(Debug, Clone)]
pub(super) struct Request {
    pub(super) variant: BodyVariant,
    pub(super) character: Option<Arc<Character>>,
    pub(super) pieces: Vec<Guid>,
    pub(super) loadout: Option<String>,
    pub(super) lod: u32,
    pub(super) expressions: bool,
}

/// A built Armour with the scene that shows it.
pub(super) struct Built {
    pub(super) armour: Armour,
    pub(super) scene: Scene,
    pub(super) spans: Vec<Option<MorphSpan>>,
    pub(super) replaced: Vec<Replaced>,
    pub(super) npc_folder: Option<String>,
}

pub(super) enum Ended {
    Built(Box<Built>),
    Cancelled,
    Empty,
    Failed(ModelExportFailure),
}

impl From<NoEntries> for Ended {
    fn from(error: NoEntries) -> Self {
        Self::Failed(error.into())
    }
}

#[derive(Debug, Default)]
pub(super) struct Shared {
    stage: AtomicU8,
    done: AtomicU64,
}

impl Shared {
    pub(super) fn progress(&self) -> ViewProgress {
        let reading = self.stage.load(Ordering::Relaxed) == 0;
        ViewProgress {
            stage: if reading {
                ViewStage::Reading
            } else {
                ViewStage::Flattening
            },
            done: self.done.load(Ordering::Relaxed),
            total: 0,
            models_done: 0,
            models_total: 0,
        }
    }
}

struct Counting<'a> {
    inner: &'a dyn ModelProvider,
    shared: &'a Shared,
}

impl ModelProvider for Counting<'_> {
    fn model(&self, path: &str, lod: u32) -> Result<ProvidedModel, ModelFailure> {
        let model = self.inner.model(path, lod);
        self.shared.done.fetch_add(1, Ordering::Relaxed);
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

    fn file(&self, path: &str, limit: u64) -> Result<Vec<u8>, ModelFailure> {
        self.inner.file(path, limit)
    }
}

fn failed(reason: impl Into<String>) -> Ended {
    Ended::Failed(ModelExportFailure::Failed(reason.into()))
}

/// What a Loadout and the given pieces make: the pieces to wear, the pieces of the Loadout that were
/// replaced, and the folder of the head the Loadout gives its character.
type Worn = (Vec<Piece>, Vec<Replaced>, Option<String>);

/// Returns the pieces to wear, the Loadout's with the given ones on top, and the head's.
fn worn(
    forge: &DataForge<'_>,
    files: &ArchiveFiles<'_>,
    request: &Request,
) -> Result<Worn, String> {
    let mut resolver = LoadoutResolver::new(forge).with_files(files);
    let given: Vec<Piece> = request
        .pieces
        .iter()
        .map(|guid| {
            forge.record_by_guid(*guid).map_or_else(
                || Piece::failed(guid.to_string(), "the DataForge has no such Record"),
                |record| match resolver.wearable(&record) {
                    Ok(wearable) => Piece::wearable(wearable),
                    Err(error) => Piece::failed(guid.to_string(), describe(&error)),
                },
            )
        })
        .collect();
    let (mut pieces, replaced, npc_folder) = match &request.loadout {
        Some(path) => {
            let tree = resolver
                .resolve_file(path)
                .map_err(|error| describe(&error))?;
            let folder = resolver
                .npc_head(&tree)
                .and_then(|head| head.folder().map(str::to_owned));
            let (pieces, replaced) = Piece::layer(resolver.worn(&tree), given);
            (pieces, replaced, folder)
        }
        None => (given, Vec::new(), None),
    };
    if let Some(character) = &request.character {
        let head = Head::dressed(character, &mut ForgeItems::new(&mut resolver))
            .map_err(|error| describe(&error))?;
        let head = if request.expressions {
            head.with_expressions(&Expressions::All)
        } else {
            head
        };
        pieces.extend(head.pieces().iter().cloned());
    }
    Ok((pieces, replaced, npc_folder))
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

/// Builds the Armour of `request` from `index` and flattens it into a scene.
pub(super) fn run(
    index: &EntryIndex,
    request: &Request,
    cancel: &CancelToken,
    shared: &Shared,
) -> Ended {
    let source = match read_forge(index) {
        Ok(source) => source,
        Err(ended) => return ended,
    };
    let forge = match source.entry.open() {
        Ok(forge) => forge,
        Err(error) => return failed(describe(&error)),
    };
    let Ok(archive) = index.entries_archive() else {
        return NoEntries.into();
    };
    let files = ArchiveFiles::new(archive);
    let (pieces, replaced, npc_folder) = match worn(&forge, &files, request) {
        Ok(worn) => worn,
        Err(reason) => return failed(reason),
    };
    if cancel.is_cancelled() {
        return Ended::Cancelled;
    }
    let archive_models = ArchiveModels::new(archive);
    let stoppable = Stoppable {
        models: &archive_models,
        cancel,
    };
    let models = Counting {
        inner: &stoppable,
        shared,
    };
    let options = AssemblyOptions::default().with_lod(request.lod);
    let palettes = ForgePalettes::new(&forge);
    let built = Armour::build_with_palettes(request.variant, &pieces, &models, &palettes, &options);
    let armour = match built {
        Ok(armour) => armour,
        Err(_) if cancel.is_cancelled() => return Ended::Cancelled,
        Err(AssemblyError::Provider(error)) => return Ended::Failed(failure_of(index, &*error)),
        Err(other) => return Ended::Failed(failure_of(index, &other)),
    };
    shared.stage.store(1, Ordering::Relaxed);
    if cancel.is_cancelled() {
        return Ended::Cancelled;
    }
    if index.watch().check().is_err() {
        return Ended::Failed(ModelExportFailure::ArchiveChanged);
    }
    match Scene::from_armour(&armour) {
        Ok((scene, spans)) => Ended::Built(Box::new(Built {
            armour,
            scene,
            spans,
            replaced,
            npc_folder,
        })),
        Err(SceneError::Empty) => Ended::Empty,
        Err(error) => failed(error.to_string()),
    }
}
