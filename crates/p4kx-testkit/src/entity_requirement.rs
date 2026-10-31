// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::{TestInstance, TestValue};

/// The `Requirement` of a synthetic loadout assortment or group.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TestRequirement {
    /// Tags that must be present and tags that must be absent, as the GUIDs of the Tags.
    Tags {
        /// The required Tags.
        required: Vec<u128>,
        /// The banned Tags.
        banned: Vec<u128>,
    },
    /// Holds when one of the parts holds.
    AnyOf(Vec<Self>),
    /// Holds when the part does not.
    Not(Box<Self>),
}

impl TestRequirement {
    /// Returns a tag Requirement that names no tag and so restricts nothing.
    #[must_use]
    pub const fn unrestricted() -> Self {
        Self::Tags {
            required: Vec::new(),
            banned: Vec::new(),
        }
    }

    /// Returns the pointer value that holds the Requirement in a Record.
    #[must_use]
    pub fn value(&self) -> TestValue {
        let tags = |guids: &[u128]| {
            TestValue::Array(guids.iter().copied().map(TestValue::Reference).collect())
        };
        TestValue::Struct(match self {
            Self::Tags { required, banned } => TestInstance::new("SLoadoutRequirementTags")
                .value("RequiredTags", tags(required))
                .value("BannedTags", tags(banned)),
            Self::AnyOf(parts) => TestInstance::new("SLoadoutRequirementOR").value(
                "Requirements",
                TestValue::Array(parts.iter().map(Self::value).collect()),
            ),
            Self::Not(part) => TestInstance::new("SLoadoutRequirementNOT")
                .value("RequirementToInvert", part.value()),
        })
    }
}
