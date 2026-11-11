// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_extraction::{EntryOrigin, MaterialArchiveOrigins, ModelArchiveSource, SourceLocation};

use super::{Armour, ArmourPart, ShapeArchiveSources};
use crate::PaletteRecords;

impl ArmourPart {
    /// Returns the original DataForge field that selected the model path.
    #[must_use]
    pub const fn geometry_selector(&self) -> Option<&SourceLocation> {
        self.geometry_selector.as_ref()
    }

    /// Returns the original DataForge field that selected the material library.
    #[must_use]
    pub const fn material_selector(&self) -> Option<&SourceLocation> {
        self.material_selector.as_ref()
    }
    /// Returns the material library and layer reads selected for this part.
    #[must_use]
    pub fn material_archive_origins(&self) -> &MaterialArchiveOrigins {
        &self.material_archive_origins
    }

    /// Returns the palette Records for a slot that took tint values.
    #[must_use]
    pub fn palette_records_for(&self, slot: usize) -> Option<PaletteRecords> {
        self.palette.as_ref()?.records_for(slot)
    }

    /// Returns the original model and rig inputs used to shape this part.
    #[must_use]
    pub fn shape_archive_sources(&self) -> &[Arc<ShapeArchiveSources>] {
        &self.shape_sources
    }

    /// Returns whether a facial rig supplied joints to the assembled skeleton.
    #[must_use]
    pub const fn has_facial_rig(&self) -> bool {
        self.face.is_some()
    }

    /// Returns the physical model and companion inputs selected for this part.
    #[must_use]
    pub fn model_archive_sources(&self) -> &[ModelArchiveSource] {
        &self.archive_sources
    }

    /// Returns the character definition that supplied this part's model selection.
    #[must_use]
    pub const fn model_definition_origin(&self) -> Option<&EntryOrigin> {
        self.definition_origin.as_ref()
    }
}

impl Armour {
    /// Returns the admitted base file that supplies the body skeleton.
    #[must_use]
    pub const fn skeleton_archive_source(&self) -> Option<&ModelArchiveSource> {
        self.skeleton_archive_source.as_ref()
    }
}
