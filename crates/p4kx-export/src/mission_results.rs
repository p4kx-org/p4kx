// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceUse, StructValue, Value};

use crate::blueprint::as_struct;
use crate::catalog_read::original::{OriginalFlowFields, OriginalValueFact};
use crate::guid_text::CatalogGuid;
use crate::mission_layers::{items, items_value, one};
use crate::record_fields::boolean;
use crate::{CatalogError, ExportAccountingError};

#[path = "mission_reputation_gains.rs"]
mod reputation_gains;
#[path = "mission_run_terms.rs"]
mod run_terms;

pub(crate) use self::reputation_gains::gains;
pub(crate) use self::run_terms::RunTerms;

const RESULT_BLOCK: &str = "contractResults";

type ResultFact<'fields, 'scope, 'read, 'source> =
    &'fields OriginalValueFact<'scope, 'read, 'source>;

/// Selects the effective result block and observes the consulted sub-contract and parent inputs.
pub(crate) fn selected_result_fact<'fields, 'scope, 'read, 'source>(
    fields: &'fields OriginalFlowFields<'scope, 'read, 'source>,
    parent: &StructValue<'source>,
    sub: Option<&StructValue<'source>>,
    mut retain: impl FnMut(SourceUse) -> Result<(), CatalogError>,
) -> Result<Option<ResultFact<'fields, 'scope, 'read, 'source>>, CatalogError> {
    if let Some(sub) = sub
        && let Some(results) = selected_result_list(fields, sub, &mut retain)?
        && !items_value(results.value()).is_empty()
    {
        return Ok(Some(results));
    }
    selected_result_list(fields, parent, &mut retain)
}

fn selected_result_list<'fields, 'scope, 'read, 'source>(
    fields: &'fields OriginalFlowFields<'scope, 'read, 'source>,
    owner: &StructValue<'source>,
    retain: &mut impl FnMut(SourceUse) -> Result<(), CatalogError>,
) -> Result<Option<ResultFact<'fields, 'scope, 'read, 'source>>, CatalogError> {
    let owner = fields
        .owner_of(owner)?
        .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
    let Some(block) = fields.observe_struct_selection(owner, RESULT_BLOCK, &mut *retain)? else {
        return Ok(None);
    };
    fields.observe_field_selection(block, RESULT_BLOCK, retain)
}

/// Returns the `contractResults` block an entry reads: the sub contract's when it states results
/// of its own, else the contract's.
pub(crate) fn entry_results<'t, 'f>(
    contract: &'t StructValue<'f>,
    sub: Option<&'t StructValue<'f>>,
) -> Option<&'t StructValue<'f>> {
    sub.and_then(|sub| one(Some(sub), "contractResults"))
        .filter(|block| !items(Some(block), "contractResults").is_empty())
        .or_else(|| one(Some(contract), "contractResults"))
}

/// Returns the link in the field `name` of `parent`: the GUID and the Record name of its target.
pub(crate) fn link<'f>(parent: &StructValue<'f>, name: &str) -> Option<(CatalogGuid, &'f str)> {
    match parent.field(name)? {
        Value::Reference(target) if target.location().is_none() => {
            let path = target.record_path();
            Some((
                CatalogGuid(target.guid()),
                path.rsplit('/').next().unwrap_or(path),
            ))
        }
        _ => None,
    }
}

/// Returns the results of the `contractResults` block `block`, in stored order.
pub(crate) fn results<'t, 'f>(
    block: Option<&'t StructValue<'f>>,
) -> impl Iterator<Item = &'t StructValue<'f>> {
    items(block, "contractResults").iter().filter_map(as_struct)
}

pub(crate) fn succeeds(result: &StructValue<'_>) -> bool {
    match items(Some(result), "missionResults").first() {
        Some(Value::Bool(set)) => *set,
        Some(Value::Struct(flag)) => boolean(Some(flag), "value").unwrap_or(false),
        _ => false,
    }
}
