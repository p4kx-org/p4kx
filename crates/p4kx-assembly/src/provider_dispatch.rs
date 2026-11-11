// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_extraction::model::{LayerBudget, MaterialStatus, Model};
use p4kx_extraction::{ArchiveMaterialRead, ModelSource};

use super::{ArchiveModels, ModelFailure, ModelProvider, ProvidedModel};
use crate::cdf::is_character_definition;

impl ModelProvider for ArchiveModels<'_> {
    fn model(&self, path: &str, lod: u32) -> Result<ProvidedModel, ModelFailure> {
        let entry = self.entry_of(path).ok_or(ModelFailure::Missing)?;
        if is_character_definition(&entry) {
            return self.character(&entry, lod);
        }
        self.read_model(entry, lod, Vec::new())
    }

    fn library(&self, entry: &str, name: &str) -> MaterialStatus {
        self.library_observed(entry, name).status
    }

    fn library_observed(&self, entry: &str, name: &str) -> ArchiveMaterialRead {
        ModelSource::material_library_observed(
            &self.index,
            entry,
            name,
            &self.layers,
            &self.verdicts,
            &self.material_reads,
        )
    }

    fn layer_budget(&self) -> LayerBudget {
        self.layers.budget()
    }

    fn layer_library(&self, path: &str, budget: &mut LayerBudget) -> Arc<MaterialStatus> {
        ModelSource::layer_library_tracking(
            &self.index,
            path,
            &self.layers,
            &self.verdicts,
            budget,
            Some(&self.material_reads),
        )
    }

    fn file(&self, path: &str, limit: u64) -> Result<Vec<u8>, ModelFailure> {
        let entry = self.file_entry(path).ok_or(ModelFailure::Missing)?;
        self.read_file_entry(entry, limit)
    }

    fn deployed(&self, path: &str, model: &Model) -> Result<Option<Model>, ModelFailure> {
        self.pose_gear(path, model)
    }
}
