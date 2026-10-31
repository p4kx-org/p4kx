// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

use crate::contract::{LocalisedText, every_field_required};
use crate::guid_text::CatalogGuid;
use crate::spawn_model::TagPaths;
use crate::template_property::ObjectiveProperty;

/// The state of an objective token or of the mission, as the mission flow, the comms
/// notifications and the flow actions name it. A value the catalog does not know is `null`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = crate::closed_set::closed_string_set)]
pub(crate) enum ObjectiveState {
    /// The objective is running.
    Active,
    /// The objective succeeded.
    Completed,
    /// The objective failed.
    Failed,
    /// The player abandoned the objective.
    Abandoned,
}

impl ObjectiveState {
    /// Returns the state the stored enum name `stored` stands for.
    pub(crate) fn parse(stored: &str) -> Option<Self> {
        Some(match stored {
            "Active" => Self::Active,
            "Completed" => Self::Completed,
            "Failed" => Self::Failed,
            "Abandoned" => Self::Abandoned,
            _ => return None,
        })
    }
}

/// The category of an objective display. A value the catalog does not know is `null`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = crate::closed_set::closed_string_set)]
pub(crate) enum ObjectiveCategory {
    /// A primary objective.
    Primary,
}

impl ObjectiveCategory {
    /// Returns the category the stored enum name `stored` stands for.
    pub(crate) fn parse(stored: &str) -> Option<Self> {
        (stored == "Primary").then_some(Self::Primary)
    }
}

/// When a comms notification is shown relative to its HUD notification. A value the catalog
/// does not know is `null`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = crate::closed_set::closed_string_set)]
pub(crate) enum CommsTiming {
    /// Both at once.
    Simultaneous,
    /// The comms message first.
    CommsNotificationFirst,
    /// The HUD notification first.
    HudNotificationFirst,
}

impl CommsTiming {
    /// Returns the timing the stored enum name `stored` stands for.
    pub(crate) fn parse(stored: &str) -> Option<Self> {
        Some(match stored {
            "Simultaneous" => Self::Simultaneous,
            "CommsNotificationFirst" => Self::CommsNotificationFirst,
            "HUDNotificationFirst" => Self::HudNotificationFirst,
            _ => return None,
        })
    }
}

/// The display info of an objective or of one of its sub objectives: the texts and flags as
/// stored.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct ObjectiveDisplay<'a> {
    /// The `category`; `null` when absent or a value the catalog does not know.
    pub(crate) category: Option<ObjectiveCategory>,
    /// The `hideOnHUD` flag, `null` when absent.
    pub(crate) hide_on_hud: Option<bool>,
    /// The `objectiveMarkerLabel`.
    #[schemars(extend("x-gameText" = true))]
    pub(crate) marker_label: LocalisedText<'a>,
    /// The `shortDescription`.
    #[schemars(extend("x-gameText" = true))]
    pub(crate) short_description: LocalisedText<'a>,
    /// The `longDescription`.
    #[schemars(extend("x-gameText" = true))]
    pub(crate) long_description: LocalisedText<'a>,
}

/// The title and description an objective overrides in the mission details.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct OverrideDisplay<'a> {
    /// The `titleOverride`.
    #[schemars(extend("x-gameText" = true))]
    pub(crate) title_override: LocalisedText<'a>,
    /// The `descriptionOverride`.
    #[schemars(extend("x-gameText" = true))]
    pub(crate) description_override: LocalisedText<'a>,
}

/// The `Hauling_ObjectiveSettings` of a hauling handler: the extended text tokens and the display
/// info of each way the objective is phrased.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct HaulingSettings<'a> {
    /// The `itemExtendedTextToken`.
    pub(crate) item_token: Option<String>,
    /// The `pickupLocationExtendedTextToken`.
    pub(crate) pickup_location_token: Option<String>,
    /// The `amountExtendedTextToken`.
    pub(crate) amount_token: Option<String>,
    /// The `totalExtendedTextToken`.
    pub(crate) total_token: Option<String>,
    /// The `dropOffLocationExtendedTextToken`.
    pub(crate) drop_off_location_token: Option<String>,
    /// The `andObjective`.
    pub(crate) and_objective: Option<ObjectiveDisplay<'a>>,
    /// The `orObjective`.
    pub(crate) or_objective: Option<ObjectiveDisplay<'a>>,
    /// The `collectObjective`.
    pub(crate) collect_objective: Option<ObjectiveDisplay<'a>>,
    /// The `collectAndDeliverObjective`.
    pub(crate) collect_and_deliver_objective: Option<ObjectiveDisplay<'a>>,
    /// The `resourceDeliverObjective`.
    pub(crate) resource_deliver_objective: Option<ObjectiveDisplay<'a>>,
    /// The `itemDeliverObjective`.
    pub(crate) item_deliver_objective: Option<ObjectiveDisplay<'a>>,
    /// The `listItemObjective`.
    pub(crate) list_item_objective: Option<ObjectiveDisplay<'a>>,
}

/// The `navPointSpawnInfo` of a handler: the entity the game spawns as the marker.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct NavPoint<'a> {
    /// The `entityClassDefinition` Record, an entry of `items`.
    pub(crate) entity_class_guid: Option<CatalogGuid>,
    /// The `name` of the marker.
    #[schemars(extend("x-gameText" = true))]
    pub(crate) display_name: LocalisedText<'a>,
    /// The `extendedTextTokens` in stored order.
    pub(crate) extended_text_tokens: Vec<String>,
}

/// The fields of a `TransitToInstance` handler.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct InstanceEntry<'a> {
    /// The `completePhaseOnPlayerEnterInstance` flag.
    pub(crate) complete_phase_on_player_enter: Option<bool>,
    /// The `instanceEnterTravelRadiusKM`.
    pub(crate) enter_travel_radius_km: Option<f32>,
    /// The `instanceHUDObjectiveText`.
    #[schemars(extend("x-gameText" = true))]
    pub(crate) hud_objective_text: LocalisedText<'a>,
    /// The tag path of the `instanceTagIdentifier`.
    pub(crate) tag_identifier: Option<String>,
    /// The `ocTagsToSearchInstanceEnter` as tag paths.
    pub(crate) search_tags: TagPaths,
    /// The `transitToInstanceObjectiveMarker`.
    pub(crate) objective_marker: Option<String>,
}

/// An objective handler: the struct and the settings its fields hold. A field the struct does not
/// have is `null`; the orders of a hauling handler are `haulingOrders` of the template.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct ObjectiveHandler<'a> {
    /// The struct, e.g. `ObjectiveHandler_Hauling`.
    pub(crate) struct_name: String,
    /// The `module` file of the mission scripting, `null` when empty.
    pub(crate) module: Option<String>,
    /// The `moduleHierarchy` Record, `null` when absent.
    pub(crate) module_hierarchy_guid: Option<CatalogGuid>,
    /// The `disableTravelObjectives` flag.
    pub(crate) disable_travel_objectives: Option<bool>,
    /// The `disableReturnObjectives` flag.
    pub(crate) disable_return_objectives: Option<bool>,
    /// The `travelRadiusKM`.
    pub(crate) travel_radius_km: Option<f32>,
    /// The `allPlayersLeftGracePeriod`, as stored.
    pub(crate) all_players_left_grace_period: Option<f32>,
    /// The `missionVariableName` of the property the `location` points to.
    pub(crate) location_property: Option<String>,
    /// The `missionVariableName` of the property the `organization` points to.
    pub(crate) organization_property: Option<String>,
    /// The `missionVariableName` of the property the `haulingLimitProperty` points to.
    pub(crate) hauling_limit_property: Option<String>,
    /// The `missionVariableName` of the property the `targetObject` points to.
    pub(crate) target_property: Option<String>,
    /// The `minDistance` of a near-location handler.
    pub(crate) min_distance: Option<i64>,
    /// The `maxDistance` of a near-location handler.
    pub(crate) max_distance: Option<i64>,
    /// The `hideMarkers` flag of a hauling handler.
    pub(crate) hide_markers: Option<bool>,
    /// The `securityManifestToOverride` Record.
    pub(crate) security_manifest_guid: Option<CatalogGuid>,
    /// The `navPointSpawnInfo`, `null` when absent.
    pub(crate) nav_point: Option<NavPoint<'a>>,
    /// The `travelObjectiveInfo`, `null` when absent.
    pub(crate) travel_objective: Option<ObjectiveDisplay<'a>>,
    /// The `returnObjectiveInfo`, `null` when absent.
    pub(crate) return_objective: Option<ObjectiveDisplay<'a>>,
    /// The `objectiveSettings` of a hauling handler.
    pub(crate) hauling: Option<HaulingSettings<'a>>,
    /// The fields of a `TransitToInstance` handler.
    pub(crate) instance: Option<InstanceEntry<'a>>,
}

/// The `rewardContribution` of an objective: how much of the reward it stands for.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct RewardContribution {
    /// The struct, e.g. `ObjectiveRewardContribution_PercentOfMission`.
    pub(crate) struct_name: String,
    /// The `percentContribution`.
    pub(crate) percent_contribution: Option<i64>,
    /// The `doPartialRewardPayout` flag.
    pub(crate) do_partial_reward_payout: Option<bool>,
}

/// A `CommsNotificationSelector` of an objective.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct ObjectiveComms {
    /// The state of the objective that triggers the message.
    pub(crate) trigger: Option<ObjectiveState>,
    /// The `commsTiming`.
    pub(crate) timing: Option<CommsTiming>,
    /// The `delay`, as stored.
    pub(crate) delay: Option<i64>,
    /// The tag path of the `communicationTags` tag.
    pub(crate) communication_tag: Option<String>,
}

/// An objective token of a template.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct TemplateObjective<'a> {
    /// The `id` of the token; unique within the template.
    pub(crate) id: CatalogGuid,
    /// The `id` of the token whose `childMissionPhases` hold this one, `null` at the top level.
    pub(crate) parent_id: Option<CatalogGuid>,
    /// The `debugName`, `null` when empty.
    pub(crate) debug_name: Option<String>,
    /// The `startsActive` flag, `null` when absent.
    pub(crate) starts_active: Option<bool>,
    /// The `variableName` of a child mission phase, `null` for a top level token.
    pub(crate) variable_name: Option<String>,
    /// The `weight` of a child mission phase, `null` for a top level token.
    pub(crate) weight: Option<f32>,
    /// The tag path of the `missionPhaseIdentifierTag`, `null` when absent.
    pub(crate) phase_tag: Option<String>,
    /// The `objectiveHandler`, `null` when absent.
    pub(crate) handler: Option<ObjectiveHandler<'a>>,
    /// The `displayInfo`.
    pub(crate) display: ObjectiveDisplay<'a>,
    /// The `overrideMissionDetailsDisplayInfo`.
    pub(crate) override_display: OverrideDisplay<'a>,
    /// The `rewardContribution`, `null` when absent.
    pub(crate) reward_contribution: Option<RewardContribution>,
    /// The `properties` in stored order.
    pub(crate) objective_properties: Vec<ObjectiveProperty<'a>>,
    /// The `commsNotifications` in stored order.
    pub(crate) comms: Vec<ObjectiveComms>,
}
