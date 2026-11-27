// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

/// Recognized `ScopedLocation` filter meaning independently of retained source payloads.
#[derive(Debug, Clone, Copy, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(extend("x-p4kx-vocabularyClass" = "control"))]
pub(crate) enum ScopedLocationFilterKind {
    RequiredFactionHostility,
    ExcludedFactionHostility,
    RequiredLocationClass,
    RequiredLocationClasses,
    ExcludedLocationClass,
    ExcludedLocationClasses,
    RequiredLocationSize,
    RequiredLocationSizes,
    ExcludedLocationSize,
    ExcludedLocationSizes,
    RequiredLocationFunction,
    RequiredLocationFunctions,
    ExcludedLocationFunction,
    ExcludedLocationFunctions,
    NotLocation,
    SameLocation,
    NotLocations,
    SameScope,
    DifferentScope,
}

impl ScopedLocationFilterKind {
    pub(crate) fn read(name: &str) -> Option<Self> {
        Some(match name {
            "LocationCondition_RequiredFactionHostility" => Self::RequiredFactionHostility,
            "LocationCondition_ExcludedFactionHostility" => Self::ExcludedFactionHostility,
            "LocationCondition_RequiredLocationClass" => Self::RequiredLocationClass,
            "LocationCondition_RequiredLocationClasses" => Self::RequiredLocationClasses,
            "LocationCondition_ExcludedLocationClass" => Self::ExcludedLocationClass,
            "LocationCondition_ExcludedLocationClasses" => Self::ExcludedLocationClasses,
            "LocationCondition_RequiredLocationSize" => Self::RequiredLocationSize,
            "LocationCondition_RequiredLocationSizes" => Self::RequiredLocationSizes,
            "LocationCondition_ExcludedLocationSize" => Self::ExcludedLocationSize,
            "LocationCondition_ExcludedLocationSizes" => Self::ExcludedLocationSizes,
            "LocationCondition_RequiredLocationFunction" => Self::RequiredLocationFunction,
            "LocationCondition_RequiredLocationFunctions" => Self::RequiredLocationFunctions,
            "LocationCondition_ExcludedLocationFunction" => Self::ExcludedLocationFunction,
            "LocationCondition_ExcludedLocationFunctions" => Self::ExcludedLocationFunctions,
            "LocationCondition_NotLocation" => Self::NotLocation,
            "LocationCondition_SameLocation" => Self::SameLocation,
            "LocationCondition_NotLocations" => Self::NotLocations,
            "LocationCondition_SameScope" => Self::SameScope,
            "LocationCondition_DifferentScope" => Self::DifferentScope,
            _ => return None,
        })
    }
}
