// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataForge, StructValue};
use schemars::JsonSchema;
use serde::Serialize;

use crate::contract::every_field_required;
use crate::guid_text::CatalogGuid;
use crate::mission_layers::one;
use crate::record_fields::{boolean, integer, linked, number, structs, text};
use crate::spawn_model::TagTerm;
use crate::spawn_read::Reader;

/// The Record that owns a template: the team or developer that maintains it.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct TemplateOwner {
    /// The GUID of the `DevOwner` Record.
    pub(crate) guid: CatalogGuid,
    /// The Record name without its type, e.g. `MDT`; `null` when the Archive lacks the Record.
    pub(crate) class_name: Option<String>,
}

/// The `PartialContractRewards` of a template: what an abandoned or partly completed contract
/// still pays.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct PartialPayout {
    /// The `percentageThresholds` in stored order.
    pub(crate) thresholds: Vec<PayoutRange>,
}

/// A `PartialContractRewardRange`: the share of the reward paid between two completion percentages.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct PayoutRange {
    /// The `minPercentage` of the contract that is completed.
    pub(crate) min_percentage: Option<i64>,
    /// The `maxPercentage` of the contract that is completed.
    pub(crate) max_percentage: Option<i64>,
    /// The `currencyRewardMultiplier` applied to the payout, as stored.
    pub(crate) currency_reward_multiplier: Option<f32>,
    /// The `reputationMultipliers` in stored order.
    pub(crate) reputation_multipliers: Vec<PayoutReputation>,
}

/// A `PartialContractRewardRepAdjustment`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct PayoutReputation {
    /// The `reputationScope`, an entry of `reputationScopes`.
    pub(crate) scope_guid: Option<CatalogGuid>,
    /// The `reputationRewardMultiplier` applied to the gain in that scope, as stored.
    pub(crate) reputation_reward_multiplier: Option<f32>,
}

/// A modifier of a template (`MissionModifier_*`), as stored.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(tag = "modifier", rename_all = "camelCase")]
pub(crate) enum TemplateModifier {
    /// `MissionModifier_SecurityClearance`.
    #[serde(rename_all = "camelCase")]
    SecurityClearance {
        /// The `modifierName`, `null` when empty.
        modifier_name: Option<String>,
        /// The `enabled` flag.
        enabled: Option<bool>,
        /// The `missionVariableName` of the `locationProperty`.
        location_property: Option<String>,
        /// The `clearanceToken` Record.
        clearance_token_guid: Option<CatalogGuid>,
    },
    /// `MissionModifier_RequestRentShip`.
    #[serde(rename_all = "camelCase")]
    RequestRentShip {
        /// The `modifierName`, `null` when empty.
        modifier_name: Option<String>,
        /// The `enabled` flag.
        enabled: Option<bool>,
        /// The `missionVariableName` of the `locationProperty`.
        location_property: Option<String>,
        /// The `itemRecordGUID`, the ship item that is rented; an entry of `items` or `ships`.
        item_guid: Option<CatalogGuid>,
        /// The `durationSeconds` of the rental.
        duration_seconds: Option<i64>,
        /// The `clearRentalOnFail` flag.
        clear_rental_on_fail: Option<bool>,
    },
    /// `MissionModifier_ShipKioskFilter`.
    #[serde(rename_all = "camelCase")]
    ShipKioskFilter {
        /// The `modifierName`, `null` when empty.
        modifier_name: Option<String>,
        /// The `enabled` flag.
        enabled: Option<bool>,
        /// The `missionVariableName` of the `locationProperty`.
        location_property: Option<String>,
        /// The `terms` of the `kioskFilterType`: a ship matches one of them.
        terms: Vec<TagTerm>,
    },
    /// A modifier of another struct.
    #[serde(rename_all = "camelCase")]
    Other {
        /// The struct name.
        struct_name: String,
        /// The `modifierName`, `null` when empty.
        modifier_name: Option<String>,
        /// The `enabled` flag.
        enabled: Option<bool>,
    },
}

/// The `contractStartCommsNotification`: the message sent when the contract starts.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct StartComms {
    /// The `delay`, as stored.
    pub(crate) delay: Option<i64>,
    /// The tag path of the `communicationTags` tag.
    pub(crate) communication_tag: Option<String>,
}

/// A `ContractEndCommsNotification`: a message sent when the contract ends.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct EndComms {
    /// How the contract ended; `null` for a value the catalog does not know.
    pub(crate) reason: Option<EndReason>,
    /// The `delay`, as stored.
    pub(crate) delay: Option<i64>,
    /// The tag path of the `communicationTags` tag.
    pub(crate) communication_tag: Option<String>,
}

/// How a contract ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = crate::closed_set::closed_string_set)]
pub(crate) enum EndReason {
    /// The player completed it (`Complete`).
    Complete,
    /// It failed (`Fail`).
    Fail,
    /// The player abandoned it (`Abandon`).
    Abandon,
}

impl EndReason {
    fn parse(stored: &str) -> Option<Self> {
        Some(match stored {
            "Complete" => Self::Complete,
            "Fail" => Self::Fail,
            "Abandon" => Self::Abandon,
            _ => return None,
        })
    }
}

/// Reads the owner of the Record whose root is `root`; the name comes from the Record in `forge`.
pub(crate) fn owner(
    forge: &DataForge<'_>,
    root: Option<&StructValue<'_>>,
) -> Option<TemplateOwner> {
    let guid = linked(root, "owner")?;
    let class_name = forge.record_by_guid(guid.0).and_then(|record| {
        let name = record.name().ok()?;
        let type_name = record.type_name().ok()?;
        Some(
            name.strip_prefix(type_name)
                .and_then(|rest| rest.strip_prefix('.'))
                .unwrap_or(name)
                .to_owned(),
        )
    });
    Some(TemplateOwner { guid, class_name })
}

/// Reads the `partialRewardPayout` of `root`.
pub(crate) fn partial_payout(root: Option<&StructValue<'_>>) -> Option<PartialPayout> {
    let payout = one(root, "partialRewardPayout")?;
    Some(PartialPayout {
        thresholds: structs(Some(payout), "percentageThresholds")
            .map(|range| PayoutRange {
                min_percentage: integer(Some(range), "minPercentage"),
                max_percentage: integer(Some(range), "maxPercentage"),
                currency_reward_multiplier: number(Some(range), "currencyRewardMultiplier"),
                reputation_multipliers: structs(Some(range), "reputationMultipliers")
                    .map(|adjustment| PayoutReputation {
                        scope_guid: linked(Some(adjustment), "reputationScope"),
                        reputation_reward_multiplier: number(
                            Some(adjustment),
                            "reputationRewardMultiplier",
                        ),
                    })
                    .collect(),
            })
            .collect(),
    })
}

/// Reads the `modifiers` of `root`.
pub(crate) fn modifiers(
    reader: &Reader<'_, '_>,
    root: Option<&StructValue<'_>>,
) -> Vec<TemplateModifier> {
    structs(root, "modifiers")
        .map(|modifier| {
            let parent = Some(modifier);
            let modifier_name = text(parent, "modifierName").map(str::to_owned);
            let enabled = boolean(parent, "enabled");
            let location_property = reader.property_variable(parent, "locationProperty");
            let name = modifier.type_name();
            match name.strip_prefix("MissionModifier_").unwrap_or(name) {
                "SecurityClearance" => TemplateModifier::SecurityClearance {
                    modifier_name,
                    enabled,
                    location_property,
                    clearance_token_guid: linked(parent, "clearanceToken"),
                },
                "RequestRentShip" => TemplateModifier::RequestRentShip {
                    modifier_name,
                    enabled,
                    location_property,
                    item_guid: linked(parent, "itemRecordGUID"),
                    duration_seconds: integer(parent, "durationSeconds"),
                    clear_rental_on_fail: boolean(parent, "clearRentalOnFail"),
                },
                "ShipKioskFilter" => TemplateModifier::ShipKioskFilter {
                    modifier_name,
                    enabled,
                    location_property,
                    terms: structs(one(parent, "kioskFilterType"), "terms")
                        .map(|term| TagTerm {
                            positive_tags: reader.tag_array(Some(term), "positiveTags"),
                            negative_tags: reader.tag_array(Some(term), "negativeTags"),
                        })
                        .collect(),
                },
                _ => TemplateModifier::Other {
                    struct_name: name.to_owned(),
                    modifier_name,
                    enabled,
                },
            }
        })
        .collect()
}

/// Reads the `contractStartCommsNotification` of `root`.
pub(crate) fn start_comms(
    reader: &Reader<'_, '_>,
    root: Option<&StructValue<'_>>,
) -> Option<StartComms> {
    let comms = one(root, "contractStartCommsNotification")?;
    Some(StartComms {
        delay: integer(Some(comms), "delay"),
        communication_tag: reader.tag_path(Some(comms), "communicationTags"),
    })
}

/// Reads the `contractEndCommsNotifications` of `root`.
pub(crate) fn end_comms(reader: &Reader<'_, '_>, root: Option<&StructValue<'_>>) -> Vec<EndComms> {
    structs(root, "contractEndCommsNotifications")
        .map(|comms| EndComms {
            reason: text(Some(comms), "reason").and_then(EndReason::parse),
            delay: integer(Some(comms), "delay"),
            communication_tag: reader.tag_path(Some(comms), "communicationTags"),
        })
        .collect()
}
