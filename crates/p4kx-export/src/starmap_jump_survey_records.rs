// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, HashMap};

use p4kx_extraction::{DataForge, TreeBuilder, Value};
use serde::Serialize;

use super::{endpoint_text, relation_field};
use crate::CatalogError;
use crate::build::original_input::OriginalCatalogInput;
use crate::starmap_containers::jump_class::JumpEndpointRecords;

const COMPONENT: &str = "SJumpPointParams";

#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RecordSurvey {
    endpoint_records: usize,
    operating_entity_records: usize,
    global_configuration_records: usize,
    other_selected_type_records: usize,
    read_records: usize,
    unreadable_records: usize,
    unreadable_values: usize,
    unavailable_record_headers: usize,
    unavailable_component_probes: usize,
    unavailable_selected_records: usize,
    component_probe_declared: bool,
    present_relation_named_fields: usize,
    unreadable_relation_named_fields: usize,
    original_endpoint_references: usize,
    endpoint_references_in_other_records: usize,
    endpoint_guid_cells: usize,
    ambiguous_endpoint_references: usize,
    candidate_shapes: BTreeMap<String, BTreeMap<String, usize>>,
}

pub(super) fn survey<'source>(
    forge: &'source DataForge<'source>,
    input: &OriginalCatalogInput<'_, 'source>,
    endpoints: &JumpEndpointRecords,
) -> Result<RecordSurvey, CatalogError> {
    input.check_forge(forge)?;
    let probe = forge.pointer_probe("Components", COMPONENT);
    let mut counts = RecordSurvey {
        component_probe_declared: probe.is_some(),
        ..RecordSurvey::default()
    };
    let mut trees = TreeBuilder::new();
    let mut kinds = HashMap::new();
    for record in forge.records() {
        let Ok(kind) = record.type_name() else {
            counts.unavailable_record_headers += 1;
            continue;
        };
        let endpoint = endpoints.get(record.guid()).is_some();
        let global = matches!(kind, "GlobalJumpPointParams" | "GlobalJumpTunnelHostParams");
        let operating = if let (Some(probe), "EntityClassDefinition") = (&probe, kind) {
            trees.holds(probe, &record).unwrap_or_else(|_| {
                counts.unavailable_component_probes += 1;
                false
            })
        } else {
            false
        };
        let other = *kinds.entry(kind).or_insert_with(|| candidate_type(kind));
        let selected = endpoint || global || operating || other;
        counts.endpoint_records += usize::from(endpoint);
        counts.operating_entity_records += usize::from(operating);
        counts.global_configuration_records += usize::from(global);
        counts.other_selected_type_records += usize::from(other && !global);
        let Ok(tree) = trees.build(&record) else {
            counts.unreadable_records += 1;
            counts.unavailable_selected_records += usize::from(selected);
            continue;
        };
        input.record_scope(&record, &tree)?;
        counts.read_records += 1;
        tree.root().visit_owned_values(|field| {
            counts.value(kind, field.field_name(), field.value(), endpoints, selected);
        });
    }
    Ok(counts)
}

fn candidate_type(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    name.contains("jump") || (name.contains("starmap") && relation_field(&name))
}

impl RecordSurvey {
    fn value(
        &mut self,
        kind: &str,
        name: &str,
        value: &Value<'_>,
        endpoints: &JumpEndpointRecords,
        selected: bool,
    ) {
        self.unreadable_values += usize::from(matches!(value, Value::Error(_)));
        let named = selected && relation_field(name);
        if named {
            match value {
                Value::Error(_) => self.unreadable_relation_named_fields += 1,
                Value::Null | Value::String("") => {}
                _ => self.present_relation_named_fields += 1,
            }
        }
        let endpoint_value = match value {
            Value::Reference(reference) => {
                endpoints.get(reference.guid()).is_some_and(|endpoint| {
                    if endpoint.index == reference.record_index() {
                        self.original_endpoint_references += 1;
                        self.endpoint_references_in_other_records += usize::from(!selected);
                    } else {
                        self.ambiguous_endpoint_references += 1;
                    }
                    true
                })
            }
            Value::Guid(guid) => endpoints.get(*guid).is_some(),
            Value::String(text) | Value::Locale(text) | Value::Enum(text) => {
                endpoint_text(text, endpoints)
            }
            _ => false,
        };
        if endpoint_value && !matches!(value, Value::Reference(_)) {
            self.endpoint_guid_cells += 1;
        }
        if named || endpoint_value {
            *self
                .candidate_shapes
                .entry(kind.to_owned())
                .or_default()
                .entry(name.to_owned())
                .or_default() += 1;
        }
    }
}

#[path = "starmap_jump_survey_record_tests.rs"]
mod tests;
