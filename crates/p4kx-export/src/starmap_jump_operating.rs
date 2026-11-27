// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{HashMap, HashSet};

use p4kx_assembly::Entities;
use p4kx_extraction::{
    CancelToken, Record, SourceUse, SourceUseRole, SourceUses, SourceValue, SourceValueState,
    SourcedValue, TreeBuilder, Value,
};
use schemars::JsonSchema;
use serde::Serialize;

use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_read::original::{OriginalFlowFields, OriginalValueEdge, OriginalValuePart};
use crate::guid_text::CatalogGuid;
use crate::record_header_field::RecordHeaderField;
use crate::schema_fields::every_field_required;
use crate::{CatalogError, ExportAccountingError};

const COMPONENT: &str = "SJumpPointParams";

#[derive(Clone, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct OperatingEntity {
    owner_guid: CatalogGuid,
    owner_record_index: u32,
}

#[derive(Clone, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct OperatingConfiguration {
    owner_record_index: u32,
    component_ordinal: u32,
}

pub(super) struct OperatingBindings {
    pub(super) entity: SourcedValue<OperatingEntity>,
    pub(super) configurations: SourcedValue<Vec<OperatingConfiguration>>,
}

pub(crate) struct OperatingResolver<'a, 'read, 'source> {
    entities: &'a Entities<'source>,
    input: &'a OriginalCatalogInput<'read, 'source>,
    cancel: Option<&'a CancelToken>,
    owners: HashMap<u32, ObservedOwner>,
    selected: HashSet<u32>,
    trees: TreeBuilder<'source>,
}

struct ObservedOwner {
    entity: OperatingEntity,
    inputs: Vec<SourceUse>,
    configurations: SourceValue<Vec<OperatingConfiguration>>,
}

impl<'a, 'read, 'source> OperatingResolver<'a, 'read, 'source> {
    pub(crate) fn new(
        entities: &'a Entities<'source>,
        input: &'a OriginalCatalogInput<'read, 'source>,
        cancel: Option<&'a CancelToken>,
    ) -> Self {
        Self {
            entities,
            input,
            cancel,
            owners: HashMap::new(),
            selected: HashSet::new(),
            trees: TreeBuilder::new(),
        }
    }

    pub(crate) fn into_selected(self) -> HashSet<u32> {
        self.selected
    }

    pub(super) fn resolve(
        &mut self,
        object_state: SourceValueState,
        object_complete: bool,
        class: Option<&str>,
        local: &[SourceUse],
    ) -> Result<OperatingBindings, CatalogError> {
        if self.cancel.is_some_and(CancelToken::is_cancelled) {
            return Err(CatalogError::Cancelled);
        }
        let sources = &self.input.origins().uses;
        let Some(class) = class else {
            return Ok(no_owner(sources, object_state, local, object_complete));
        };
        let mut uses = local.to_vec();
        let mut candidates =
            self.entities
                .named_class_candidates(class)
                .iter()
                .filter(|candidate| {
                    candidate
                        .name()
                        .ok()
                        .and_then(|name| name.strip_prefix("EntityClassDefinition."))
                        == Some(class)
                });
        let Some(record) = candidates.next() else {
            return Ok(no_owner(sources, SourceValueState::Invalid, &uses, false));
        };
        if let Some(second) = candidates.next() {
            for candidate in std::iter::once(record)
                .chain(std::iter::once(second))
                .chain(candidates)
            {
                uses.extend(header_inputs(self.input, candidate)?);
            }
            return Ok(no_owner(
                sources,
                SourceValueState::Invalid,
                &uses,
                object_complete,
            ));
        }
        let owner =
            match self.owners.entry(record.index()) {
                std::collections::hash_map::Entry::Occupied(entry) => entry.into_mut(),
                std::collections::hash_map::Entry::Vacant(entry) => entry.insert(
                    ObservedOwner::read(self.input, &mut self.trees, record, self.cancel)?,
                ),
            };
        uses.extend(header_inputs(self.input, record)?);
        let entity = sources.value(
            SourceValue::Present(owner.entity.clone()),
            &uses,
            object_complete,
        );
        uses.extend(owner.inputs.iter().cloned());
        if matches!(&owner.configurations, SourceValue::Present(rows) if !rows.is_empty()) {
            self.selected.insert(record.index());
        }
        let complete = object_complete && !matches!(owner.configurations, SourceValue::Unreadable);
        Ok(OperatingBindings {
            entity,
            configurations: sources.value(owner.configurations.clone(), &uses, complete),
        })
    }
}

impl ObservedOwner {
    fn read<'source>(
        input: &OriginalCatalogInput<'_, 'source>,
        trees: &mut TreeBuilder<'source>,
        record: &Record<'source>,
        cancel: Option<&CancelToken>,
    ) -> Result<Self, CatalogError> {
        let entity = OperatingEntity {
            owner_guid: CatalogGuid(record.guid()),
            owner_record_index: record.index(),
        };
        let mut owner = Self {
            entity,
            inputs: Vec::new(),
            configurations: SourceValue::Unreadable,
        };
        let Ok(tree) = trees.build(record) else {
            owner.inputs.push(
                input
                    .record_context(record)?
                    .observe_header(RecordHeaderField::RecordIndex, SourceUseRole::Attempted)?,
            );
            return Ok(owner);
        };
        let scope = input.record_scope(record, &tree)?;
        let original = OriginalFlowFields::read(&scope, cancel)?;
        let Some((_, fact)) = original.field(None, "Components")? else {
            owner.configurations = SourceValue::Absent;
            return Ok(owner);
        };
        owner
            .inputs
            .push(fact.observe(OriginalValuePart::Value, SourceUseRole::Selector)?);
        let Value::Array(components) = fact.value() else {
            return Ok(owner);
        };
        let mut rows = Vec::new();
        for component in components {
            original.check_cancel()?;
            let fact = original.fact(component)?;
            let observed = fact.observe(OriginalValuePart::Value, SourceUseRole::Selector)?;
            if matches!(component, Value::Error(_)) {
                owner.inputs.push(observed);
                return Ok(owner);
            }
            if !matches!(component, Value::Struct(value) if value.type_name() == COMPONENT) {
                continue;
            }
            let OriginalValueEdge::Element { ordinal } = fact.edge() else {
                return Err(ExportAccountingError::ConflictingSourceDatum.into());
            };
            owner.inputs.push(observed);
            rows.push(OperatingConfiguration {
                owner_record_index: record.index(),
                component_ordinal: u32::try_from(ordinal)
                    .map_err(|_| ExportAccountingError::CounterOverflow)?,
            });
        }
        owner.configurations = SourceValue::Present(rows);
        Ok(owner)
    }
}

fn header_inputs(
    input: &OriginalCatalogInput<'_, '_>,
    record: &Record<'_>,
) -> Result<[SourceUse; 4], CatalogError> {
    let record = input.record_context(record)?;
    Ok([
        record.observe_header(RecordHeaderField::RecordName, SourceUseRole::Selector)?,
        record.observe_header(RecordHeaderField::SourceType, SourceUseRole::Selector)?,
        record.observe_header(RecordHeaderField::Guid, SourceUseRole::Payload)?,
        record.observe_header(RecordHeaderField::RecordIndex, SourceUseRole::Payload)?,
    ])
}

fn no_owner(
    sources: &SourceUses,
    state: SourceValueState,
    uses: &[SourceUse],
    complete: bool,
) -> OperatingBindings {
    const fn empty<T>(state: SourceValueState) -> SourceValue<T> {
        match state {
            SourceValueState::Absent => SourceValue::Absent,
            SourceValueState::Unreadable => SourceValue::Unreadable,
            _ => SourceValue::Invalid,
        }
    }
    OperatingBindings {
        entity: sources.value(empty(state), uses, complete),
        configurations: sources.value(empty(state), uses, complete),
    }
}
