// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

use crate::contract::{LocalisedText, every_field_required};
use crate::guid_text::CatalogGuid;
use crate::spawn_model::{LocationCondition, TagPaths};

/// How an objective holds a mission property (`ObjectiveProperty_*`). A struct the catalog does
/// not know is `null`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = crate::closed_set::closed_string_set)]
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

/// A property of an objective token.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct ObjectiveProperty<'a> {
    /// How the objective holds the property.
    pub(crate) kind: Option<PropertyKind>,
    /// The `missionVariableName` of the property, `null` when empty.
    pub(crate) variable: Option<String>,
    /// The `extendedTextToken` of the property, `null` when empty.
    pub(crate) token: Option<String>,
    /// The `missionVariableName` of the property of the template the `property` of a referenced
    /// property points to; `null` for any other kind or a pointer to nothing.
    pub(crate) contract_property: Option<String>,
    /// The value of an output, embedded or input property, `null` for a referenced one.
    pub(crate) value: Option<PropertyValue<'a>>,
    /// The output an input property takes, `null` for any other kind.
    pub(crate) linked_output: Option<LinkedOutput>,
}

/// The output property an input property takes the value of.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct LinkedOutput {
    /// The `id` of the objective that holds the output, an entry of `objectives`; `null` when the
    /// pointer does not name one.
    pub(crate) objective_id: Option<CatalogGuid>,
    /// The `missionVariableName` of the output.
    pub(crate) variable: Option<String>,
}

/// An option of a string hash property.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct StringOption<'a> {
    /// The `textId`.
    #[schemars(extend("x-gameText" = true))]
    pub(crate) display_name: LocalisedText<'a>,
    /// The `value`, `null` when empty.
    pub(crate) value: Option<String>,
    /// The `weighting` of the option among the others.
    pub(crate) weighting: Option<i64>,
    /// The `missionVariableName` of the properties the option makes dependent.
    pub(crate) dependent_properties: Vec<String>,
}

/// An option of an integer or float property.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct NumberOption<'a> {
    /// The `textId`.
    #[schemars(extend("x-gameText" = true))]
    pub(crate) display_name: LocalisedText<'a>,
    /// The `value`, as stored.
    pub(crate) value: Option<f32>,
    /// The `variation` the game applies around the value, as stored.
    pub(crate) variation: Option<f32>,
    /// The `weighting` of the option among the others.
    pub(crate) weighting: Option<i64>,
    /// The `missionVariableName` of the properties the option makes dependent.
    pub(crate) dependent_properties: Vec<String>,
}

/// The value of a mission property (`MissionPropertyValue_*`), tagged by `type`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(tag = "type", rename_all = "camelCase")]
pub(crate) enum PropertyValue<'a> {
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
        options: Vec<StringOption<'a>>,
    },
    /// `MissionPropertyValue_Integer`.
    Integer {
        /// The `options` in stored order.
        options: Vec<NumberOption<'a>>,
    },
    /// `MissionPropertyValue_Float`.
    Float {
        /// The `options` in stored order.
        options: Vec<NumberOption<'a>>,
    },
    /// `MissionPropertyValue_Tags`.
    #[serde(rename_all = "camelCase")]
    Tags {
        /// The `tags` as tag paths.
        tags: TagPaths,
        /// The `negativeTags` as tag paths.
        negative_tags: TagPaths,
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
}
