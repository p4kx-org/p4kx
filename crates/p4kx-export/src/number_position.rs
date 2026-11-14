// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::Value;

use crate::resource_raw_policy::{self, ResourceNumberContract};

/// Supported raw-source traversal positions used only to stage possible numeric coverage.
#[derive(Clone, Copy)]
pub(crate) enum NumberPosition {
    None,
    Root,
    Density,
    DensityUnit,
    Properties,
    Property,
    Distribution,
    Overrides,
    OverrideList,
    OverrideEntry,
    Quantization,
    Bands,
    Band,
    Transformation,
    TransformationChildren,
    Number(ResourceNumberContract),
}

impl NumberPosition {
    pub(crate) fn field(self, owner: &str, name: &str) -> Self {
        match self {
            Self::Root => root_field(owner, name),
            Self::Density => match (owner, name) {
                ("ResourceTypeDensity", "densityUnit") => Self::DensityUnit,
                ("ResourceTypeMolarMass", "molarMass") => numeric(owner, name),
                _ => Self::None,
            },
            Self::DensityUnit | Self::Distribution | Self::Band => numeric(owner, name),
            Self::Property => property_field(owner, name),
            Self::Overrides
                if owner == "CraftingQualityLocationOverride" && name == "locationOverrideList" =>
            {
                Self::OverrideList
            }
            Self::OverrideEntry
                if owner == "CraftingQualityLocationOverrideEntry"
                    && name == "qualityDistribution" =>
            {
                Self::Distribution
            }
            Self::Quantization if owner == "CraftingQualityQuantization" && name == "bands" => {
                Self::Bands
            }
            Self::Transformation => match (owner, name) {
                ("CraftingDisplayTransformation_Scale", "scale") => numeric(owner, name),
                ("CraftingDisplayTransformation_Sequence", "transformations") => {
                    Self::TransformationChildren
                }
                _ => Self::None,
            },
            _ => Self::None,
        }
    }

    pub(crate) const fn element(self) -> Self {
        match self {
            Self::Properties => Self::Property,
            Self::OverrideList => Self::OverrideEntry,
            Self::Bands => Self::Band,
            Self::TransformationChildren => Self::Transformation,
            _ => Self::None,
        }
    }

    pub(crate) const fn contract(self, value: &Value<'_>) -> Option<ResourceNumberContract> {
        match self {
            Self::Number(contract) if contract.captures(value) => Some(contract),
            _ => None,
        }
    }
}

fn root_field(owner: &str, name: &str) -> NumberPosition {
    match (owner, name) {
        ("ResourceType", "densityType") => NumberPosition::Density,
        ("ResourceType", "properties") => NumberPosition::Properties,
        ("CraftingQualityDistributionRecord", "qualityDistribution") => {
            NumberPosition::Distribution
        }
        ("CraftingQualityLocationOverrideRecord", "locationOverride") => NumberPosition::Overrides,
        ("CraftingQualityQuantizationRecord", "qualityQuantization") => {
            NumberPosition::Quantization
        }
        ("CraftingGameplayPropertyDef", "displayTransformation") => NumberPosition::Transformation,
        _ => NumberPosition::None,
    }
}

fn property_field(owner: &str, name: &str) -> NumberPosition {
    match (owner, name) {
        ("ResourceTypeVolatility", "volatility" | "healthDecayPerSecond") => numeric(owner, name),
        ("ResourceTypeCraftingData", "qualityDistribution") => NumberPosition::Distribution,
        ("ResourceTypeCraftingData", "qualityLocationOverride") => NumberPosition::Overrides,
        ("ResourceTypeCraftingData", "qualityQuantization") => NumberPosition::Quantization,
        _ => NumberPosition::None,
    }
}

fn numeric(owner: &str, name: &str) -> NumberPosition {
    resource_raw_policy::field_contract(owner, name)
        .map_or(NumberPosition::None, NumberPosition::Number)
}
