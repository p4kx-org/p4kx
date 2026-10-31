// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::{Path, PathBuf};
use std::sync::Arc;

use p4kx_archive::Archive;
use p4kx_model::{
    LayerBudget, LayerCache, LodSelection, MaterialReading, MaterialStatus, Model, PositionBox,
};

use crate::ConversionError;
use crate::chunk_source::ChunkSource;
use crate::model_companions::{ArchiveCompanions, DiskCompanions};
use crate::path_index::PathIndex;
use crate::texture_grey::TextureVerdicts;

/// A model read from a chunk file Entry of an Archive or from a file on disk, with the
/// companions next to it.
#[derive(Debug)]
pub struct ModelSource {
    name: String,
    inputs: Vec<PathBuf>,
    model: Model,
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

    /// Reads the model like [`Self::from_index_with_box`], reading the material library the model
    /// names as `reading` says, see [`Model::read_with_materials`]: [`MaterialReading::Skipped`]
    /// for a caller that chooses and caches the libraries itself, see [`Self::material_library`],
    /// [`MaterialReading::Plain`] for a listing such as `model info`. The colour verdicts of the
    /// textures the library names come from `verdicts` and stay there.
    ///
    /// # Errors
    ///
    /// As [`Self::from_index_with`].
    pub fn from_index_reading(
        index: &PathIndex<'_>,
        path: &str,
        lods: LodSelection,
        position_box: PositionBox,
        reading: MaterialReading,
        verdicts: &TextureVerdicts,
    ) -> Result<Self, ConversionError> {
        let source = ChunkSource::from_index(index, path)?;
        let name = source.name().to_owned();
        let (folder, file_name) = split_entry_path(&name);
        let mut companions = ArchiveCompanions {
            index,
            folder,
            verdicts,
        };
        let read = Model::read_with_materials(
            file_name,
            source.file(),
            &mut companions,
            lods,
            position_box,
            reading,
        );
        let model = read.map_err(|source| ConversionError::Model {
            file: name.clone(),
            source,
        })?;
        Ok(Self {
            name,
            inputs: vec![index.archive().path().to_path_buf()],
            model,
        })
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
        let (folder, _) = split_entry_path(model_path);
        let mut companions = ArchiveCompanions {
            index,
            folder,
            verdicts,
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
        let mut companions = ArchiveCompanions {
            index,
            folder: "",
            verdicts,
        };
        cache.library(path, &mut companions, budget)
    }

    /// Returns the model, giving up the source.
    #[must_use]
    pub fn into_model(self) -> Model {
        self.model
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
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        let folder = path.parent().unwrap_or_else(|| Path::new(""));
        let mut companions = DiskCompanions {
            folder,
            read: vec![path.to_path_buf()],
            absent: None,
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
            name: path.display().to_string(),
            inputs: companions.read,
            model,
        })
    }

    /// Returns the name of the model: its Entry path or its path on disk.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
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
