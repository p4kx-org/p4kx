// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

/// How a faction reacts to the other in a `FactionHostility` modifier. A value the catalog does
/// not know is `null`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = crate::schema_vocabulary::open_string_vocabulary)]
pub(crate) enum Reaction {
    /// Friendly.
    Friendly,
    /// Neutral.
    Neutral,
    /// Hostile.
    Hostile,
}

impl Reaction {
    /// Returns the reaction the stored enum name `stored` stands for.
    pub(crate) fn parse(stored: &str) -> Option<Self> {
        Some(match stored {
            "Friendly" => Self::Friendly,
            "Neutral" => Self::Neutral,
            "Hostile" => Self::Hostile,
            _ => return None,
        })
    }
}

/// The licence a `LawLicense` modifier asks for. A value the catalog does not know is `null`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = crate::schema_vocabulary::open_string_vocabulary)]
pub(crate) enum LicenseType {
    /// `IllegalGoodsLicense_ClassA`.
    IllegalGoodsLicenseClassA,
    /// `IllegalGoodsLicense_ClassB`.
    IllegalGoodsLicenseClassB,
    /// `IllegalGoodsLicense_ClassC`.
    IllegalGoodsLicenseClassC,
}

impl LicenseType {
    /// Returns the licence the stored enum name `stored` stands for.
    pub(crate) fn parse(stored: &str) -> Option<Self> {
        Some(match stored {
            "IllegalGoodsLicense_ClassA" => Self::IllegalGoodsLicenseClassA,
            "IllegalGoodsLicense_ClassB" => Self::IllegalGoodsLicenseClassB,
            "IllegalGoodsLicense_ClassC" => Self::IllegalGoodsLicenseClassC,
            _ => return None,
        })
    }
}
