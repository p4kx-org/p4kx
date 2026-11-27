// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

pub(crate) const STRING_OPTION_TYPE: &str = "MissionPropertyValueOption_StringHash";
pub(crate) const INTEGER_OPTION_TYPE: &str = "MissionPropertyValueOption_Integer";
pub(crate) const FLOAT_OPTION_TYPE: &str = "MissionPropertyValueOption_Float";
pub(crate) const TAGS_OPTION_TYPE: &str = "MissionPropertyValueOption_Tags";

/// The recognized property-value operation independently of its retained original source type.
#[derive(Debug, Clone, Copy, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(extend("x-p4kx-vocabularyClass" = "control"))]
pub(crate) enum PropertyValueKind {
    AutoSpawns,
    Investigation,
    StringHash,
    Integer,
    Float,
    Tags,
    TimeTrialRace,
    ScopedLocation,
    Object,
    DeliveryOrder,
    Boolean,
    Organization,
    MissionItem,
    AiName,
    CombinedDataSetEntries,
    HaulingOrders,
    Reward,
    StarMapLocation,
    ShipSpawnDescriptions,
    NpcSpawnDescriptions,
    EntitySpawnDescriptions,
    Location,
    Locations,
}

impl PropertyValueKind {
    pub(crate) const fn option_type(self) -> Option<&'static str> {
        match self {
            Self::StringHash => Some(STRING_OPTION_TYPE),
            Self::Integer => Some(INTEGER_OPTION_TYPE),
            Self::Float => Some(FLOAT_OPTION_TYPE),
            Self::Tags => Some(TAGS_OPTION_TYPE),
            _ => None,
        }
    }

    pub(crate) fn read(name: &str) -> Option<Self> {
        delegated(name)
            .or_else(|| suffix(name))
            .or_else(|| spawn(name))
    }

    pub(crate) const fn is_spawn(self) -> bool {
        matches!(
            self,
            Self::ShipSpawnDescriptions
                | Self::NpcSpawnDescriptions
                | Self::EntitySpawnDescriptions
                | Self::Location
                | Self::Locations
        )
    }
}

fn delegated(name: &str) -> Option<PropertyValueKind> {
    Some(match name {
        "MissionPropertyValue_AutoSpawns" => PropertyValueKind::AutoSpawns,
        "MissionPropertyValue_Investigation" => PropertyValueKind::Investigation,
        "MissionPropertyValue_StringHash" => PropertyValueKind::StringHash,
        "MissionPropertyValue_Integer" => PropertyValueKind::Integer,
        "MissionPropertyValue_Float" => PropertyValueKind::Float,
        "MissionPropertyValue_Tags" => PropertyValueKind::Tags,
        "MissionPropertyValue_TimeTrialRace" => PropertyValueKind::TimeTrialRace,
        _ => return None,
    })
}

fn suffix(name: &str) -> Option<PropertyValueKind> {
    Some(
        match name.strip_prefix("MissionPropertyValue_").unwrap_or(name) {
            "ScopedLocation" => PropertyValueKind::ScopedLocation,
            "Object" => PropertyValueKind::Object,
            "DeliveryOrder" => PropertyValueKind::DeliveryOrder,
            "Boolean" => PropertyValueKind::Boolean,
            "Organization" => PropertyValueKind::Organization,
            "MissionItem" => PropertyValueKind::MissionItem,
            "AIName" => PropertyValueKind::AiName,
            "CombinedDataSetEntries" => PropertyValueKind::CombinedDataSetEntries,
            "HaulingOrders" => PropertyValueKind::HaulingOrders,
            "Reward" => PropertyValueKind::Reward,
            "StarMapLocation" => PropertyValueKind::StarMapLocation,
            _ => return None,
        },
    )
}

fn spawn(name: &str) -> Option<PropertyValueKind> {
    Some(match name {
        "MissionPropertyValue_ShipSpawnDescriptions" => PropertyValueKind::ShipSpawnDescriptions,
        "MissionPropertyValue_NPCSpawnDescriptions" => PropertyValueKind::NpcSpawnDescriptions,
        "MissionPropertyValue_EntitySpawnDescriptions" => {
            PropertyValueKind::EntitySpawnDescriptions
        }
        "MissionPropertyValue_Location" => PropertyValueKind::Location,
        "MissionPropertyValue_Locations" => PropertyValueKind::Locations,
        _ => return None,
    })
}
