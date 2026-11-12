// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::Kind;

/// The tokens that mark a development leftover: a Record whose record path or name holds one of
/// them as a whole word, ignoring ASCII case, is left out and counted as skipped.
///
/// Words are the runs of ASCII letters and digits, so `paint_test_01` holds `test` while
/// `contestant` does not.
pub const EXCLUDED_TOKENS: [&str; 6] = ["test", "template", "dummy", "debug", "deprecated", "dev"];

/// The leftover tokens of a contract template: [`EXCLUDED_TOKENS`] without `template`, which
/// live templates such as `handyman_template` carry.
const CONTRACT_TEMPLATE_EXCLUDED_TOKENS: [&str; 5] =
    ["test", "dummy", "debug", "deprecated", "dev"];

/// The extra tokens of a leftover ship.
///
/// A Record of the ships kind whose record path or name holds one of them as a whole word, ignoring
/// ASCII case, is left out and counted as skipped, as for [`EXCLUDED_TOKENS`]. The other kinds keep
/// their `display` and `temp` Records.
pub const SHIP_EXCLUDED_TOKENS: [&str; 3] = ["temp", "display", "hijacked"];

fn has_word(record_path: &str, name: &str, tokens: &[&str]) -> bool {
    [record_path, name].into_iter().any(|text| {
        text.split(|c: char| !c.is_ascii_alphanumeric())
            .any(|word| tokens.iter().any(|token| word.eq_ignore_ascii_case(token)))
    })
}

/// Returns whether a Record with `record_path` and `name` is a development leftover.
pub(crate) fn is_leftover(record_path: &str, name: &str) -> bool {
    has_word(record_path, name, &EXCLUDED_TOKENS)
}

/// Returns whether a source Record is excluded by the kind's development-leftover policy.
///
/// Contract templates permit `template`; ships also exclude [`SHIP_EXCLUDED_TOKENS`].
/// Type-only kinds retain every source Record of their exact type.
#[must_use]
pub fn is_kind_leftover(kind: Kind, record_path: &str, name: &str) -> bool {
    match kind {
        Kind::Tags
        | Kind::Factions
        | Kind::ReputationPerks
        | Kind::ReputationTriggerRecords
        | Kind::ReputationRewardAmounts
        | Kind::Infractions
        | Kind::InfractionSets
        | Kind::Jurisdictions
        | Kind::LocationClasses
        | Kind::LocationFunctions
        | Kind::LocationSizes
        | Kind::LocationClassRegistries
        | Kind::LocationFunctionRegistries
        | Kind::LocationSizeRegistries
        | Kind::InventoryContainers
        | Kind::StarmapTypes
        | Kind::StarmapPresets
        | Kind::StarmapAmenities => false,
        Kind::ContractTemplates => has_word(record_path, name, &CONTRACT_TEMPLATE_EXCLUDED_TOKENS),
        Kind::Ships => {
            is_leftover(record_path, name) || has_word(record_path, name, &SHIP_EXCLUDED_TOKENS)
        }
        _ => is_leftover(record_path, name),
    }
}
