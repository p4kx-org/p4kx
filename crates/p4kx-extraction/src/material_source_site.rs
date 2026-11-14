// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_model::{MaterialReadObservation, MaterialReadUse};

use crate::{SourceLocation, SourceUseRole};

/// The XML coordinate and role supplied by a material parser observation.
pub struct MaterialSourceSite {
    location: SourceLocation,
    role: SourceUseRole,
}

impl MaterialSourceSite {
    /// Retains original element and attribute ordinals for a supported use kind.
    #[must_use]
    pub fn from_observation(observed: MaterialReadObservation<'_>) -> Option<Self> {
        let role = match observed.use_kind {
            MaterialReadUse::Authored => SourceUseRole::Payload,
            MaterialReadUse::Selected => SourceUseRole::Selector,
            MaterialReadUse::Interpreted => SourceUseRole::Conversion,
            _ => return None,
        };
        let mut indices: Vec<_> = observed.indices.iter().map(|&index| index as u64).collect();
        let namespace = observed
            .attribute_ordinal
            .map_or("materialXmlElement", |attribute| {
                indices.push(attribute as u64);
                "materialXmlAttribute"
            });
        Some(Self {
            location: SourceLocation::Subresource {
                namespace: namespace.into(),
                indices,
                field: observed.source_field.into(),
            },
            role,
        })
    }

    /// Returns the XML coordinate inside the original source Entry.
    #[must_use]
    pub const fn location(&self) -> &SourceLocation {
        &self.location
    }

    /// Returns how the original coordinate contributed to the value.
    #[must_use]
    pub const fn role(&self) -> SourceUseRole {
        self.role
    }
}
