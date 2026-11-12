// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_model::{LodSelection, MaterialReading, Model, ModelFile, ModelNodeSources, PositionBox};

use super::{ModelSource, split_entry_path};
use crate::ConversionError;
use crate::chunk_source::ChunkSource;
use crate::model_companions::ArchiveCompanions;
use crate::path_index::PathIndex;
use crate::source_provenance::ModelArchiveSource;
use crate::texture_grey::TextureVerdicts;

#[derive(Clone, Copy)]
enum NodeCoordinates {
    Skip,
    Observe,
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
            NodeCoordinates::Skip,
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
            NodeCoordinates::Observe,
        )
    }

    fn read_archive(
        index: &PathIndex<'_>,
        path: &str,
        lods: LodSelection,
        position_box: PositionBox,
        reading: MaterialReading,
        verdicts: &TextureVerdicts,
        coordinates: NodeCoordinates,
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
        let mut nodes = ModelNodeSources::default();
        let observe =
            matches!(coordinates, NodeCoordinates::Observe) && source.archive_origin().is_some();
        let read = if observe {
            Model::read_with_node_sources(
                file_name,
                source.file(),
                &mut companions,
                lods,
                position_box,
                reading,
                &mut nodes,
            )
        } else {
            Model::read_with_materials(
                file_name,
                source.file(),
                &mut companions,
                lods,
                position_box,
                reading,
            )
        };
        let model = read.map_err(|source| ConversionError::Model {
            file: name.clone(),
            source,
        })?;
        if observe && let Some(primary) = archive_sources.first_mut() {
            primary.set_node_sources(nodes);
        }
        Ok(Self {
            protection: source.into_protection(),
            name,
            inputs: vec![index.archive().path().to_path_buf()],
            model,
            archive_sources,
        })
    }
}
