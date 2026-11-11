// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;

use p4kx_extraction::{Record, Value};

use crate::build::original_input::OriginalCatalogInput;
use crate::export_diagnostic::ExportSourceRole;
use crate::export_ledger::{DerivedFailure, ExportLedger};
use crate::record_observation::RecordObservation;
use crate::record_read::ReadError;
use crate::template_issues::TemplateError;
use crate::template_property_plan::SelectedProperty;
use crate::template_source_members::TemplateMembers;
use crate::{CatalogError, CatalogReadFailure, Kind};

/// Retains required C failures while the successfully emitted D owner remains projected.
pub(crate) fn retain<'source>(
    input: &OriginalCatalogInput<'_, '_>,
    record: &Record<'source>,
    errors: Vec<ReadError>,
    ledger: &mut ExportLedger,
    failures: &mut Vec<CatalogReadFailure<'source>>,
) -> Result<(), CatalogError> {
    if errors.is_empty() {
        return Ok(());
    }
    let admission = RecordObservation::capture(input, record)?;
    for error in errors {
        let diagnosis = crate::export_source_failure::diagnosis(error.as_ref());
        for kind in [
            Kind::ContractTemplates,
            Kind::ContractShipSpawns,
            Kind::ContractNpcSpawns,
            Kind::ContractEntitySpawns,
            Kind::ContractLocationQueries,
        ] {
            ledger.derived_failure(DerivedFailure::of_subject(
                admission.subject().clone(),
                kind,
                Kind::ContractTemplates,
                ExportSourceRole::DerivedInput,
                diagnosis.clone(),
            )?);
        }
        failures.push(CatalogReadFailure::of(record, error.as_ref()));
    }
    Ok(())
}

pub(crate) struct TemplateErrors {
    pub(crate) before_flow: Vec<ReadError>,
    pub(crate) after_flow: Vec<ReadError>,
}
impl TemplateErrors {
    pub(crate) fn retain<'source>(
        self,
        input: &OriginalCatalogInput<'_, '_>,
        record: &Record<'source>,
        flow_failure: Option<TemplateError>,
        ledger: &mut ExportLedger,
        failures: &mut Vec<CatalogReadFailure<'source>>,
    ) -> Result<(), CatalogError> {
        let mut errors = self.before_flow;
        errors.extend(flow_failure.map(|error| Box::new(error) as ReadError));
        errors.extend(self.after_flow);
        retain(input, record, errors, ledger, failures)
    }
}

/// Orders property errors in the root-spawn, objective, root-value read sequence.
pub(crate) fn order<'scope, 'read, 'source>(
    members: &TemplateMembers<'_, 'scope, 'read, 'source>,
    properties: &[SelectedProperty<'_, 'scope, 'read, 'source>],
    mut failures: Vec<(u64, ReadError)>,
) -> Result<TemplateErrors, CatalogError> {
    let mut ranks = Ranks::default();
    ranks.root_spawns(members, properties)?;
    for value in properties
        .iter()
        .filter(|property| property.objective.is_some())
        .filter_map(|property| property.value)
    {
        ranks.append(value.ordinal())?;
    }
    let before_flow_end = ranks.next;
    for value in properties
        .iter()
        .filter(|property| property.objective.is_none())
        .filter_map(|property| property.value)
    {
        ranks.append(value.ordinal())?;
    }
    failures.sort_by_key(|(ordinal, _)| ranks.positions.get(ordinal).copied());
    let mut ordered = TemplateErrors {
        before_flow: Vec::new(),
        after_flow: Vec::new(),
    };
    for (ordinal, error) in failures {
        if ranks
            .positions
            .get(&ordinal)
            .is_none_or(|rank| *rank < before_flow_end)
        {
            ordered.before_flow.push(error);
        } else {
            ordered.after_flow.push(error);
        }
    }
    Ok(ordered)
}

#[derive(Default)]
struct Ranks {
    positions: HashMap<u64, usize>,
    next: usize,
}

impl Ranks {
    fn append(&mut self, ordinal: u64) -> Result<(), CatalogError> {
        self.positions.entry(ordinal).or_insert(self.next);
        self.next = self
            .next
            .checked_add(1)
            .ok_or(crate::ExportAccountingError::CounterOverflow)?;
        Ok(())
    }

    fn root_spawns<'scope, 'read, 'source>(
        &mut self,
        members: &TemplateMembers<'_, 'scope, 'read, 'source>,
        properties: &[SelectedProperty<'_, 'scope, 'read, 'source>],
    ) -> Result<(), CatalogError> {
        for property in properties
            .iter()
            .filter(|property| property.objective.is_none())
        {
            let Some(value) = property.value else {
                continue;
            };
            let Some((_, variable)) = members
                .fields()
                .field(Some(property.naming), "missionVariableName")?
            else {
                continue;
            };
            if matches!(variable.value(), Value::String(name) | Value::Enum(name) if !name.is_empty())
                && matches!(value.value(), Value::Struct(source)
                    if crate::template_property_kind::PropertyValueKind::read(source.type_name())
                        .is_some_and(crate::template_property_kind::PropertyValueKind::is_spawn))
            {
                self.append(value.ordinal())?;
            }
        }
        Ok(())
    }
}
