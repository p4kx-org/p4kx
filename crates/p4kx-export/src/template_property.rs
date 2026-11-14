// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

use crate::broker_terms::MissionReward;
use crate::contract_hauling::HaulingRequirement;
use crate::guid_text::CatalogGuid;
use crate::mission_property_auto::AutoSpawns;
use crate::mission_property_investigation::Investigation;
use crate::mission_property_option::{FloatOption, IntegerOption, StringOption, TagOption};
use crate::mission_property_race::TimeTrialSplit;
use crate::mission_property_source::SourceTagList;
use crate::npc_spawn_model::AiName;
use crate::scoped_location::ScopedLocation;
use crate::spawn_model::{LocationCondition, TagPaths};

/// How an objective holds a mission property (`ObjectiveProperty_*`). A struct the catalog does
/// not know is `null`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = crate::schema_vocabulary::open_string_vocabulary)]
pub(crate) enum PropertyKind {
    /// The objective refers to a property of the template's `contractProperties`.
    Referenced,
    /// The objective produces the property.
    Output,
    /// The objective holds the property itself.
    Embedded,
    /// The objective takes the output of another objective's property.
    Input,
}

impl PropertyKind {
    /// Returns the kind the struct `stored` stands for.
    pub(crate) fn parse(stored: &str) -> Option<Self> {
        Some(match stored.strip_prefix("ObjectiveProperty_")? {
            "Referenced" => Self::Referenced,
            "Output" => Self::Output,
            "Embedded" => Self::Embedded,
            "Input" => Self::Input,
            _ => return None,
        })
    }
}

/// The value of a mission property (`MissionPropertyValue_*`), tagged by `type`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(tag = "type", rename_all = "camelCase")]
pub(crate) enum PropertyValue<'a> {
    /// `MissionPropertyValue_ScopedLocation` with all declared source filters.
    ScopedLocation(Box<ScopedLocation>),
    /// `MissionPropertyValue_AutoSpawns` without computing runtime spawns.
    AutoSpawns(Box<AutoSpawns>),
    /// `MissionPropertyValue_Investigation` with all authored clue variants.
    Investigation(Box<Investigation<'a>>),
    /// `MissionPropertyValue_Object`: an object the mission scripting provides.
    Object,
    /// `MissionPropertyValue_DeliveryOrder`: a delivery order the mission scripting provides.
    DeliveryOrder,
    /// `MissionPropertyValue_Boolean`.
    Boolean {
        /// The `value`.
        value: Option<bool>,
    },
    /// `MissionPropertyValue_StringHash`.
    StringHash {
        /// The `options` in stored order.
        options: Option<Vec<Option<StringOption<'a>>>>,
    },
    /// `MissionPropertyValue_Integer`.
    Integer {
        /// The `options` in stored order.
        options: Option<Vec<Option<IntegerOption<'a>>>>,
    },
    /// `MissionPropertyValue_Float`.
    Float {
        /// The `options` in stored order.
        options: Option<Vec<Option<FloatOption<'a>>>>,
    },
    /// `MissionPropertyValue_Tags`.
    #[serde(rename_all = "camelCase")]
    Tags {
        /// The `tags` as tag paths.
        tags: TagPaths,
        /// The `negativeTags` as tag paths.
        negative_tags: TagPaths,
        /// The source positive tag identities.
        source_tags: Option<SourceTagList>,
        /// The source negative tag identities.
        source_negative_tags: Option<SourceTagList>,
        /// The complete authored tag options in source order.
        options: Option<Vec<Option<TagOption<'a>>>>,
    },
    /// `MissionPropertyValue_ShipSpawnDescriptions`.
    #[serde(rename_all = "camelCase")]
    ShipSpawnDescriptions {
        /// The `guid` of the entry of `contractShipSpawns`; `null` for a description without a
        /// group.
        pool_guid: Option<CatalogGuid>,
    },
    /// `MissionPropertyValue_Location` and `MissionPropertyValue_Locations`.
    #[serde(rename_all = "camelCase")]
    Location {
        /// The `guid` of the entry of `contractLocationQueries`.
        query_guid: CatalogGuid,
    },
    /// `MissionPropertyValue_NPCSpawnDescriptions`.
    #[serde(rename_all = "camelCase")]
    NpcSpawnDescriptions {
        /// The `guid` of the entry of `contractNpcSpawns`; `null` for a description without a
        /// group.
        pool_guid: Option<CatalogGuid>,
    },
    /// `MissionPropertyValue_EntitySpawnDescriptions`.
    #[serde(rename_all = "camelCase")]
    EntitySpawnDescriptions {
        /// The `guid` of the entry of `contractEntitySpawns`; `null` for a description without a
        /// group.
        pool_guid: Option<CatalogGuid>,
    },
    /// `MissionPropertyValue_Organization`: the organisations a mission may pick from.
    #[serde(rename_all = "camelCase")]
    Organization {
        /// The `matchConditions`, as for a location query.
        conditions: Vec<LocationCondition>,
        /// The `debugForceChosenOrganization` Record (`MissionOrganization`), `null` when absent.
        forced_organization_guid: Option<CatalogGuid>,
    },
    /// `MissionPropertyValue_MissionItem`: the mission items a mission may pick from.
    #[serde(rename_all = "camelCase")]
    MissionItem {
        /// The `matchConditions`, as for a location query.
        conditions: Vec<LocationCondition>,
        /// The `debugForceChosenItem` Record (`MissionItem`), `null` when absent.
        forced_item_guid: Option<CatalogGuid>,
        /// The `minItemsToFind`.
        min_items_to_find: Option<i64>,
        /// The `maxItemsToFind`.
        max_items_to_find: Option<i64>,
    },
    /// `MissionPropertyValue_AIName`: the name an NPC gets.
    #[serde(rename = "aiName")]
    AiName(AiName<'a>),
    /// `MissionPropertyValue_CombinedDataSetEntries`: the properties whose data set entries the
    /// mission combines.
    #[serde(rename_all = "camelCase")]
    CombinedDataSetEntries {
        /// The `missionVariableName` of the properties of `dataSetEntryProperties`, in stored
        /// order.
        entry_properties: Vec<String>,
    },
    /// `MissionPropertyValue_HaulingOrders`: what the mission hauls, as in `haulingOrders`.
    HaulingOrders {
        /// The requirements of the `haulingOrderContent`.
        requirements: Vec<HaulingRequirement>,
    },
    /// `MissionPropertyValue_Reward`.
    Reward {
        /// The `rewardDef`, `null` when absent.
        reward: Option<MissionReward>,
    },
    /// `MissionPropertyValue_TimeTrialRace` without evaluating race results.
    #[serde(rename_all = "camelCase")]
    TimeTrialRace {
        /// The authored split structures in source order.
        target_splits: Option<Vec<Option<TimeTrialSplit>>>,
    },
    /// `MissionPropertyValue_StarMapLocation`: a star map location the mission scripting provides.
    StarMapLocation,
}
