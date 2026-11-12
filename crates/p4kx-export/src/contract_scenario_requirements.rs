// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;
use std::collections::BTreeSet;

use p4kx_extraction::{DataForge, DecimalU64, PhysicalSourceRef, RecordTree, SourcedValue, Value};
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_read::original::{OriginalFlowFields, OriginalValueEdge, OriginalValueFact};
use crate::catalog_record_reference::CatalogRecordReference;
use crate::catalog_table::{CatalogTable, DatasetAvailability};
use crate::schema_fields::every_field_required;
use crate::source_fields::{FieldCapture, FieldError};
use crate::source_value_catalog::{SourceRecordReference, SourceValueRef};
use crate::{CatalogError, ExportAccountingError};

const FIELD: &str = "required_active_scenarios";
const TARGET: &str = "MissionScenario";
const OWNERS: [&str; 3] = [
    "ContractGenerator",
    "ContractGeneratorHandlerBase",
    "ContractBase",
];

/// The physical generator arena and occurrence that declared the scenario list.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct RequirementSource {
    input_ref: PhysicalSourceRef,
    record_index: u32,
    owner: Option<SourceValueRef>,
    scenarios_value: SourceValueRef,
}

/// One ordered, authored scenario-reference list at its original owner.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct ContractScenarioRequirements<'source> {
    source: RequirementSource,
    #[schemars(with = "SourcedValue<Vec<SourcedValue<CatalogRecordReference>>>")]
    scenarios: SourcedValue<Vec<SourcedValue<Box<SourceRecordReference<'source>>>>>,
}

pub(crate) fn read<'source>(
    input: &OriginalCatalogInput<'_, 'source>,
    tree: &RecordTree<'source>,
    fields: &OriginalFlowFields<'_, '_, 'source>,
    capture: &dyn FieldCapture<'source>,
    notes: &mut Vec<FieldError>,
) -> Result<Vec<ContractScenarioRequirements<'source>>, CatalogError> {
    fields.check_tree(tree)?;
    let numbers = RefCell::new(Vec::new());
    let root = Observed::definition(fields, input, capture, &numbers);
    let mut rows = Vec::new();
    for declaration in fields.facts() {
        fields.check_cancel()?;
        if declaration.field().name() != FIELD
            || !matches!(declaration.edge(), OriginalValueEdge::Field { .. })
        {
            continue;
        }
        let owner = declaration
            .parent_ordinal()
            .map(|ordinal| original_owner(fields, ordinal))
            .transpose()?;
        let owner_index = match owner.map(OriginalValueFact::value) {
            Some(Value::Struct(source)) => source.struct_index(),
            None => tree.root().struct_index(),
            Some(_) => return Err(ExportAccountingError::ConflictingSourceDatum.into()),
        };
        if !is_requirement_owner(tree, owner_index)? {
            continue;
        }
        if !matches!(fields.field(owner, FIELD)?, Some((_, fact)) if std::ptr::eq(fact, declaration))
        {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        let observed = owner.map_or(root, |owner| root.at_owner(owner));
        rows.push(ContractScenarioRequirements {
            source: RequirementSource {
                input_ref: fields.input_ref().clone(),
                record_index: fields.record_index(),
                owner: owner.map(value_ref),
                scenarios_value: value_ref(declaration),
            },
            scenarios: observed.record_link_array_to(FIELD, TARGET, notes)?,
        });
    }
    Ok(rows)
}

fn is_requirement_owner(tree: &RecordTree<'_>, index: u32) -> Result<bool, CatalogError> {
    let mut declaration = tree
        .struct_at(index)
        .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
    let mut visited = BTreeSet::new();
    while visited.insert(declaration.index()) {
        if OWNERS.contains(&declaration.name().map_err(CatalogError::DataForge)?) {
            return Ok(true);
        }
        let Some(parent) = declaration.parent() else {
            return Ok(false);
        };
        declaration = parent;
    }
    Err(ExportAccountingError::ConflictingSourceDatum.into())
}

fn original_owner<'fields, 'scope, 'read, 'source>(
    fields: &'fields OriginalFlowFields<'scope, 'read, 'source>,
    ordinal: u64,
) -> Result<&'fields OriginalValueFact<'scope, 'read, 'source>, CatalogError> {
    let index = usize::try_from(ordinal).map_err(|_| ExportAccountingError::CounterOverflow)?;
    let owner = fields
        .facts()
        .get(index)
        .filter(|fact| fact.ordinal() == ordinal)
        .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
    Ok(owner)
}

const fn value_ref(fact: &OriginalValueFact<'_, '_, '_>) -> SourceValueRef {
    SourceValueRef {
        value_index: DecimalU64::new(fact.ordinal()),
    }
}

pub(crate) fn table<'source>(
    forge: &DataForge<'_>,
    rows: Vec<ContractScenarioRequirements<'source>>,
) -> CatalogTable<ContractScenarioRequirements<'source>> {
    let availability = if rows.is_empty() {
        declared_availability(forge)
    } else {
        DatasetAvailability::Available
    };
    CatalogTable { availability, rows }
}

fn declared_availability(forge: &DataForge<'_>) -> DatasetAvailability {
    let mut unavailable = false;
    for structure in forge.structs() {
        match structure.name() {
            Ok(name) if OWNERS.contains(&name) => {}
            Ok(_) => continue,
            Err(_) => {
                unavailable = true;
                continue;
            }
        }
        for property in structure.properties() {
            match property.name() {
                Ok(FIELD) => return DatasetAvailability::Available,
                Ok(_) => {}
                Err(_) => unavailable = true,
            }
        }
    }
    if unavailable {
        DatasetAvailability::Unavailable
    } else {
        DatasetAvailability::Absent
    }
}
