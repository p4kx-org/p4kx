// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    CancelToken, DecimalU64, SourceUse, SourceUseRole, SourceUses, SourceValue, SourcedValue,
};
use schemars::JsonSchema;
use serde::Serialize;

use crate::schema_fields::every_field_required;
use crate::shop_inventory_source_read::{
    self as source, ParsedInventory, SourceMember, SourceNode, SourceNodeValue,
};
use crate::shop_registry;
use crate::{CatalogError, ExportAccountingError};

/// Typed navigation to known authored shop inventory fields in the same source arena.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct ShopInventoryFields {
    pub(super) shop_id: SourcedValue<DecimalU64>,
    pub(super) inventory: SourcedValue<InventoryLines>,
    pub(super) collection: SourcedValue<InventoryCollection>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct InventoryCollection {
    pub(super) inventory: SourcedValue<InventoryLines>,
}

type InventoryLines = Vec<SourcedValue<InventoryLine>>;

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct InventoryLine {
    pub(super) node: DecimalU64,
    pub(super) id: SourcedValue<InventoryId>,
    pub(super) buy_price: SourcedValue<DecimalU64>,
    pub(super) sell_price: SourcedValue<DecimalU64>,
    pub(super) current_inventory: SourcedValue<DecimalU64>,
    pub(super) max_inventory: SourcedValue<DecimalU64>,
    pub(super) rental_offerings: SourcedValue<DecimalU64>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct InventoryId {
    pub(super) alternatives: SourcedValue<Vec<SourcedValue<DecimalU64>>>,
}

#[derive(Clone, Copy)]
enum FieldPosition {
    NamedOnly,
    Sequence { index: usize, length: usize },
}

const ROOT_COLLECTION: FieldPosition = FieldPosition::Sequence {
    index: 0,
    length: 2,
};
const ROOT_INVENTORY: FieldPosition = FieldPosition::Sequence {
    index: 1,
    length: 2,
};
const ONLY_FIELD: FieldPosition = FieldPosition::Sequence {
    index: 0,
    length: 1,
};

struct View<'source, 'read> {
    parsed: &'source ParsedInventory<'read>,
    sources: &'source SourceUses,
    cancel: Option<&'source CancelToken>,
}

impl ShopInventoryFields {
    pub(super) fn read(
        parsed: &ParsedInventory<'_>,
        sources: &SourceUses,
        cancel: Option<&CancelToken>,
    ) -> Result<Self, CatalogError> {
        let view = View {
            parsed,
            sources,
            cancel,
        };
        let root = view.parsed.node(view.parsed.root)?;
        Ok(Self {
            shop_id: view.field(root, "ShopID", FieldPosition::NamedOnly, text_ref)?,
            inventory: view.field(root, "Inventory", ROOT_INVENTORY, |_, node| {
                view.lines(node)
            })?,
            collection: view.field(root, "Collection", ROOT_COLLECTION, |_, node| {
                view.collection(node)
            })?,
        })
    }

    pub(super) fn public_strings(
        &self,
        parsed: &ParsedInventory<'_>,
        cancel: Option<&CancelToken>,
    ) -> Result<Vec<usize>, CatalogError> {
        let mut result = Vec::new();
        admit_public(parsed, self.shop_id.value(), is_shop_id, &mut result)?;
        let collection = self
            .collection
            .value()
            .and_then(|collection| collection.inventory.value());
        for lines in [self.inventory.value(), collection].into_iter().flatten() {
            for line in lines {
                source::check_cancel(cancel)?;
                let Some(id) = line.value().and_then(|line| line.id.value()) else {
                    continue;
                };
                let Some(alternatives) = id.alternatives.value() else {
                    continue;
                };
                for alternative in alternatives {
                    source::check_cancel(cancel)?;
                    admit_public(
                        parsed,
                        alternative.value(),
                        shop_registry::is_guid,
                        &mut result,
                    )?;
                }
            }
        }
        result.sort_unstable();
        result.dedup();
        Ok(result)
    }
}

impl<'read> View<'_, 'read> {
    fn collection(
        &self,
        node: &SourceNode<'read>,
    ) -> Result<SourceValue<InventoryCollection>, CatalogError> {
        if !single_field_record(node) {
            return Ok(SourceValue::Invalid);
        }
        Ok(SourceValue::Present(InventoryCollection {
            inventory: self.field(node, "Inventory", ONLY_FIELD, |_, node| self.lines(node))?,
        }))
    }

    fn lines(&self, node: &SourceNode<'read>) -> Result<SourceValue<InventoryLines>, CatalogError> {
        let SourceNodeValue::Array(original) = &node.value else {
            return Ok(SourceValue::Invalid);
        };
        let mut lines = Vec::new();
        lines
            .try_reserve_exact(original.len())
            .map_err(CatalogError::JsonSourceAllocation)?;
        for &index in original {
            source::check_cancel(self.cancel)?;
            let element = self.parsed.node(index)?;
            let value = if single_field_record(element) {
                SourceValue::Present(self.line(index, element)?)
            } else if matches!(element.value, SourceNodeValue::Null) {
                SourceValue::Null
            } else {
                SourceValue::Invalid
            };
            lines.push(self.payload(value, element));
        }
        Ok(SourceValue::Present(lines))
    }

    fn line(&self, index: usize, node: &SourceNode<'read>) -> Result<InventoryLine, CatalogError> {
        Ok(InventoryLine {
            node: source::address(index)?,
            id: self.field(node, "ID", ONLY_FIELD, |_, node| self.id(node))?,
            buy_price: self.field(node, "BuyPrice", FieldPosition::NamedOnly, number_ref)?,
            sell_price: self.field(node, "SellPrice", FieldPosition::NamedOnly, number_ref)?,
            current_inventory: self.field(
                node,
                "CurrentInventory",
                FieldPosition::NamedOnly,
                number_ref,
            )?,
            max_inventory: self.field(
                node,
                "MaxInventory",
                FieldPosition::NamedOnly,
                number_ref,
            )?,
            rental_offerings: self.field(
                node,
                "RentalOfferings",
                FieldPosition::NamedOnly,
                |index, _| Ok(SourceValue::Present(source::address(index)?)),
            )?,
        })
    }

    fn id(&self, node: &SourceNode<'read>) -> Result<SourceValue<InventoryId>, CatalogError> {
        if !single_field_record(node) {
            return Ok(SourceValue::Invalid);
        }
        Ok(SourceValue::Present(InventoryId {
            alternatives: self.field(node, "ID", ONLY_FIELD, |_, node| self.alternatives(node))?,
        }))
    }

    fn alternatives(
        &self,
        node: &SourceNode<'read>,
    ) -> Result<SourceValue<Vec<SourcedValue<DecimalU64>>>, CatalogError> {
        let SourceNodeValue::Array(original) = &node.value else {
            return Ok(SourceValue::Invalid);
        };
        let mut alternatives = Vec::new();
        alternatives
            .try_reserve_exact(original.len())
            .map_err(CatalogError::JsonSourceAllocation)?;
        for &index in original {
            source::check_cancel(self.cancel)?;
            let node = self.parsed.node(index)?;
            let value = if matches!(node.value, SourceNodeValue::Null) {
                SourceValue::Null
            } else {
                text_ref(index, node)?
            };
            alternatives.push(self.payload(value, node));
        }
        Ok(SourceValue::Present(alternatives))
    }

    fn field<T>(
        &self,
        parent: &SourceNode<'read>,
        name: &str,
        position: FieldPosition,
        project: impl FnOnce(usize, &SourceNode<'read>) -> Result<SourceValue<T>, CatalogError>,
    ) -> Result<SourcedValue<T>, CatalogError> {
        match &parent.value {
            SourceNodeValue::Object(members) => self.named(parent, members, name, project),
            SourceNodeValue::Array(elements) => {
                self.positional(parent, elements, position, project)
            }
            _ => Ok(self.payload(SourceValue::Invalid, parent)),
        }
    }

    fn named<T>(
        &self,
        parent: &SourceNode<'read>,
        members: &[SourceMember<'read>],
        name: &str,
        project: impl FnOnce(usize, &SourceNode<'read>) -> Result<SourceValue<T>, CatalogError>,
    ) -> Result<SourcedValue<T>, CatalogError> {
        let mut matching = members
            .iter()
            .filter(|member| member.name.text() == Some(name));
        let Some(member) = matching.next() else {
            return self.absent(parent);
        };
        let node = self.parsed.node(member.value)?;
        let own = member_uses(member, node)?;
        if let Some(duplicate) = matching.next() {
            return self.repeated(own, std::iter::once(duplicate).chain(matching));
        }
        let value = field_value(member.value, node, project)?;
        Ok(self.sources.value(value, &own, true))
    }

    fn positional<T>(
        &self,
        parent: &SourceNode<'read>,
        elements: &[usize],
        position: FieldPosition,
        project: impl FnOnce(usize, &SourceNode<'read>) -> Result<SourceValue<T>, CatalogError>,
    ) -> Result<SourcedValue<T>, CatalogError> {
        let FieldPosition::Sequence { index, length } = position else {
            return self.absent(parent);
        };
        let Some(&index) = elements.get(index).filter(|_| elements.len() == length) else {
            return Ok(self.payload(SourceValue::Invalid, parent));
        };
        let node = self.parsed.node(index)?;
        let selector = parent.token.observe(SourceUseRole::Selector)?;
        let value = field_value(index, node, project)?;
        Ok(self
            .sources
            .value(value, &[selector, node.payload.clone()], true))
    }

    fn absent<T>(&self, parent: &SourceNode<'read>) -> Result<SourcedValue<T>, CatalogError> {
        let use_ = parent.token.observe(SourceUseRole::Selector)?;
        Ok(self.sources.value(SourceValue::Absent, &[use_], true))
    }

    fn repeated<'member, T>(
        &self,
        own: [SourceUse; 2],
        remaining: impl Iterator<Item = &'member SourceMember<'read>>,
    ) -> Result<SourcedValue<T>, CatalogError>
    where
        'read: 'member,
    {
        let mut uses = Vec::from(own);
        for member in remaining {
            source::check_cancel(self.cancel)?;
            let node = self.parsed.node(member.value)?;
            let next = member_uses(member, node)?;
            uses.try_reserve(next.len())
                .map_err(CatalogError::JsonSourceAllocation)?;
            uses.extend(next);
        }
        Ok(self.sources.value(SourceValue::Invalid, &uses, true))
    }

    fn payload<T>(&self, value: SourceValue<T>, node: &SourceNode<'read>) -> SourcedValue<T> {
        self.sources
            .value(value, std::slice::from_ref(&node.payload), true)
    }
}

const fn single_field_record(node: &SourceNode<'_>) -> bool {
    match &node.value {
        SourceNodeValue::Object(_) => true,
        SourceNodeValue::Array(elements) => elements.len() == 1,
        _ => false,
    }
}

fn field_value<'read, T>(
    index: usize,
    node: &SourceNode<'read>,
    project: impl FnOnce(usize, &SourceNode<'read>) -> Result<SourceValue<T>, CatalogError>,
) -> Result<SourceValue<T>, CatalogError> {
    if matches!(node.value, SourceNodeValue::Null) {
        Ok(SourceValue::Null)
    } else {
        project(index, node)
    }
}

fn member_uses(
    member: &SourceMember<'_>,
    node: &SourceNode<'_>,
) -> Result<[SourceUse; 2], CatalogError> {
    Ok([
        member.name_token.observe(SourceUseRole::Selector)?,
        node.payload.clone(),
    ])
}

fn admit_public(
    parsed: &ParsedInventory<'_>,
    reference: Option<&DecimalU64>,
    valid: impl FnOnce(&str) -> bool,
    result: &mut Vec<usize>,
) -> Result<(), CatalogError> {
    let Some(reference) = reference else {
        return Ok(());
    };
    let index = source::index(*reference)?;
    let SourceNodeValue::String(value) = &parsed.node(index)?.value else {
        return Err(ExportAccountingError::ConflictingSourceDatum.into());
    };
    if value.text().is_some_and(valid) {
        source::push(result, index)?;
    }
    Ok(())
}

fn is_shop_id(value: &str) -> bool {
    value
        .split(',')
        .all(|part| shop_registry::is_guid(part.trim()))
}

fn text_ref(index: usize, node: &SourceNode<'_>) -> Result<SourceValue<DecimalU64>, CatalogError> {
    match &node.value {
        SourceNodeValue::String(value) if value.text().is_some() => {
            Ok(SourceValue::Present(source::address(index)?))
        }
        _ => Ok(SourceValue::Invalid),
    }
}

fn number_ref(
    index: usize,
    node: &SourceNode<'_>,
) -> Result<SourceValue<DecimalU64>, CatalogError> {
    if matches!(node.value, SourceNodeValue::Number(_)) {
        Ok(SourceValue::Present(source::address(index)?))
    } else {
        Ok(SourceValue::Invalid)
    }
}
