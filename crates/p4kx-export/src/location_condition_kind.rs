// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

/// The existing dataset-condition interpretation independently of its original values.
#[derive(Debug, Clone, Copy, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(extend("x-p4kx-vocabularyClass" = "control"))]
#[non_exhaustive]
pub enum LocationConditionKind {
    /// Matches authored Tag terms.
    TagSearch,
    /// Excludes locations within an authored distance.
    ExcludeNearbyLocations,
    /// Excludes locations outside an authored distance.
    ExcludeDistantLocations,
    /// Excludes a location selected by another property.
    ExcludeProperty,
    /// Matches tags supplied by another property.
    PropertyTags,
    /// Matches the authored organization candidates.
    SpecificOrganizations,
    /// Matches the authored item candidates.
    SpecificItems,
    /// Matches the authored faction identity.
    FactionIdentity,
}

impl LocationConditionKind {
    pub(crate) fn read(name: &str) -> Option<Self> {
        Some(
            match name.strip_prefix("DataSetMatchCondition_").unwrap_or(name) {
                "TagSearch" => Self::TagSearch,
                "ExcludeNearbyLocationsDef" => Self::ExcludeNearbyLocations,
                "ExcludeDistantLocationsDef" => Self::ExcludeDistantLocations,
                "ExcludeProperty" => Self::ExcludeProperty,
                "PropertyTags" => Self::PropertyTags,
                "SpecificOrganizationsDef" => Self::SpecificOrganizations,
                "SpecificItemsDef" => Self::SpecificItems,
                "FactionIdentityDef" => Self::FactionIdentity,
                _ => return None,
            },
        )
    }
}
