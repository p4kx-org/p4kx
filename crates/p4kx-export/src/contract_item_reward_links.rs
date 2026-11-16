// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceUseRole, SourceValue, SourcedValue, StructValue};

use crate::catalog_read::original::{OriginalFlowFields, OriginalValuePart};
use crate::mission_generator::value_ref;
use crate::mission_layers;
use crate::mission_results;
use crate::source_value_catalog::SourceValueRef;
use crate::{CatalogError, ExportAccountingError};

use super::{RESULT, is_item};

pub(crate) fn read<'source>(
    fields: &OriginalFlowFields<'_, '_, 'source>,
    contract: &StructValue<'source>,
    sub: Option<&StructValue<'source>>,
) -> Result<Vec<SourcedValue<SourceValueRef>>, CatalogError> {
    let block = mission_results::entry_results(contract, sub);
    if !mission_results::results(block).any(|value| value.type_name() == RESULT) {
        return Ok(Vec::new());
    }
    let mut selected = Vec::new();
    let results = mission_results::selected_result_fact(fields, contract, sub, |used| {
        selected
            .try_reserve(1)
            .map_err(CatalogError::SourceUseAllocation)?;
        selected.push(used);
        Ok(())
    })?
    .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
    let complete = selected
        .iter()
        .all(|used| used.role() != SourceUseRole::Attempted);
    let mut links = Vec::new();
    for value in mission_layers::items_value(results.value())
        .iter()
        .filter(|value| is_item(value))
    {
        fields.check_cancel()?;
        let fact = fields.fact(value)?;
        let mut datum = fact.bind(
            OriginalValuePart::Value,
            SourceValue::Present(value_ref(fact)),
            SourceUseRole::Selector,
            complete,
        )?;
        datum.contribute(fact.observe(OriginalValuePart::ConcreteType, SourceUseRole::Selector)?);
        for used in &selected {
            datum.contribute(used.clone());
        }
        links
            .try_reserve(1)
            .map_err(CatalogError::SourceUseAllocation)?;
        links.push(datum.into_cell(fields.sources()));
    }
    Ok(links)
}
