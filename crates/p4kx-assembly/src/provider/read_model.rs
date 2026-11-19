// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::model::{LodSelection, MaterialReading, ModelError, PositionBox};
use p4kx_extraction::{ArchiveModelRead, ConversionError, ModelSource};

use super::{ArchiveModels, ModelFailure, ProvidedModel, failure, keep_nearest_lod};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum ModelAdmission {
    Draw,
    DeclaredCdf,
}

impl ArchiveModels<'_> {
    pub(super) fn read_model(
        &self,
        entry: String,
        lod: u32,
        left_out: Vec<String>,
    ) -> Result<ProvidedModel, ModelFailure> {
        self.read_model_with(entry, lod, left_out, ModelAdmission::Draw)
    }

    pub(super) fn read_model_with(
        &self,
        entry: String,
        lod: u32,
        left_out: Vec<String>,
        admission: ModelAdmission,
    ) -> Result<ProvidedModel, ModelFailure> {
        let read_model = if self.observe_nodes {
            ModelSource::from_index_observing_model_sources
        } else {
            ModelSource::from_index_reading
        };
        let read = |selection| {
            read_model(
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
        };
        let source = match source {
            Err(ConversionError::Model {
                source: ModelError::MissingGeometry { .. },
                ..
            }) if admission == ModelAdmission::DeclaredCdf => read(LodSelection::OnlyOrBase(lod)),
            other => other,
        }
        .map_err(failure)?;
        let ArchiveModelRead {
            mut model,
            mut sources,
            ..
        } = source.into_model_with_archive_sources();
        keep_nearest_lod(&mut model, lod);
        if model.lods.is_empty() {
            return Err(ModelFailure::Unreadable {
                reason: "the model has no level of detail".to_owned(),
            });
        }
        sources.retain(|source| source.supplies(&model));
        Ok(ProvidedModel {
            entry,
            model,
            left_out,
            attachments: Vec::new(),
            skeleton: None,
            archive_sources: sources.into(),
            declared_rig: None,
            cdf_model_sources: None,
            definition_origin: None,
        })
    }
}
