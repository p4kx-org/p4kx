// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_chunks::ChunkFile;
use p4kx_model::{
    CompanionResolver, LodSelection, MaterialReadObservation, MaterialReading, Model, ModelError,
    ModelFile, ModelNodeSources, ModelReadSources, PositionBox,
};

use super::{ModelSource, split_entry_path};
use crate::chunk_source::ChunkSource;
use crate::model_companions::ArchiveCompanions;
use crate::model_info_materials::receipt::{
    MaterialEvidence, MaterialPageOrigins, MaterialPageRead, Observations, ReceiptError,
};
use crate::path_index::PathIndex;
use crate::source_provenance::ModelArchiveSource;
use crate::source_provenance::model_material_origins::MaterialObservation;
use crate::texture_grey::TextureVerdicts;
use crate::{ArchiveMaterialReads, ConversionError, FieldPath, MaterialArchiveOrigins, SourceUses};

#[derive(Clone, Copy)]
enum Coordinates {
    Skip,
    Nodes,
    NodesAndBones,
    NodesAndMaterialListing,
    MaterialListing,
    MaterialPage,
}

impl Coordinates {
    const fn capture<'a>(
        self,
        source: &ChunkSource,
        observed: &'a mut ModelReadSources,
    ) -> Capture<'a> {
        if source.archive_origin().is_none() {
            return Capture::Skip;
        }
        match self {
            Self::Skip | Self::MaterialListing | Self::MaterialPage => Capture::Skip,
            Self::Nodes | Self::NodesAndMaterialListing => Capture::Nodes(&mut observed.nodes),
            Self::NodesAndBones => Capture::NodesAndBones(observed),
        }
    }

    const fn keeps_materials(self) -> bool {
        matches!(
            self,
            Self::MaterialListing | Self::NodesAndMaterialListing | Self::MaterialPage
        )
    }

    fn material_evidence(
        self,
        model: &Model,
        source: &ChunkSource,
        origins: &MaterialArchiveOrigins,
        fields: Observations,
    ) -> Result<Option<MaterialEvidence>, ConversionError> {
        Ok(match self {
            Self::MaterialListing | Self::NodesAndMaterialListing => fields
                .project(model, source.archive_origin(), origins.library())?
                .map(MaterialEvidence::Listing),
            Self::MaterialPage => Some(MaterialEvidence::Page(Box::new(MaterialPageRead {
                fields,
                library: origins.library().cloned(),
            }))),
            _ => None,
        })
    }
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

    /// Reads an Archive model listing with exact origins for supported material fields.
    ///
    /// # Errors
    /// Returns the original model-read or receipt-recording error.
    pub fn from_index_with_material_receipt(
        index: &PathIndex<'_>,
        path: &str,
        verdicts: &TextureVerdicts,
    ) -> Result<Self, ConversionError> {
        Self::read_archive(
            index,
            path,
            LodSelection::All,
            PositionBox::default(),
            MaterialReading::Plain,
            verdicts,
            Coordinates::MaterialListing,
        )
    }

    /// Reads an Archive model listing with original node and material field evidence.
    ///
    /// # Errors
    /// Returns the original model-read or receipt-recording error.
    pub fn from_index_with_listing_origins(
        index: &PathIndex<'_>,
        path: &str,
        verdicts: &TextureVerdicts,
    ) -> Result<Self, ConversionError> {
        Self::read_archive(
            index,
            path,
            LodSelection::All,
            PositionBox::default(),
            MaterialReading::Plain,
            verdicts,
            Coordinates::NodesAndMaterialListing,
        )
    }

    /// Reads an Archive model listing with original material observations for paging.
    ///
    /// # Errors
    /// As [`Self::from_index_reading`].
    pub fn from_index_with_material_pages(
        index: &PathIndex<'_>,
        path: &str,
        verdicts: &TextureVerdicts,
    ) -> Result<Self, ConversionError> {
        Self::read_archive(
            index,
            path,
            LodSelection::All,
            PositionBox::default(),
            MaterialReading::Plain,
            verdicts,
            Coordinates::MaterialPage,
        )
    }

    /// Writes source evidence for the emitted range of a material listing at `items`.
    ///
    /// # Errors
    /// Fails if the model was not read for material paging, a row is outside the listing,
    /// or its evidence cannot be recorded.
    pub fn material_page_origins(
        &self,
        uses: &SourceUses,
        items: &FieldPath,
        rows: Range<usize>,
    ) -> Result<MaterialPageOrigins, ConversionError> {
        let Some(MaterialEvidence::Page(read)) = &self.material_evidence else {
            return Err(ConversionError::MaterialSourceEvidence(Box::new(
                ReceiptError::MissingRows,
            )));
        };
        let model_entry = self
            .archive_sources
            .iter()
            .find(|read| read.file() == ModelFile::Model)
            .and_then(ModelArchiveSource::origin);
        read.fields.project_page(
            &self.model,
            model_entry,
            read.library.as_ref(),
            uses,
            items,
            rows,
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
        let keep_materials = coordinates.keeps_materials();
        let reads = ArchiveMaterialReads::default();
        let mut material_origins = MaterialArchiveOrigins::default();
        let mut material_fields = Observations::default();
        let mut retain_field = |field: MaterialReadObservation<'_>| material_fields.retain(field);
        let mut companions = ArchiveCompanions {
            index,
            folder,
            verdicts,
            model_sources: Some(&mut archive_sources),
            material_observation: keep_materials.then_some(MaterialObservation {
                reads: &reads,
                used: Some(&mut material_origins),
            }),
            material_fields: keep_materials.then_some(&mut retain_field as _),
        };
        let mut observed = ModelReadSources::default();
        let capture = coordinates.capture(&source, &mut observed);
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
        if source.archive_origin().is_some() {
            bind_sources(coordinates, observed, &mut archive_sources);
        }
        let material_evidence =
            coordinates.material_evidence(&model, &source, &material_origins, material_fields)?;
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
            material_evidence,
        })
    }
}

fn bind_sources(
    coordinates: Coordinates,
    observed: ModelReadSources,
    archive_sources: &mut [ModelArchiveSource],
) {
    if matches!(
        coordinates,
        Coordinates::Nodes | Coordinates::NodesAndBones | Coordinates::NodesAndMaterialListing
    ) {
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
