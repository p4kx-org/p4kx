// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! What a mission broker entry asks of the player and grants: the reputation requirements and
//! results.

use p4kx_extraction::StructValue;
use schemars::JsonSchema;
use serde::Serialize;

use crate::broker_values::Comparison;
use crate::contract::every_field_required;
use crate::guid_text::CatalogGuid;
use crate::mission_index::MissionIndex;
use crate::mission_layers::one;
use crate::record_fields::{linked, structs, text};
use crate::template_issues::{TemplateError, Unknowns};

const REQUIREMENTS: &str = "SReputationMissionRequirementsParams";
const GIVER_REQUIREMENT: &str = "SReputationMissionGiverRequirementParams";
const AND: &str = "SReputationMissionRequirementExpression_And";
const LEFT_PARENTHESIS: &str = "SReputationMissionRequirementExpression_LeftParenthesis";
const RIGHT_PARENTHESIS: &str = "SReputationMissionRequirementExpression_RightParenthesis";

/// One element of the reputation requirement expression of an entry, in stored order, tagged by
/// `type`. The elements form an infix expression: requirements joined by `and`, grouped by
/// parentheses.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(tag = "type", rename_all = "camelCase")]
pub(crate) enum StandingTerm {
    /// `SReputationMissionGiverRequirementParams`: the standing in a scope of a faction compared
    /// with a named standing.
    #[serde(rename_all = "camelCase")]
    Requirement {
        /// The `comparison`.
        comparison: Option<Comparison>,
        /// The `factionReputation`, an entry of `factionReputations`.
        faction_reputation_guid: Option<CatalogGuid>,
        /// The `reputationScope`, an entry of `reputationScopes`.
        scope_guid: Option<CatalogGuid>,
        /// The `standing`, an entry of `reputationStandings`.
        standing_guid: Option<CatalogGuid>,
    },
    /// `SReputationMissionRequirementExpression_And`.
    And,
    /// `SReputationMissionRequirementExpression_LeftParenthesis`.
    LeftParenthesis,
    /// `SReputationMissionRequirementExpression_RightParenthesis`.
    RightParenthesis,
}

/// A reputation amount an entry grants.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct ReputationAmount {
    /// The `factionReputation`, an entry of `factionReputations`.
    pub(crate) faction_reputation_guid: Option<CatalogGuid>,
    /// The `reputationScope`, an entry of `reputationScopes`.
    pub(crate) scope_guid: Option<CatalogGuid>,
    /// The reward Record the amount is read from.
    pub(crate) reward_guid: Option<CatalogGuid>,
    /// The `reputationAmount` of the reward Record; `null` when it cannot be read.
    pub(crate) amount: Option<i64>,
}

/// One element of `missionResultReputationRewards`: the amounts granted for one result of the
/// mission.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct ReputationResult {
    /// The position in the stored array; the Archive names no result.
    pub(crate) index: usize,
    /// The `reputationAmounts` in stored order.
    pub(crate) amounts: Vec<ReputationAmount>,
}

/// Reads the reputation requirement expression of `root`.
///
/// # Errors
///
/// Returns [`TemplateError::UnknownStruct`] for a requirement block or an element of a struct the
/// catalog does not know.
pub(crate) fn requirements(
    unknowns: &Unknowns,
    root: Option<&StructValue<'_>>,
) -> Result<Vec<StandingTerm>, TemplateError> {
    let Some(params) = one(root, "reputationRequirements") else {
        return Ok(Vec::new());
    };
    if params.type_name() != REQUIREMENTS {
        return Err(TemplateError::unknown("reputationRequirements", params));
    }
    structs(Some(params), "expression")
        .map(|term| {
            let parent = Some(term);
            Ok(match term.type_name() {
                GIVER_REQUIREMENT => StandingTerm::Requirement {
                    comparison: unknowns.parse(
                        "comparison",
                        text(parent, "comparison"),
                        Comparison::parse,
                    ),
                    faction_reputation_guid: linked(parent, "factionReputation"),
                    scope_guid: linked(parent, "reputationScope"),
                    standing_guid: linked(parent, "standing"),
                },
                AND => StandingTerm::And,
                LEFT_PARENTHESIS => StandingTerm::LeftParenthesis,
                RIGHT_PARENTHESIS => StandingTerm::RightParenthesis,
                _ => {
                    return Err(TemplateError::unknown(
                        "reputationRequirements expression",
                        term,
                    ));
                }
            })
        })
        .collect()
}

/// Reads the `missionResultReputationRewards` of `root`; the amounts come from the reward Records
/// of `index`.
pub(crate) fn results(
    index: &MissionIndex<'_>,
    root: Option<&StructValue<'_>>,
) -> Vec<ReputationResult> {
    structs(root, "missionResultReputationRewards")
        .enumerate()
        .map(|(position, result)| ReputationResult {
            index: position,
            amounts: structs(Some(result), "reputationAmounts")
                .map(|amount| {
                    let amount = Some(amount);
                    let reward = linked(amount, "reward");
                    ReputationAmount {
                        faction_reputation_guid: linked(amount, "factionReputation"),
                        scope_guid: linked(amount, "reputationScope"),
                        reward_guid: reward,
                        amount: reward.and_then(|reward| index.reward(reward.0)),
                    }
                })
                .collect(),
        })
        .collect()
}
