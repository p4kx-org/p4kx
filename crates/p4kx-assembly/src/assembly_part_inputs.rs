// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_extraction::{EntryOrigin, MaterialArchiveOrigins, ModelArchiveSource, SourceLocation};

use super::Part;
use crate::PortPlacementSource;
use crate::build_models::Loaded;
use crate::cdf_model_sources::CdfModelSources;

#[derive(Debug, Clone, Default)]
pub(crate) struct ModelDefinition {
    origin: Option<EntryOrigin>,
    cdf: Option<Arc<CdfModelSources>>,
    material_from_model: bool,
}

impl ModelDefinition {
    pub(crate) fn of(loaded: Option<&Loaded>, material_from_model: bool) -> Self {
        let Some(loaded) = loaded else {
            return Self::default();
        };
        Self {
            origin: loaded.definition_origin.clone(),
            cdf: loaded.cdf_model_sources.clone(),
            material_from_model,
        }
    }
}

impl Part {
    /// Returns the original authored field used to choose this part's model.
    #[must_use]
    pub fn geometry_selector(&self) -> Option<SourceLocation> {
        self.geometry_selectors.model_field()
    }

    /// Returns the original authored field that requests this part's material library.
    #[must_use]
    pub fn material_selector(&self) -> Option<SourceLocation> {
        self.geometry_selectors.material_field()
    }

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

    /// Returns the original CDF attributes that selected the part's main model.
    pub fn cdf_model_selectors(&self) -> impl Iterator<Item = SourceLocation> {
        self.model_definition
            .cdf
            .iter()
            .flat_map(|source| source.model())
    }

    /// Returns the original CDF material selector when the part uses that override.
    #[must_use]
    pub fn cdf_material_selector(&self) -> Option<SourceLocation> {
        if self.model_definition.material_from_model {
            self.model_definition.cdf.as_ref()?.material()
        } else {
            None
        }
    }

    /// Returns the actual character definition that selected the model, if observed.
    #[must_use]
    pub const fn model_definition_origin(&self) -> Option<&EntryOrigin> {
        self.model_definition.origin.as_ref()
    }

    /// Returns the original observations of the material library used by this part.
    #[must_use]
    pub fn material_archive_origins(&self) -> &MaterialArchiveOrigins {
        &self.materials.origins
    }
}
