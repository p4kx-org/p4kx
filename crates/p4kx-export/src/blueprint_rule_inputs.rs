// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;

use p4kx_extraction::{CancelToken, Guid, RecordTree, SourceUse, SourceUseRole, Value};

use crate::CatalogError;
use crate::blueprint_field_source::BlueprintField;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_read::original::{OriginalFlowFields, OriginalValuePart};
use crate::record_header_field::{RecordHeaderField, RecordHeaderInput};
use crate::record_observation::RecordObservation;

#[derive(Default)]
pub(crate) struct BlueprintRules {
    pub(super) defaults: RuleInputs,
    pub(super) item_exclusions: RuleInputs,
    pub(super) processes: Vec<SourceUse>,
    pub(super) process_unavailable: bool,
}

#[derive(Default)]
pub(super) struct RuleInputs {
    pub(super) sources: Vec<SourceUse>,
    pub(super) members: BTreeMap<Guid, Vec<SourceUse>>,
    pub(super) unavailable: bool,
}

impl BlueprintRules {
    pub(crate) fn merge(&mut self, other: Self) {
        self.defaults.merge(other.defaults);
        self.item_exclusions.merge(other.item_exclusions);
        self.processes.extend(other.processes);
        self.process_unavailable |= other.process_unavailable;
    }

    pub(crate) fn read_globals<'source>(
        tree: &RecordTree<'source>,
        admission: &RecordObservation<'_, 'source>,
        input: &OriginalCatalogInput<'_, 'source>,
        cancel: Option<&CancelToken>,
    ) -> Result<Self, CatalogError> {
        let scope = input.record_scope(admission.record(), tree)?;
        let fields = OriginalFlowFields::read(&scope, cancel)?;
        let defaults = BlueprintField::read(&fields, tree.root(), "defaultBlueprintSelection")?
            .child("blueprintRecords")?;
        let items = BlueprintField::read(&fields, tree.root(), "dismantleBlacklistEntityClasses")?;
        Ok(Self {
            defaults: RuleInputs::read(&fields, &defaults)?,
            item_exclusions: RuleInputs::read(&fields, &items)?,
            ..Self::default()
        })
    }

    pub(crate) fn read_dismantle<'source>(
        tree: &RecordTree<'source>,
        admission: &RecordObservation<'_, 'source>,
        input: &OriginalCatalogInput<'_, 'source>,
        cancel: Option<&CancelToken>,
    ) -> Result<Self, CatalogError> {
        let scope = input.record_scope(admission.record(), tree)?;
        let fields = OriginalFlowFields::read(&scope, cancel)?;
        let process = BlueprintField::read(&fields, tree.root(), "blueprint")?
            .child("processSpecificData")?;
        let context = input.record_context(admission.record())?;
        let mut selectors = Vec::new();
        for header in [
            RecordHeaderInput::Field(RecordHeaderField::Guid),
            RecordHeaderInput::Field(RecordHeaderField::SourceType),
            RecordHeaderInput::NormalizedPath,
            RecordHeaderInput::Field(RecordHeaderField::RecordName),
        ] {
            context.append_header_input(header, SourceUseRole::Selector, &mut selectors)?;
        }
        let mut readable = true;
        if let Some(fact) = process.fact() {
            readable = !matches!(fact.value(), Value::Error(_));
            let part = if matches!(fact.value(), Value::Struct(_)) {
                OriginalValuePart::ConcreteType
            } else {
                OriginalValuePart::Value
            };
            let role = if readable {
                SourceUseRole::Selector
            } else {
                SourceUseRole::Attempted
            };
            selectors.push(fact.observe(part, role)?);
        } else {
            selectors.push(process.absence_input()?);
        }
        Ok(Self {
            processes: selectors,
            process_unavailable: !readable,
            ..Self::default()
        })
    }
}

impl RuleInputs {
    fn read<'source>(
        fields: &OriginalFlowFields<'_, '_, 'source>,
        field: &BlueprintField<'_, '_, '_, 'source>,
    ) -> Result<Self, CatalogError> {
        let mut result = Self::default();
        let Some(fact) = field.fact() else {
            result.sources.push(field.absence_input()?);
            return Ok(result);
        };
        result.unavailable = matches!(fact.value(), Value::Error(_));
        let role = if result.unavailable {
            SourceUseRole::Attempted
        } else {
            SourceUseRole::Selector
        };
        result
            .sources
            .push(fact.observe(OriginalValuePart::Value, role)?);
        if !field.is_blocked() && matches!(fact.value(), Value::Array(_)) {
            for value in fields.elements(fact)? {
                if let Some(guid) = crate::record_fields::record_reference(value.value()) {
                    let selected =
                        value.observe(OriginalValuePart::Value, SourceUseRole::Selector)?;
                    result.members.entry(guid.0).or_default().push(selected);
                } else if matches!(value.value(), Value::Error(_)) {
                    result
                        .sources
                        .push(value.observe(OriginalValuePart::Value, SourceUseRole::Attempted)?);
                    result.unavailable = true;
                }
            }
        }
        Ok(result)
    }

    fn merge(&mut self, other: Self) {
        self.sources.extend(other.sources);
        self.unavailable |= other.unavailable;
        for (guid, values) in other.members {
            self.members.entry(guid).or_default().extend(values);
        }
    }
}
