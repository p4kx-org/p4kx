// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

/// The existing dataset-condition interpretation independently of its original values.
#[derive(Debug, Clone, Copy, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(extend("x-p4kx-vocabularyClass" = "control"))]
pub(crate) enum LocationConditionKind {
    TagSearch,
    ExcludeNearbyLocations,
    ExcludeDistantLocations,
    ExcludeProperty,
    PropertyTags,
    SpecificOrganizations,
    SpecificItems,
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
