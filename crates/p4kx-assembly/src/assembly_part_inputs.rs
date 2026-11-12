// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{EntryOrigin, MaterialArchiveOrigins, ModelArchiveSource};

use super::Part;
use crate::PortPlacementSource;

impl Part {
    /// Returns authored port fields used by this part's placement, when retained.
    #[must_use]
    pub const fn port_placement_source(&self) -> Option<&PortPlacementSource> {
        self.placement_source.as_ref()
    }

    /// Returns the original Archive reads of the model retained by this part.
    #[must_use]
    pub fn model_archive_sources(&self) -> &[ModelArchiveSource] {
        &self.model_archive_sources
    }

    /// Returns the actual character definition that selected the model, if observed.
    #[must_use]
    pub const fn model_definition_origin(&self) -> Option<&EntryOrigin> {
        self.model_definition_origin.as_ref()
    }

    /// Returns the original observations of the material library used by this part.
    #[must_use]
    pub fn material_archive_origins(&self) -> &MaterialArchiveOrigins {
        &self.materials.origins
    }
}
