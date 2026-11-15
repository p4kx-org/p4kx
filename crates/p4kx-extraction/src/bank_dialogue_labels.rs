// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_audio::{BankDialogueChildCoverage, BankDialogueKeyRole};
use schemars::JsonSchema;
use serde::Serialize;

/// Interpretation of the original `DialogueEvent` mode byte.
#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum DialogueDecisionMode {
    /// Best-match selection mode.
    BestMatch,
    /// Weighted selection mode.
    Weighted,
    /// The raw mode code has no established label.
    Unknown,
}

impl From<u8> for DialogueDecisionMode {
    fn from(code: u8) -> Self {
        match code {
            0 => Self::BestMatch,
            1 => Self::Weighted,
            _ => Self::Unknown,
        }
    }
}

/// Interpretation of an original `DialogueEvent` argument type byte.
#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum DialogueGroupType {
    /// The argument selects a switch value.
    Switch,
    /// The argument selects a state value.
    State,
    /// The raw group type code has no established label.
    Unknown,
}

impl From<u8> for DialogueGroupType {
    fn from(code: u8) -> Self {
        match code {
            0 => Self::Switch,
            1 => Self::State,
            _ => Self::Unknown,
        }
    }
}

/// Disposition of the unclassified target or child-range word.
#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum DialogueLinkInterpretation {
    /// Both readings remain candidates, with no selected interpretation.
    Undetermined,
}

/// Role suggested by the original key and physical node position.
#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum DialogueKeyRole {
    /// The physical root has no argument condition.
    Root,
    /// A non-root zero key can match the default condition on a route.
    DefaultCandidate,
    /// A non-root key can identify an argument value on a route.
    ValueCandidate,
}

impl TryFrom<BankDialogueKeyRole> for DialogueKeyRole {
    type Error = &'static str;

    fn try_from(role: BankDialogueKeyRole) -> Result<Self, Self::Error> {
        match role {
            BankDialogueKeyRole::Root => Ok(Self::Root),
            BankDialogueKeyRole::DefaultCandidate => Ok(Self::DefaultCandidate),
            BankDialogueKeyRole::ValueCandidate => Ok(Self::ValueCandidate),
            _ => Err("unexpected DialogueEvent node key role"),
        }
    }
}

/// Checked relationship between a potential child range and the original node table.
#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum DialogueChildCoverage {
    /// All candidate children fit in the readable complete prefix.
    Complete,
    /// The declared range fits but extends beyond readable complete nodes.
    Truncated,
    /// The raw range exceeds the declared table of whole nodes.
    OutsideDeclared,
    /// The original candidate child count is zero.
    Empty,
}

impl TryFrom<BankDialogueChildCoverage> for DialogueChildCoverage {
    type Error = &'static str;

    fn try_from(coverage: BankDialogueChildCoverage) -> Result<Self, Self::Error> {
        match coverage {
            BankDialogueChildCoverage::Complete => Ok(Self::Complete),
            BankDialogueChildCoverage::Truncated => Ok(Self::Truncated),
            BankDialogueChildCoverage::OutsideDeclared => Ok(Self::OutsideDeclared),
            BankDialogueChildCoverage::Empty => Ok(Self::Empty),
            _ => Err("unexpected DialogueEvent child range coverage"),
        }
    }
}
