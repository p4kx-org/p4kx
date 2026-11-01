// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The closed value sets of the mission broker entries. A stored name the catalog does not know
//! is `null` in the entry and counted in the warning `unknown_template_values`.

use schemars::JsonSchema;
use serde::Serialize;

/// A calendar month of an availability schedule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = crate::closed_set::closed_string_set)]
pub(crate) enum Month {
    /// January.
    January,
    /// February.
    February,
    /// March.
    March,
    /// April.
    April,
    /// May.
    May,
    /// June.
    June,
    /// July.
    July,
    /// August.
    August,
    /// September.
    September,
    /// October.
    October,
    /// November.
    November,
    /// December.
    December,
}

impl Month {
    /// Returns the month the stored enum name `stored` stands for.
    pub(crate) fn parse(stored: &str) -> Option<Self> {
        Some(match stored {
            "January" => Self::January,
            "February" => Self::February,
            "March" => Self::March,
            "April" => Self::April,
            "May" => Self::May,
            "June" => Self::June,
            "July" => Self::July,
            "August" => Self::August,
            "September" => Self::September,
            "October" => Self::October,
            "November" => Self::November,
            "December" => Self::December,
            _ => return None,
        })
    }
}

/// How an availability schedule repeats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = crate::closed_set::closed_string_set)]
pub(crate) enum RepeatType {
    /// The window repeats every year until the repeat end (`RepeatYearly`).
    RepeatYearly,
}

impl RepeatType {
    /// Returns the repeat type the stored enum name `stored` stands for.
    pub(crate) fn parse(stored: &str) -> Option<Self> {
        (stored == "RepeatYearly").then_some(Self::RepeatYearly)
    }
}

/// How a reputation requirement compares the standing of the player with the one it names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = crate::closed_set::closed_string_set)]
pub(crate) enum Comparison {
    /// The standing equals it (`EqualTo`).
    EqualTo,
    /// The standing is higher (`GreaterThan`).
    GreaterThan,
    /// The standing is at least as high (`GreaterThanOrEqualTo`).
    GreaterThanOrEqualTo,
    /// The standing is at most as high (`LessThanOrEqualTo`).
    LessThanOrEqualTo,
}

impl Comparison {
    /// Returns the comparison the stored enum name `stored` stands for.
    pub(crate) fn parse(stored: &str) -> Option<Self> {
        Some(match stored {
            "EqualTo" => Self::EqualTo,
            "GreaterThan" => Self::GreaterThan,
            "GreaterThanOrEqualTo" => Self::GreaterThanOrEqualTo,
            "LessThanOrEqualTo" => Self::LessThanOrEqualTo,
            _ => return None,
        })
    }
}
