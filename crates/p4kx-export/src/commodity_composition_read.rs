// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceUseRole, SourceValue, SourceValueState, SourcedValue, StructValue};

use crate::catalog_origins::CatalogOrigins;
use crate::commodity_authoring_contract::CommodityComposition;
use crate::commodity_authoring_inputs;
use crate::commodity_binding_read::BindingFacts;
use crate::commodity_inputs::CommodityInputs;
use crate::resource_value_read::{ResourceContext, ResourceFields};
use crate::source_fields::FieldError;
use crate::{CatalogError, ExportAccountingError};

pub(crate) fn read<'source>(
    source: &StructValue<'source>,
    container: &str,
    record_index: u32,
    origins: &CatalogOrigins,
    context: &ResourceContext<'_>,
    notes: &mut Vec<FieldError>,
) -> Result<
    BindingFacts<SourcedValue<Vec<SourcedValue<CommodityComposition<'source>>>>>,
    CatalogError,
> {
    let fields = ResourceFields::new(source, container, context);
    let values = fields.array("defaultComposition", notes)?;
    let path = format!("{container}.defaultComposition");
    let mut inputs = CommodityInputs::new(origins);
    let source_role = if values.value().is_some() {
        SourceUseRole::Selector
    } else {
        role(values.state())
    };
    inputs.at(record_index, &path, source_role)?;
    inputs.require_complete(values.origin().source_set_complete());
    let mut rows = Vec::new();
    if let Some(values) = values.value().copied() {
        for (position, value) in values.iter().enumerate() {
            let item_path = format!("{path}[{position}]");
            let item = context.structure_element(value, source.type_name(), &item_path, notes)?;
            let mut element = CommodityInputs::new(origins);
            if let Some(source) = item.value() {
                let facts = composition(source, &item_path, record_index, origins, context, notes)?;
                element.extend(facts.inputs, facts.complete);
                rows.push(element.value(facts.value));
            } else {
                commodity_authoring_inputs::field(&mut element, record_index, &item_path, &item)?;
                let state = commodity_authoring_inputs::without_payload(item.state()).ok_or(
                    CatalogError::Accounting(ExportAccountingError::ConflictingFieldInspection),
                )?;
                let (used, complete) = element.into_evidence();
                rows.push(origins.uses.value(state, &used, complete));
                inputs.extend(used, complete);
                continue;
            }
            let (used, complete) = element.into_evidence();
            inputs.extend(used, complete);
        }
    }
    let (inputs, complete) = inputs.into_evidence();
    let state = commodity_authoring_inputs::without_payload(values.state())
        .unwrap_or(SourceValue::Present(rows));
    let value = origins.uses.value(state, &inputs, complete);
    Ok(BindingFacts {
        value,
        inputs,
        complete,
    })
}

const fn role(state: SourceValueState) -> SourceUseRole {
    match state {
        SourceValueState::Null | SourceValueState::Invalid => SourceUseRole::Payload,
        SourceValueState::Absent | SourceValueState::Unsupported => SourceUseRole::Selector,
        _ => SourceUseRole::Attempted,
    }
}

fn composition<'source>(
    source: &StructValue<'source>,
    path: &str,
    record_index: u32,
    origins: &CatalogOrigins,
    context: &ResourceContext<'_>,
    notes: &mut Vec<FieldError>,
) -> Result<BindingFacts<CommodityComposition<'source>>, CatalogError> {
    let fields = ResourceFields::new(source, path, context);
    let entry = fields.reference("entry", notes)?;
    let mut inputs = CommodityInputs::new(origins);
    inputs.at(record_index, path, SourceUseRole::Selector)?;
    inputs.at(
        record_index,
        &format!("{path}.$type"),
        SourceUseRole::Selector,
    )?;
    commodity_authoring_inputs::reference(
        &mut inputs,
        record_index,
        &format!("{path}.entry"),
        &entry,
    )?;
    Ok(BindingFacts::new(
        CommodityComposition {
            source_type: fields.source_type()?,
            entry,
        },
        inputs,
    ))
}
