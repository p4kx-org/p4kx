// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_model::{LodSelection, MaterialReadField, MaterialReading, ModelFile, PositionBox};

use super::{Coordinates, MaterialEvidence, ModelSource, ReceiptError};
use crate::{
    ConversionError, FieldOrigins, FieldPath, ModelArchiveSource, PathIndex, SourceUses,
    TextureVerdicts,
};

#[path = "model_source_library_report_origins.rs"]
mod library_report;

impl Coordinates {
    pub(super) const fn keeps_material_field(self, field: MaterialReadField) -> bool {
        use MaterialReadField::{LibraryDefaultPalette, LibrarySingle, LibraryTotal};
        !matches!(self, Self::MaterialSummary)
            || matches!(field, LibraryDefaultPalette | LibrarySingle | LibraryTotal)
    }
}

impl ModelSource {
    /// Reads the model with original node, skeleton and loaded material summary evidence.
    ///
    /// # Errors
    /// Returns the original model-read error.
    pub fn from_index_with_material_summary_origins(
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
            Coordinates::MaterialSummary,
        )
    }

    /// Projects actual loaded library fields into the caller's source inventory.
    ///
    /// The count and library paths address the caller's output. An unloaded library yields no fields.
    ///
    /// # Errors
    /// Fails if a loaded library was not observed for this projection or its evidence cannot be recorded.
    pub fn loaded_material_summary_origins(
        &self,
        uses: &SourceUses,
        library: &FieldPath,
        count: &FieldPath,
    ) -> Result<FieldOrigins, ConversionError> {
        if self.model.materials.library().is_none() {
            return Ok(FieldOrigins::new());
        }
        let Some(MaterialEvidence::Summary(read)) = &self.material_evidence else {
            return Err(ConversionError::MaterialSourceEvidence(Box::new(
                ReceiptError::MissingRows,
            )));
        };
        let model_entry = self
            .archive_sources
            .iter()
            .find(|read| read.file() == ModelFile::Model)
            .and_then(ModelArchiveSource::origin);
        read.fields.project_loaded_summary(
            &self.model,
            model_entry,
            read.library.as_ref(),
            uses,
            library,
            Some(count),
        )
    }
}
