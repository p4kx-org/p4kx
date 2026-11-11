// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{EntryOrigin, MaterialArchiveOrigins, ModelArchiveSource};

use super::{Armour, ArmourPart};

impl ArmourPart {
    /// Returns the material library and layer reads selected for this part.
    #[must_use]
    pub fn material_archive_origins(&self) -> &MaterialArchiveOrigins {
        &self.material_archive_origins
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
