// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_chunks::ChunkFile;
use p4kx_model::{
    CompanionResolver, LodSelection, MaterialReading, Model, ModelError, ModelFile,
    ModelNodeSources, ModelReadSources, PositionBox,
};

use super::{ModelSource, split_entry_path};
use crate::ConversionError;
use crate::chunk_source::ChunkSource;
use crate::model_companions::ArchiveCompanions;
use crate::path_index::PathIndex;
use crate::source_provenance::ModelArchiveSource;
use crate::texture_grey::TextureVerdicts;

#[derive(Clone, Copy)]
enum Coordinates {
    Skip,
    Nodes,
    NodesAndBones,
}

enum Capture<'a> {
    Skip,
    Nodes(&'a mut ModelNodeSources),
    NodesAndBones(&'a mut ModelReadSources),
}

impl Capture<'_> {
    fn read<B: AsRef<[u8]>>(
        self,
        name: &str,
        file: &ChunkFile<B>,
        companions: &mut dyn CompanionResolver,
        lods: LodSelection,
        position_box: PositionBox,
        reading: MaterialReading,
    ) -> Result<Model, ModelError> {
        match self {
            Self::Skip => {
                Model::read_with_materials(name, file, companions, lods, position_box, reading)
            }
            Self::Nodes(nodes) => Model::read_with_node_sources(
                name,
                file,
                companions,
                lods,
                position_box,
                reading,
                nodes,
            ),
            Self::NodesAndBones(sources) => Model::read_with_sources(
                name,
                file,
                companions,
                lods,
                position_box,
                reading,
                sources,
            ),
        }
    }
}

impl ModelSource {
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
        Self::read_archive(
            index,
            path,
            lods,
            position_box,
            reading,
            verdicts,
            Coordinates::Skip,
        )
    }

    /// Reads the model with original node coordinates in its observed primary Entry.
    ///
    /// # Errors
    ///
    /// As [`Self::from_index_reading`].
    pub fn from_index_observing_nodes(
        index: &PathIndex<'_>,
        path: &str,
        lods: LodSelection,
        position_box: PositionBox,
        reading: MaterialReading,
        verdicts: &TextureVerdicts,
    ) -> Result<Self, ConversionError> {
        Self::read_archive(
            index,
            path,
            lods,
            position_box,
            reading,
            verdicts,
            Coordinates::Nodes,
        )
    }

    /// Reads the model with original node and skeleton coordinates bound to their Entries.
    ///
    /// # Errors
    ///
    /// As [`Self::from_index_reading`].
    pub fn from_index_observing_model_sources(
        index: &PathIndex<'_>,
        path: &str,
        lods: LodSelection,
        position_box: PositionBox,
        reading: MaterialReading,
        verdicts: &TextureVerdicts,
    ) -> Result<Self, ConversionError> {
        Self::read_archive(
            index,
            path,
            lods,
            position_box,
            reading,
            verdicts,
            Coordinates::NodesAndBones,
        )
    }

    fn read_archive(
        index: &PathIndex<'_>,
        path: &str,
        lods: LodSelection,
        position_box: PositionBox,
        reading: MaterialReading,
        verdicts: &TextureVerdicts,
        coordinates: Coordinates,
    ) -> Result<Self, ConversionError> {
        let source = ChunkSource::from_index(index, path)?;
        let name = source.name().to_owned();
        let (folder, file_name) = split_entry_path(&name);
        let mut archive_sources = vec![ModelArchiveSource::new(
            ModelFile::Model,
            source.archive_origin().cloned(),
        )];
        let mut companions = ArchiveCompanions {
            index,
            folder,
            verdicts,
            model_sources: Some(&mut archive_sources),
            material_observation: None,
        };
        let mut observed = ModelReadSources::default();
        let coordinates = if source.archive_origin().is_some() {
            coordinates
        } else {
            Coordinates::Skip
        };
        let capture = match coordinates {
            Coordinates::Skip => Capture::Skip,
            Coordinates::Nodes => Capture::Nodes(&mut observed.nodes),
            Coordinates::NodesAndBones => Capture::NodesAndBones(&mut observed),
        };
        let read = capture.read(
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
        bind_sources(coordinates, observed, &mut archive_sources);
        Ok(Self {
            protection: source.into_protection(),
            name,
            inputs: vec![index.archive().path().to_path_buf()],
            default_material: (reading != MaterialReading::Skipped
                && model
                    .lods
                    .iter()
                    .any(|lod| lod.requires_material_fallback(&model.materials)))
            .then(|| Self::archive_default_material(index, verdicts)),
            model,
            archive_sources,
        })
    }
}

fn bind_sources(
    coordinates: Coordinates,
    observed: ModelReadSources,
    archive_sources: &mut [ModelArchiveSource],
) {
    if !matches!(coordinates, Coordinates::Skip) {
        if let Some(primary) = archive_sources.first_mut() {
            primary.set_node_sources(observed.nodes);
            if let Some(selection) = observed.primary_skeleton {
                primary.set_primary_skeleton(selection);
            }
        }
        for skeleton in observed.skeletons {
            if let Some(source) = archive_sources
                .iter_mut()
                .find(|source| source.file() == skeleton.file())
            {
                source.set_bone_sources(skeleton);
            }
        }
    }
}
