// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

use crate::contract::every_field_required;
use crate::guid_text::CatalogGuid;

/// What a contract has to haul, as one level of the contract states it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct HaulingOrder<'a> {
    /// Where the order is stated.
    pub(crate) source: HaulingSource,
    /// The `missionVariableName` of the `MissionPropertyValue_HaulingOrders` property of a
    /// `property` order; `null` for an `objective` order.
    pub(crate) variable_name: Option<&'a str>,
    /// What has to be hauled, all of it. A requirement with several `options` is satisfied by any
    /// one of them.
    pub(crate) requirements: Vec<HaulingRequirement>,
}

/// The level of the contract that states a hauling order. Of the property levels the nearest one
/// wins per `variableName`; the objective orders are those of the template, else of the broker
/// entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = crate::schema_vocabulary::open_string_vocabulary)]
pub(crate) enum HaulingSource {
    /// A hauling orders property in the overrides of the sub contract.
    SubContract,
    /// A hauling orders property in the overrides of the contract.
    Contract,
    /// A hauling orders property in the parameters of the handler.
    Handler,
    /// A hauling orders property in the `contractProperties` of the template.
    Template,
    /// An order of a hauling objective of the template.
    TemplateObjective,
    /// An order of a hauling objective of the `MissionBrokerEntry`.
    BrokerObjective,
}

/// One requirement of a hauling order: the alternatives of which one is hauled. Most
/// requirements have exactly one option.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct HaulingRequirement {
    /// The alternatives; each is a list of items that are all hauled.
    pub(crate) options: Vec<Vec<HaulingItem>>,
}

/// One thing to haul.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct HaulingItem {
    /// What kind of thing the item is.
    pub(crate) kind: HaulingItemKind,
    /// The GUID of the `ResourceType` Record of a `resource` item, the `subtypeGuid` of a
    /// commodity; `null` for another kind or when the item names none. A GUID of a `ResourceType`
    /// that no commodity of the catalog carries is kept.
    pub(crate) resource_guid: Option<CatalogGuid>,
    /// The GUID of the `EntityClassDefinition` Record of an `entityClass` item.
    pub(crate) entity_class_guid: Option<CatalogGuid>,
    /// The GUID of the `Hauling_EntityClasses` Record of an `entityClasses` item: a list of
    /// entity classes of which any one counts. No kind lists these Records.
    pub(crate) entity_classes_guid: Option<CatalogGuid>,
    /// The `minSCU` of a `resource` item, as stored.
    pub(crate) min_scu: Option<i64>,
    /// The `maxSCU` of a `resource` item, as stored; 0 when the order states no upper bound.
    pub(crate) max_scu: Option<i64>,
    /// The `maxContainerSize` of a `resource` item, as stored; -1 when it states no limit.
    pub(crate) max_container_size: Option<i64>,
    /// The `minAmount` of an item that counts pieces, as stored.
    pub(crate) min_amount: Option<i64>,
    /// The `maxAmount` of an item that counts pieces, as stored; 0 when it states no upper bound.
    pub(crate) max_amount: Option<i64>,
}

/// What a hauling item stands for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = crate::schema_vocabulary::open_string_vocabulary)]
pub(crate) enum HaulingItemKind {
    /// A commodity in SCU (`…_Resource`, `…_ResourceUnlimitedDropOff`).
    Resource,
    /// A number of one entity class (`…_EntityClass`).
    EntityClass,
    /// A number of entities out of a list (`…_EntityClasses`).
    EntityClasses,
    /// A mission item the game creates for the mission (`HaulingOrderContent_MissionItem`).
    MissionItem,
}
