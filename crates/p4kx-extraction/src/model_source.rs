// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::{Path, PathBuf};
use std::sync::Arc;

use p4kx_archive::{Archive, EntryOrigin};
use p4kx_model::{
    LayerBudget, LayerCache, LodSelection, MaterialReading, MaterialStatus, Model, PositionBox,
};

use crate::ConversionError;
pub use crate::archive_material_reference::{ArchiveDefaultMaterial, ArchiveDefaultMaterialState};
use crate::chunk_source::ChunkSource;
use crate::disk_case::CaseIndex;
use crate::model_companions::{ArchiveCompanions, DiskCompanions};
use crate::path_index::PathIndex;
use crate::source_protection::SourceProtection;
use crate::source_provenance::model_material_origins::MaterialObservation;
use crate::source_provenance::{
    ArchiveMaterialRead, ArchiveMaterialReads, ArchiveModelRead, MaterialArchiveOrigins,
    ModelArchiveSource,
};
use crate::texture_grey::TextureVerdicts;

#[path = "model_source_archive.rs"]
mod archive_read;

/// A model read from a chunk file Entry of an Archive or from a file on disk, with the
/// companions next to it.
#[derive(Debug)]
pub struct ModelSource {
    pub(crate) protection: SourceProtection,
    name: String,
    inputs: Vec<PathBuf>,
    model: Model,
    archive_sources: Vec<ModelArchiveSource>,
    default_material: Option<Arc<ArchiveDefaultMaterial>>,
}

impl ModelSource {
    /// Reads the model Entry at `path`, matched with either separator and ignoring ASCII case,
    /// and its companions from the same folder of the Archive, all looked up through `index`.
    ///
    /// Each call judges the colour of its textures afresh; to share the verdicts across reads use
    /// [`Self::from_index_reading`] with one [`TextureVerdicts`].
    ///
    /// # Errors
    ///
    /// Returns [`ConversionError::UnknownEntry`] when no Entry has that path,
    /// [`ConversionError::ChunkFile`] when it is no valid chunk file,
    /// [`ConversionError::Model`] when the model or a companion is missing, unreadable or
    /// corrupt, and another [`ConversionError`] when an Entry cannot be read.
    pub fn from_index(index: &PathIndex<'_>, path: &str) -> Result<Self, ConversionError> {
        Self::from_index_with(index, path, LodSelection::All)
    }

    /// Reads only the levels of detail `lods` of the model Entry at `path`; see
    /// [`Self::from_index`].
    ///
    /// # Errors
    ///
    /// As [`Self::from_index`], and [`ConversionError::Model`] with
    /// [`p4kx_model::ModelError::NoSuchLod`] when `lods` names a level the model lacks.
    pub fn from_index_with(
        index: &PathIndex<'_>,
        path: &str,
        lods: LodSelection,
    ) -> Result<Self, ConversionError> {
        Self::from_index_with_box(index, path, lods, PositionBox::default())
    }

    /// Reads the levels of detail `lods` of the model Entry at `path` like
    /// [`Self::from_index_with`], dequantising its positions with `position_box`, see
    /// [`Model::read_with_box`]. Like [`Self::from_index`], it judges its textures afresh.
    ///
    /// # Errors
    ///
    /// As [`Self::from_index_with`].
    pub fn from_index_with_box(
        index: &PathIndex<'_>,
        path: &str,
        lods: LodSelection,
        position_box: PositionBox,
    ) -> Result<Self, ConversionError> {
        Self::from_index_reading(
            index,
            path,
            lods,
            position_box,
            MaterialReading::Drawn,
            &TextureVerdicts::new(),
        )
    }

    /// Reads the material library `library` as a model Entry at `model_path` names it: a bare
    /// name is looked for beside the model first, then below `Data`. A library that does not
    /// exist or cannot be read is a status, not a failure.
    ///
    /// The layer files come from `cache` and stay there, so that the libraries of one ship fetch
    /// and parse each shared layer file once; likewise `verdicts` for the colour of the textures.
    #[must_use]
    pub fn material_library(
        index: &PathIndex<'_>,
        model_path: &str,
        library: &str,
        cache: &LayerCache,
        verdicts: &TextureVerdicts,
    ) -> MaterialStatus {
        Self::read_material_library(index, model_path, library, cache, verdicts, None)
    }

    /// Reads the selected library and binds the original reads of its retained layers.
    #[must_use]
    pub fn material_library_observed(
        index: &PathIndex<'_>,
        model_path: &str,
        library: &str,
        cache: &LayerCache,
        verdicts: &TextureVerdicts,
        reads: &ArchiveMaterialReads,
    ) -> ArchiveMaterialRead {
        let mut origins = MaterialArchiveOrigins::default();
        let observation = MaterialObservation {
            reads,
            used: Some(&mut origins),
        };
        let status = Self::read_material_library(
            index,
            model_path,
            library,
            cache,
            verdicts,
            Some(observation),
        );
        ArchiveMaterialRead { status, origins }
    }

    fn read_material_library(
        index: &PathIndex<'_>,
        model_path: &str,
        library: &str,
        cache: &LayerCache,
        verdicts: &TextureVerdicts,
        material_observation: Option<MaterialObservation<'_>>,
    ) -> MaterialStatus {
        let (folder, _) = split_entry_path(model_path);
        let mut companions = ArchiveCompanions {
            index,
            folder,
            verdicts,
            model_sources: None,
            material_observation,
        };
        MaterialStatus::load_with(
            Some(library),
            &mut companions,
            MaterialReading::Drawn,
            cache,
        )
    }

    /// Reads the layer file at `path`, a path below `Data`, as a library through `cache`, which
    /// fetches and parses it once for every caller; see [`LayerCache::library`].
    #[must_use]
    pub fn layer_library(
        index: &PathIndex<'_>,
        path: &str,
        cache: &LayerCache,
        verdicts: &TextureVerdicts,
        budget: &mut LayerBudget,
    ) -> Arc<MaterialStatus> {
        Self::layer_library_tracking(index, path, cache, verdicts, budget, None)
    }

    /// Reads the layer and retains its physical read identity in the shared cache.
    #[must_use]
    pub fn layer_library_tracking(
        index: &PathIndex<'_>,
        path: &str,
        cache: &LayerCache,
        verdicts: &TextureVerdicts,
        budget: &mut LayerBudget,
        reads: Option<&ArchiveMaterialReads>,
    ) -> Arc<MaterialStatus> {
        Self::read_layer(index, path, cache, verdicts, budget, reads).0
    }

    /// Reads the cached layer and resolves the physical Entry of that exact read.
    #[must_use]
    pub fn layer_library_observed(
        index: &PathIndex<'_>,
        path: &str,
        cache: &LayerCache,
        verdicts: &TextureVerdicts,
        budget: &mut LayerBudget,
        reads: &ArchiveMaterialReads,
    ) -> (Arc<MaterialStatus>, Option<EntryOrigin>) {
        let (status, read) = Self::read_layer(index, path, cache, verdicts, budget, Some(reads));
        (status, read.as_ref().and_then(|read| reads.origin(read)))
    }

    fn read_layer(
        index: &PathIndex<'_>,
        path: &str,
        cache: &LayerCache,
        verdicts: &TextureVerdicts,
        budget: &mut LayerBudget,
        reads: Option<&ArchiveMaterialReads>,
    ) -> (Arc<MaterialStatus>, Option<p4kx_model::MaterialReadOrigin>) {
        let material_observation = reads.map(|reads| MaterialObservation { reads, used: None });
        let mut companions = ArchiveCompanions {
            index,
            folder: "",
            verdicts,
            model_sources: None,
            material_observation,
        };
        cache.library_with_read_origin(path, &mut companions, budget)
    }

    /// Returns the actual model and companion Entry observations.
    #[must_use]
    pub fn archive_sources(&self) -> &[ModelArchiveSource] {
        &self.archive_sources
    }

    /// Returns the model, giving up the source.
    #[must_use]
    pub fn into_model(self) -> Model {
        self.model
    }

    /// Returns the model and its Archive reads, with an empty source list for a disk read.
    #[must_use]
    pub fn into_model_with_archive_sources(self) -> ArchiveModelRead {
        ArchiveModelRead {
            model: self.model,
            sources: self.archive_sources,
        }
    }

    /// Reads the model Entry at `path` of `archive`; see [`Self::from_index`].
    ///
    /// # Errors
    ///
    /// As [`Self::from_index`].
    pub fn from_archive(archive: &Archive, path: &str) -> Result<Self, ConversionError> {
        Self::from_index(&PathIndex::new(archive), path)
    }

    /// Reads only the levels of detail `lods` of the model Entry at `path` of `archive`; see
    /// [`Self::from_index_with`].
    ///
    /// # Errors
    ///
    /// As [`Self::from_index_with`].
    pub fn from_archive_with(
        archive: &Archive,
        path: &str,
        lods: LodSelection,
    ) -> Result<Self, ConversionError> {
        Self::from_index_with(&PathIndex::new(archive), path, lods)
    }

    /// Reads the model file at `path` and its companions from the same folder.
    ///
    /// # Errors
    ///
    /// Returns [`ConversionError::NoSuchFile`] when the file does not exist,
    /// [`ConversionError::ChunkFile`] when it is no valid chunk file,
    /// [`ConversionError::Model`] when the model or a companion is missing, unreadable or
    /// corrupt, and another [`ConversionError`] when the file cannot be read.
    pub fn from_file(path: &Path) -> Result<Self, ConversionError> {
        Self::from_file_with(path, LodSelection::All)
    }

    /// Reads only the levels of detail `lods` of the model file at `path`; see
    /// [`Self::from_file`].
    ///
    /// # Errors
    ///
    /// As [`Self::from_file`], and [`ConversionError::Model`] with
    /// [`p4kx_model::ModelError::NoSuchLod`] when `lods` names a level the model lacks.
    pub fn from_file_with(path: &Path, lods: LodSelection) -> Result<Self, ConversionError> {
        Self::from_file_reading(path, lods, MaterialReading::Drawn)
    }

    /// Reads the model file at `path` like [`Self::from_file_with`], reading the material library
    /// the model names as `reading` says; see [`Self::from_index_reading`].
    ///
    /// # Errors
    ///
    /// As [`Self::from_file_with`].
    pub fn from_file_reading(
        path: &Path,
        lods: LodSelection,
        reading: MaterialReading,
    ) -> Result<Self, ConversionError> {
        let source = ChunkSource::from_file(path)?;
        let file_name = path
            .file_name()
            .map_or_default(|name| name.to_string_lossy().into_owned());
        let folder = path.parent().unwrap_or_else(|| Path::new(""));
        let mut companions = DiskCompanions {
            folder,
            read: vec![path.to_path_buf()],
            absent: None,
            case: CaseIndex::default(),
        };
        let read = Model::read_with_materials(
            &file_name,
            source.file(),
            &mut companions,
            lods,
            PositionBox::default(),
            reading,
        );
        let model = read.map_err(|source| ConversionError::Model {
            file: path.display().to_string(),
            source,
        })?;
        Ok(Self {
            protection: source.into_protection(),
            name: path.display().to_string(),
            inputs: companions.read,
            model,
            archive_sources: Vec::new(),
            default_material: None,
        })
    }

    /// Returns the name of the model: its Entry path or its path on disk.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Resolves the source fallback from this observed Archive directory and key generation.
    #[must_use]
    pub fn archive_default_material(
        index: &PathIndex<'_>,
        verdicts: &TextureVerdicts,
    ) -> Arc<ArchiveDefaultMaterial> {
        index.default_material.resolve(index, verdicts)
    }

    /// Returns the original archive fallback lookup when this model required one.
    #[must_use]
    pub fn default_material(&self) -> Option<&ArchiveDefaultMaterial> {
        self.default_material.as_deref()
    }

    /// Returns the model.
    #[must_use]
    pub const fn model(&self) -> &Model {
        &self.model
    }

    /// Returns the files the model was read from: the model file and its companions.
    pub(crate) fn inputs(&self) -> &[PathBuf] {
        &self.inputs
    }
}

fn split_entry_path(path: &str) -> (&str, &str) {
    path.rfind(['\\', '/'])
        .and_then(|at| Some((path.get(..at)?, path.get(at + 1..)?)))
        .unwrap_or(("", path))
}
