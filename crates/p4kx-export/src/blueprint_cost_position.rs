// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceUseRole, SourceValue, SourcedValue, Value};

use super::BlueprintCostPosition;
use crate::blueprint_field_source::value_ref;
use crate::catalog_read::original::{
    OriginalFlowFields, OriginalValueEdge, OriginalValueFact, OriginalValuePart,
};
use crate::crafting_cost_read::SELECT;
use crate::{CatalogError, ExportAccountingError};

pub(super) fn tier_index(
    tier: &OriginalValueFact<'_, '_, '_>,
    fields: &OriginalFlowFields<'_, '_, '_>,
) -> Result<SourcedValue<u32>, CatalogError> {
    if tier.field().name() != "tiers" {
        return Err(ExportAccountingError::ConflictingSourceDatum.into());
    }
    position_cell(tier, fields, original_index(tier)?, None)
}

pub(super) fn cost_position(
    cost: &OriginalValueFact<'_, '_, '_>,
    fields: &OriginalFlowFields<'_, '_, '_>,
) -> Result<SourcedValue<BlueprintCostPosition>, CatalogError> {
    let (position, owner) = match (cost.field().name(), cost.edge()) {
        ("mandatoryCost", OriginalValueEdge::Field { .. }) => {
            (BlueprintCostPosition::Mandatory, None)
        }
        ("optionalCosts", OriginalValueEdge::Element { ordinal }) => (
            BlueprintCostPosition::Optional {
                index: index(ordinal)?,
            },
            None,
        ),
        ("options", OriginalValueEdge::Element { ordinal }) => {
            let options = parent(fields, cost)?;
            let parent = parent(fields, options)?;
            if !matches!(options.edge(), OriginalValueEdge::Field { .. })
                || !matches!(parent.value(), Value::Struct(source) if source.type_name() == SELECT)
            {
                return Err(ExportAccountingError::ConflictingSourceDatum.into());
            }
            (
                BlueprintCostPosition::Option {
                    parent: value_ref(parent),
                    index: index(ordinal)?,
                },
                Some(parent),
            )
        }
        _ => return Err(ExportAccountingError::ConflictingSourceDatum.into()),
    };
    position_cell(cost, fields, position, owner)
}

fn position_cell<T>(
    fact: &OriginalValueFact<'_, '_, '_>,
    fields: &OriginalFlowFields<'_, '_, '_>,
    value: T,
    owner: Option<&OriginalValueFact<'_, '_, '_>>,
) -> Result<SourcedValue<T>, CatalogError> {
    let mut datum = fact.bind(
        OriginalValuePart::Value,
        SourceValue::Present(value),
        SourceUseRole::Conversion,
        true,
    )?;
    datum.contribute(fact.observe(OriginalValuePart::FieldName, SourceUseRole::Selector)?);
    if let Some(owner) = owner {
        datum.contribute(owner.observe(OriginalValuePart::ConcreteType, SourceUseRole::Selector)?);
    }
    Ok(datum.into_cell(fields.sources()))
}

fn parent<'fields, 'scope, 'read, 'source>(
    fields: &'fields OriginalFlowFields<'scope, 'read, 'source>,
    child: &OriginalValueFact<'_, '_, '_>,
) -> Result<&'fields OriginalValueFact<'scope, 'read, 'source>, CatalogError> {
    let parent = child
        .parent_ordinal()
        .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
    let index = usize::try_from(parent).map_err(|_| ExportAccountingError::CounterOverflow)?;
    fields
        .facts()
        .get(index)
        .ok_or_else(|| ExportAccountingError::ConflictingSourceDatum.into())
}

fn original_index(fact: &OriginalValueFact<'_, '_, '_>) -> Result<u32, CatalogError> {
    let OriginalValueEdge::Element { ordinal } = fact.edge() else {
        return Err(ExportAccountingError::ConflictingSourceDatum.into());
    };
    index(ordinal)
}

fn index(ordinal: u64) -> Result<u32, CatalogError> {
    u32::try_from(ordinal).map_err(|_| ExportAccountingError::CounterOverflow.into())
}
