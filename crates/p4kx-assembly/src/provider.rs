// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::io::Read;
use std::sync::Arc;

use p4kx_extraction::model::{
    LayerBudget, LayerCache, LodSelection, MaterialReading, MaterialStatus, Model, ModelError,
    PositionBox,
};
use p4kx_extraction::{
    AnimationChoice, AnimationSet, Archive, ArchiveTextures, ConversionError, ModelSource,
    PathIndex, TextureVerdicts, archive_error_in,
};
use thiserror::Error;

use crate::MAX_ASSEMBLY_VERTICES;
use crate::cdf::is_character_definition;
use crate::path_match::starts_with_path;
use crate::skeleton::SkeletonProblem;

mod character;

/// A model a [`ModelProvider`] read and the Entry it was read from.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct ProvidedModel {
    /// The path of the model's file in the source, which companions and bare material library
    /// names are looked up beside.
    pub entry: String,
    /// The model, read without its material library, see [`MaterialReading::Skipped`].
    pub model: Model,
    /// The files of the attachments of a character definition (`.cdf`) that are neither `model`
    /// nor in `attachments`; empty for every other file.
    pub left_out: Vec<String>,
    /// The bone attachments of a character definition (`.cdf`), each a model bound to a bone of
    /// `model`'s skeleton; empty for every other file.
    pub attachments: Vec<ProvidedAttachment>,
    /// What is wrong with the skeleton file a character definition (`.cdf`) names; `None` when
    /// the file is fine, the definition names none, or the file is no character definition.
    pub skeleton: Option<SkeletonProblem>,
}

/// A rigid model a character definition binds to a bone of its skeleton.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct ProvidedAttachment {
    /// The attachment's name.
    pub name: String,
    /// The bone of the skeleton the model is bound to.
    pub bone: String,
    /// The offset from the bone in metres.
    pub position: [f32; 3],
    /// The rotation from the bone as a quaternion `[w, x, y, z]`.
    pub rotation: [f32; 4],
    /// The path of the model's file in the source.
    pub entry: String,
    /// The model, read without its material library.
    pub model: Arc<Model>,
}

/// Why a [`ModelProvider`] cannot supply a model.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ModelFailure {
    /// There is no such file.
    #[error("there is no such file")]
    Missing,
    /// The file or one of its companions cannot be read as a model.
    #[error("{reason}")]
    Unreadable {
        /// What is wrong.
        reason: String,
    },
    /// The source fails for every model, e.g. an Archive key is missing; building the Assembly
    /// stops with this failure instead of listing every part as unreadable.
    #[error("the models cannot be read")]
    Fatal(#[source] Box<dyn Error + Send + Sync>),
}

impl ModelFailure {
    /// Returns a stable code for the kind of failure, `missing`, `unreadable` or `fatal`.
    #[must_use]
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::Missing => "missing",
            Self::Unreadable { .. } => "unreadable",
            Self::Fatal(_) => "fatal",
        }
    }
}

/// Returns `error` and its causes as one line.
fn describe(error: &(dyn Error + 'static)) -> String {
    let mut text = error.to_string();
    let mut cause = error.source();
    while let Some(next) = cause {
        text.push_str(": ");
        text.push_str(&next.to_string());
        cause = next.source();
    }
    text
}

fn read_failure<E: Error + Send + Sync + 'static>(error: E) -> ModelFailure {
    let key_problem = archive_error_in(&error)
        .is_some_and(|found| found.is_key_required() || found.is_wrong_key());
    if key_problem {
        ModelFailure::Fatal(Box::new(error))
    } else {
        ModelFailure::Unreadable {
            reason: describe(&error),
        }
    }
}

fn failure(error: ConversionError) -> ModelFailure {
    let key_problem = archive_error_in(&error)
        .is_some_and(|found| found.is_key_required() || found.is_wrong_key());
    if key_problem {
        ModelFailure::Fatal(Box::new(error))
    } else {
        ModelFailure::Unreadable {
            reason: describe(&error),
        }
    }
}

/// Keeps the level of `model` that [`Model::nearest_lod`] picks for `level`.
fn keep_nearest_lod(model: &mut Model, level: u32) {
    if let Some(nearest) = model.nearest_lod(level).map(|lod| lod.level) {
        model.lods.retain(|found| found.level == nearest);
    }
}

/// Supplies the models and material libraries an Assembly is built from.
///
/// The Assembly calls it from several threads at once, once per distinct file.
pub trait ModelProvider: Sync {
    /// Reads the model at `path`, a path below `Data` as an Entity Record spells it, at the level
    /// of detail `lod`, or at the nearest lower level the model has, or at its lowest level when
    /// it has none up to `lod`. The model comes without its material library.
    ///
    /// # Errors
    ///
    /// Returns [`ModelFailure::Missing`] when there is no such file and
    /// [`ModelFailure::Unreadable`] when it, or a companion, cannot be read as a model.
    fn model(&self, path: &str, lod: u32) -> Result<ProvidedModel, ModelFailure>;

    /// Reads the material library `name` for a model at `entry`; a bare name is looked for beside
    /// the model first. A library that does not exist or cannot be read is a status.
    fn library(&self, entry: &str, name: &str) -> MaterialStatus;

    /// Returns the budget of layer files one caller, such as one package, may read through
    /// [`Self::layer_library`]; the default has the default limits.
    fn layer_budget(&self) -> LayerBudget {
        LayerBudget::default()
    }

    /// Reads the layer file at `path`, a path below `Data`, as a library for a caller that
    /// describes the layer's whole material, charging `budget`. A source that shares its layer
    /// files overrides it to fetch and parse each once. The default applies no budget: it reads
    /// the file like any library, with no entry to look beside.
    fn layer_library(&self, path: &str, _budget: &mut LayerBudget) -> Arc<MaterialStatus> {
        Arc::new(self.library("", path))
    }

    /// Reads the bytes of the file at `path`, a path below `Data`, such as the DNA rig a piece is
    /// shaped by. The default has no such files.
    ///
    /// # Errors
    ///
    /// Returns [`ModelFailure::Missing`] when there is no such file and
    /// [`ModelFailure::Unreadable`] when it holds more than `limit` bytes or cannot be read.
    fn file(&self, path: &str, limit: u64) -> Result<Vec<u8>, ModelFailure> {
        let _ = (path, limit);
        Err(ModelFailure::Missing)
    }

    /// Poses `model`, read from the landing gear's geometry `path`, at the end of the clip that
    /// deploys the gear, and returns the posed copy; `None` when the source has no such clip. The
    /// default has none.
    ///
    /// # Errors
    ///
    /// Returns [`ModelFailure::Unreadable`] when a clip is found but cannot be read or applied
    /// to `model`.
    fn deployed(&self, path: &str, model: &Model) -> Result<Option<Model>, ModelFailure> {
        let _ = (path, model);
        Ok(None)
    }
}

/// Reads models and material libraries from the Entries of an Archive below `Data`.
///
/// The layer files of the libraries are fetched and parsed once for all of them, and each texture
/// is judged for its colour once.
#[derive(Debug)]
pub struct ArchiveModels<'a> {
    archive: &'a Archive,
    index: PathIndex<'a>,
    max_attachment_vertices: usize,
    layers: LayerCache,
    verdicts: TextureVerdicts,
}

impl<'a> ArchiveModels<'a> {
    /// Indexes the Entries of `archive`.
    #[must_use]
    pub fn new(archive: &'a Archive) -> Self {
        Self {
            archive,
            index: PathIndex::new(archive),
            max_attachment_vertices: MAX_ASSEMBLY_VERTICES,
            layers: LayerCache::new(),
            verdicts: TextureVerdicts::new(),
        }
    }

    /// Returns the provider with a limit of `max` vertices for the models of one character
    /// definition together, its skin and its bone attachments, at most [`MAX_ASSEMBLY_VERTICES`].
    /// Attachments that would pass it are not read further and are listed in
    /// [`ProvidedModel::left_out`].
    #[must_use]
    pub fn with_max_attachment_vertices(mut self, max: usize) -> Self {
        self.max_attachment_vertices = max.min(MAX_ASSEMBLY_VERTICES);
        self
    }

    /// Returns the layer files the libraries read through [`ModelProvider::library`] share.
    #[must_use]
    pub const fn layer_cache(&self) -> &LayerCache {
        &self.layers
    }

    /// Returns whether the Archive holds a file at `path`, a path below `Data` as an Entity Record
    /// spells it.
    #[must_use]
    pub fn contains(&self, path: &str) -> bool {
        self.entry_of(path).is_some()
    }

    /// Returns whether the Archive holds a file below the folder `folder`, a path below `Data`,
    /// ignoring ASCII case and the kind of separator.
    #[must_use]
    pub fn has_folder(&self, folder: &str) -> bool {
        let wanted = format!("data/{}/", folder.trim_matches(['/', '\\']));
        self.archive
            .entries()
            .iter()
            .any(|entry| starts_with_path(entry.path(), &wanted))
    }

    /// Returns a loader for the textures of the materials, reading the Entries the models come
    /// from; it shares the index of the models.
    #[must_use]
    pub const fn textures(&self) -> ArchiveTextures<'_, 'a> {
        ArchiveTextures::new(&self.index)
    }

    /// Reads the animations `choice` selects from the `.caf` files and `.dba` databases below the
    /// folders `folders`, paths below `Data`, and the Entries `extra`; see
    /// [`AnimationSet::from_folders`].
    ///
    /// # Errors
    ///
    /// As [`AnimationSet::from_folders`].
    pub fn animations(
        &self,
        folders: &[&str],
        choice: &AnimationChoice,
        extra: &[String],
    ) -> Result<AnimationSet, ConversionError> {
        let folders: Vec<String> = folders
            .iter()
            .map(|folder| format!("Data/{}", folder.trim_start_matches(['/', '\\'])))
            .collect();
        let folders: Vec<&str> = folders.iter().map(String::as_str).collect();
        AnimationSet::from_folders(&self.index, &folders, choice, extra)
    }

    pub(crate) fn entry_of(&self, path: &str) -> Option<String> {
        let path = path.trim_start_matches(['/', '\\']);
        [format!("Data\\{path}"), path.to_owned()]
            .into_iter()
            .find_map(|candidate| {
                self.index
                    .find(&candidate)
                    .map(|entry| entry.path().to_owned())
            })
    }
}

impl ModelProvider for ArchiveModels<'_> {
    fn model(&self, path: &str, lod: u32) -> Result<ProvidedModel, ModelFailure> {
        let entry = self.entry_of(path).ok_or(ModelFailure::Missing)?;
        if is_character_definition(&entry) {
            return self.character(&entry, lod);
        }
        self.read_model(entry, lod, Vec::new())
    }

    fn library(&self, entry: &str, name: &str) -> MaterialStatus {
        ModelSource::material_library(&self.index, entry, name, &self.layers, &self.verdicts)
    }

    fn layer_budget(&self) -> LayerBudget {
        self.layers.budget()
    }

    fn layer_library(&self, path: &str, budget: &mut LayerBudget) -> Arc<MaterialStatus> {
        ModelSource::layer_library(&self.index, path, &self.layers, &self.verdicts, budget)
    }

    fn file(&self, path: &str, limit: u64) -> Result<Vec<u8>, ModelFailure> {
        let path = self.entry_of(path).ok_or(ModelFailure::Missing)?;
        let entry = self.index.find(&path).ok_or(ModelFailure::Missing)?;
        let size = entry.uncompressed_size();
        if size > limit {
            return Err(ModelFailure::Unreadable {
                reason: format!("the file holds {size} bytes, more than the limit of {limit}"),
            });
        }
        let reader = self.archive.open_entry(entry).map_err(read_failure)?;
        let mut bytes = Vec::with_capacity(usize::try_from(size).unwrap_or_default());
        reader
            .take(limit)
            .read_to_end(&mut bytes)
            .map_err(read_failure)?;
        Ok(bytes)
    }

    fn deployed(&self, path: &str, model: &Model) -> Result<Option<Model>, ModelFailure> {
        self.pose_gear(path, model)
    }
}

impl ArchiveModels<'_> {
    pub(crate) fn animations_of(
        &self,
        extra: &[String],
        choice: &AnimationChoice,
    ) -> Result<AnimationSet, ConversionError> {
        AnimationSet::from_folders(&self.index, &[], choice, extra)
    }

    fn read_model(
        &self,
        entry: String,
        lod: u32,
        left_out: Vec<String>,
    ) -> Result<ProvidedModel, ModelFailure> {
        let read = |selection| {
            ModelSource::from_index_reading(
                &self.index,
                &entry,
                selection,
                PositionBox::default(),
                MaterialReading::Skipped,
                &self.verdicts,
            )
        };
        let source = match read(LodSelection::AtMost(lod)) {
            Err(ConversionError::Model {
                source: ModelError::NoSuchLod { .. },
                ..
            }) => read(LodSelection::All),
            other => other,
        }
        .map_err(failure)?;
        let mut model = source.into_model();
        keep_nearest_lod(&mut model, lod);
        if model.lods.is_empty() {
            return Err(ModelFailure::Unreadable {
                reason: "the model has no level of detail".to_owned(),
            });
        }
        Ok(ProvidedModel {
            entry,
            model,
            left_out,
            attachments: Vec::new(),
            skeleton: None,
        })
    }
}

/// Returns `path` without a trailing `.mtl`, ignoring ASCII case.
fn without_mtl(path: &str) -> &str {
    match path.rsplit_once('.') {
        Some((stem, extension)) if extension.eq_ignore_ascii_case("mtl") => stem,
        _ => path,
    }
}
