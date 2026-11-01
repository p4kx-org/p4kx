// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::StructValue;

use crate::contract_hauling::{HaulingItem, HaulingItemKind, HaulingRequirement};
use crate::record_fields::{integer, reference, structs, structure, text};

const ORDERS_PROPERTY: &str = "MissionPropertyValue_HaulingOrders";
const OR_CONTENT: &str = "HaulingOrderContent_Or";
const HAULING_HANDLER: &str = "ObjectiveHandler_Hauling";

/// A hauling orders property of one level: the variable it fills and what it asks for.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct HaulingProperty<'f> {
    pub(crate) name: Option<&'f str>,
    pub(crate) requirements: Vec<HaulingRequirement>,
}

/// Reads the hauling orders properties the array field `name` of `parent` holds (the
/// `propertyOverrides` of a contract, the `contractProperties` of a template).
pub(crate) fn property_orders<'f>(
    parent: Option<&StructValue<'f>>,
    name: &str,
) -> Vec<HaulingProperty<'f>> {
    structs(parent, name)
        .filter_map(|property| {
            let value = structure(Some(property), "value")?;
            (value.type_name() == ORDERS_PROPERTY).then(|| HaulingProperty {
                name: text(Some(property), "missionVariableName"),
                requirements: value_requirements(value),
            })
        })
        .collect()
}

/// Reads the requirements a `MissionPropertyValue_HaulingOrders` value states.
pub(crate) fn value_requirements(value: &StructValue<'_>) -> Vec<HaulingRequirement> {
    structs(Some(value), "haulingOrderContent")
        .filter_map(|content| requirement(content, false))
        .collect()
}

/// Reads the orders of the hauling objectives in the `objectiveTokens` of the Record `root`:
/// the orders that state a commodity or an entity themselves. Orders that only name a drop-off
/// or point at a property are left out; their content is the property's.
pub(crate) fn objective_orders(root: Option<&StructValue<'_>>) -> Vec<HaulingRequirement> {
    structs(root, "objectiveTokens")
        .filter_map(|token| structure(Some(token), "objectiveHandler"))
        .filter(|handler| handler.type_name() == HAULING_HANDLER)
        .flat_map(|handler| structs(Some(handler), "haulingOrders"))
        .filter_map(|order| requirement(order, true))
        .collect()
}

fn requirement(content: &StructValue<'_>, objective: bool) -> Option<HaulingRequirement> {
    if content.type_name() == OR_CONTENT {
        let options = structs(Some(content), "options")
            .map(|option| {
                structs(Some(option), "orders")
                    .filter_map(|order| item(order, objective))
                    .collect()
            })
            .collect();
        return Some(HaulingRequirement { options });
    }
    item(content, objective).map(|item| HaulingRequirement {
        options: vec![vec![item]],
    })
}

/// Reads one order. A mission item of an objective is a property reference, not a fact.
fn item(order: &StructValue<'_>, objective: bool) -> Option<HaulingItem> {
    let (_, suffix) = order.type_name().split_once('_')?;
    let kind = match suffix {
        "Resource" | "ResourceUnlimitedDropOff" => HaulingItemKind::Resource,
        "EntityClass" => HaulingItemKind::EntityClass,
        "EntityClasses" => HaulingItemKind::EntityClasses,
        "MissionItem" if !objective => HaulingItemKind::MissionItem,
        _ => return None,
    };
    let order = Some(order);
    let pieces = !matches!(kind, HaulingItemKind::Resource);
    Some(HaulingItem {
        kind,
        resource_guid: reference(order, "resource"),
        entity_class_guid: reference(order, "entityClass"),
        entity_classes_guid: reference(order, "haulingEntityClasses"),
        min_scu: integer(order, "minSCU"),
        max_scu: integer(order, "maxSCU"),
        max_container_size: integer(order, "maxContainerSize"),
        min_amount: integer(order, "minAmount").filter(|_| pieces),
        max_amount: integer(order, "maxAmount").filter(|_| pieces),
    })
}
