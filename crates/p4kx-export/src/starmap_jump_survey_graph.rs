// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, HashSet};

use p4kx_extraction::{Archive, Guid, SourceValue};
use serde::Serialize;

use super::{endpoint_text, relation_field};
use crate::catalog_read::original::{
    AuthoredEntryPath, EntryCapture, EntryXml, PhysicalEntryRef, with_entry_capture,
};
use crate::export_ledger::ExportLedger;
use crate::starmap_containers::jump_class::JumpEndpointRecords;
use crate::starmap_containers::jump_gate::GateFiles;
use crate::starmap_index::Index;
use crate::starmap_parse::installed;
use crate::starmap_read;
use crate::starmap_xml::{Node, Root, attribute};
use crate::{CatalogError, ExportAccountingError};

#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct GraphSurvey {
    pub(super) roots: usize,
    ambiguous_roots: usize,
    unreached_system_container_candidates: usize,
    supplemental_physical_seeds: usize,
    #[serde(flatten)]
    pub(super) reachable: ContainerSurvey,
    supplemental: ContainerSurvey,
}

#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ContainerSurvey {
    pub(super) endpoint_declarations: usize,
    physical_containers: usize,
    non_authoritative_container_directories: usize,
    readable_descriptors: usize,
    unavailable_descriptors: usize,
    fallback_descriptors: usize,
    ambiguous_descriptors: usize,
    missing_external_targets: usize,
    ambiguous_external_targets: usize,
    missing_endpoint_external_targets: usize,
    ambiguous_endpoint_external_targets: usize,
    external_targets: usize,
    local_endpoint_container_paths: usize,
    invalid_starmap_record_texts: usize,
    marked_gate_descriptors: usize,
    unavailable_gate_markers: usize,
    gate_metadata: GateMemberCounts,
    gate_editors: GateMemberCounts,
    admitted_main_xml: FieldSurvey,
    endpoint_child_fields: FieldSurvey,
    marked_gate_xml: FieldSurvey,
}

#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct FieldSurvey {
    attributes: usize,
    relation_named_attributes: usize,
    relation_named_elements: usize,
    endpoint_guid_values: usize,
    relation_named_texts: usize,
    endpoint_guid_texts: usize,
    text_candidates_without_value_coordinates: usize,
    incomplete_coordinates: usize,
    candidate_shapes: BTreeMap<String, usize>,
}

pub(super) fn survey(
    archive: &Archive,
    endpoints: &JumpEndpointRecords,
) -> Result<GraphSurvey, CatalogError> {
    let index = Index::new(archive);
    let roots = index.systems();
    let mut state = GraphSurvey {
        roots: roots.len(),
        ..GraphSurvey::default()
    };
    let mut pending = Targets::default();
    for (_, root) in roots {
        let Some(entry) = index.entries.get(root) else {
            state.reachable.unavailable_descriptors += 1;
            continue;
        };
        match index.endpoint_candidates(entry.path()) {
            [at] if *at == root => pending.push(root),
            [_, _, ..] => state.ambiguous_roots += 1,
            _ => state.reachable.unavailable_descriptors += 1,
        }
    }
    state
        .reachable
        .drain(archive, &index, endpoints, &mut pending)?;
    pending.scope = TargetScope::Supplemental;
    for (_, selected) in index.system_containers() {
        state.unreached_system_container_candidates +=
            usize::from(!pending.seen.contains(&selected));
        let entry = index
            .entries
            .get(selected)
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        for &candidate in index.endpoint_candidates(entry.path()) {
            pending.push(candidate);
        }
    }
    state.supplemental_physical_seeds = pending.entries.len();
    state
        .supplemental
        .drain(archive, &index, endpoints, &mut pending)?;
    Ok(state)
}

impl ContainerSurvey {
    fn drain(
        &mut self,
        archive: &Archive,
        index: &Index<'_>,
        endpoints: &JumpEndpointRecords,
        pending: &mut Targets,
    ) -> Result<(), CatalogError> {
        while let Some(entry) = pending.entries.pop() {
            self.physical_containers += 1;
            with_entry_capture(archive, |capture| {
                self.inspect_container(archive, index, entry, capture, endpoints, pending)
            })?;
        }
        Ok(())
    }

    fn inspect_container(
        &mut self,
        archive: &Archive,
        index: &Index<'_>,
        entry: usize,
        capture: &EntryCapture<'_>,
        endpoints: &JumpEndpointRecords,
        pending: &mut Targets,
    ) -> Result<(), CatalogError> {
        let Some(outer) = index.entries.get(entry) else {
            self.unavailable_descriptors += 1;
            return Ok(());
        };
        let mut ledger = ExportLedger::default();
        let Ok(body) = starmap_read::main_xml(archive, outer, capture, &mut ledger)? else {
            self.unavailable_descriptors += 1;
            return Ok(());
        };
        self.non_authoritative_container_directories +=
            usize::from(crate::export_archive_health::inventory(&body.container).is_some());
        if starmap_read::named_descriptor_count(&body.container, outer)? > 1 {
            self.ambiguous_descriptors += 1;
        }
        let Ok(xml) = EntryXml::parse(&body.bytes, capture)? else {
            self.unavailable_descriptors += 1;
            return Ok(());
        };
        if xml.is_lossy() {
            self.unavailable_descriptors += 1;
            return Ok(());
        }
        self.readable_descriptors += 1;
        self.admitted_main_xml.document(&xml, endpoints)?;
        self.fallback_descriptors += usize::from(body.fallback.is_some());
        let gate = GateFiles {
            archive,
            outer,
            container: &body.container,
            capture,
            cancel: None,
        };
        self.inspect_gate(&gate, &xml, &mut ledger, endpoints)?;
        installed::visit_original_children(&xml, |child, target| {
            let endpoint = self.inspect_child(&xml, child, target.as_ref(), endpoints)?;
            if let Some(path) = target {
                self.follow(index, &path, endpoint, pending);
            }
            Ok(())
        })
    }

    fn inspect_gate(
        &mut self,
        gate: &GateFiles<'_, '_>,
        xml: &EntryXml<'_, '_, '_>,
        ledger: &mut ExportLedger,
        endpoints: &JumpEndpointRecords,
    ) -> Result<(), CatalogError> {
        let survey = gate.survey(xml, ledger, |document| {
            self.marked_gate_xml.document(document, endpoints)
        })?;
        match survey.marker {
            SourceValue::Present(true) => {
                self.marked_gate_descriptors += 1;
                self.gate_metadata.observe(&survey.metadata);
                self.gate_editors.observe(&survey.editor);
            }
            SourceValue::Present(false) => {}
            _ => self.unavailable_gate_markers += 1,
        }
        Ok(())
    }

    fn inspect_child(
        &mut self,
        xml: &EntryXml<'_, '_, '_>,
        child: Root<'_>,
        external: Option<&AuthoredEntryPath>,
        endpoints: &JumpEndpointRecords,
    ) -> Result<bool, CatalogError> {
        let Some(raw_record) = attribute(child, "starMapRecord").filter(|s| !s.trim().is_empty())
        else {
            return Ok(false);
        };
        let Ok(guid) = raw_record.trim().parse::<Guid>() else {
            self.invalid_starmap_record_texts += 1;
            return Ok(false);
        };
        if endpoints.get(guid).is_none() {
            return Ok(false);
        }
        self.endpoint_declarations += 1;
        self.local_endpoint_container_paths += usize::from(external.is_some());
        self.endpoint_child_fields
            .subtree(xml, child, endpoints, |node| {
                !installed::is_child_list(node)
            })?;
        Ok(true)
    }

    fn follow(
        &mut self,
        index: &Index<'_>,
        path: &AuthoredEntryPath,
        endpoint: bool,
        pending: &mut Targets,
    ) {
        self.external_targets += 1;
        match index.endpoint_candidates(path.as_str()) {
            [at] => pending.push(*at),
            [] => {
                self.missing_external_targets += 1;
                self.missing_endpoint_external_targets += usize::from(endpoint);
            }
            candidates => {
                self.ambiguous_external_targets += 1;
                self.ambiguous_endpoint_external_targets += usize::from(endpoint);
                if pending.scope == TargetScope::Supplemental {
                    for &candidate in candidates {
                        pending.push(candidate);
                    }
                }
            }
        }
    }
}

#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct GateMemberCounts {
    present: usize,
    missing: usize,
    unreadable: usize,
    invalid_or_ambiguous: usize,
    other_unavailable: usize,
}

impl GateMemberCounts {
    const fn observe(&mut self, value: &SourceValue<PhysicalEntryRef>) {
        match value {
            SourceValue::Present(_) => self.present += 1,
            SourceValue::Absent => self.missing += 1,
            SourceValue::Unreadable => self.unreadable += 1,
            SourceValue::Invalid => self.invalid_or_ambiguous += 1,
            _ => self.other_unavailable += 1,
        }
    }
}

#[derive(Default, PartialEq, Eq)]
enum TargetScope {
    #[default]
    Reachable,
    Supplemental,
}

#[derive(Default)]
struct Targets {
    scope: TargetScope,
    seen: HashSet<usize>,
    entries: Vec<usize>,
}

impl Targets {
    fn push(&mut self, entry: usize) {
        if self.seen.insert(entry) {
            self.entries.push(entry);
        }
    }
}

#[path = "starmap_jump_survey_graph_tests.rs"]
mod tests;

impl FieldSurvey {
    fn document(
        &mut self,
        xml: &EntryXml<'_, '_, '_>,
        endpoints: &JumpEndpointRecords,
    ) -> Result<(), CatalogError> {
        self.subtree(xml, xml.root(), endpoints, |_| true)
    }

    fn subtree<'node>(
        &mut self,
        xml: &EntryXml<'_, '_, '_>,
        root: Root<'node>,
        endpoints: &JumpEndpointRecords,
        include: impl Fn(Root<'node>) -> bool,
    ) -> Result<(), CatalogError> {
        let mut pending = vec![root];
        while let Some(node) = pending.pop() {
            let name = node.name();
            if relation_field(name) {
                self.relation_named_elements += 1;
                *self.candidate_shapes.entry(name.to_owned()).or_default() += 1;
            }
            self.node(xml, node, endpoints)?;
            self.element_text(node, endpoints);
            pending.extend(node.children().filter(|child| include(*child)));
        }
        Ok(())
    }

    fn element_text(&mut self, node: Root<'_>, endpoints: &JumpEndpointRecords) {
        let name = node.name();
        match node {
            Root::Cry(element) => {
                if let Some(text) = element.content() {
                    self.text_candidate(name, text, endpoints);
                }
            }
            Root::Text(element) => {
                for text in roxmltree::Node::children(&element)
                    .filter(roxmltree::Node::is_text)
                    .filter_map(|child| child.text())
                {
                    self.text_candidate(name, text, endpoints);
                }
            }
        }
    }

    fn text_candidate(&mut self, name: &str, text: &str, endpoints: &JumpEndpointRecords) {
        if text.trim().is_empty() {
            return;
        }
        let relation = relation_field(name);
        let endpoint = endpoint_text(text, endpoints);
        self.relation_named_texts += usize::from(relation);
        self.endpoint_guid_texts += usize::from(endpoint);
        if relation || endpoint {
            self.text_candidates_without_value_coordinates += 1;
            *self
                .candidate_shapes
                .entry(format!("{name}/#text"))
                .or_default() += 1;
        }
    }

    fn node(
        &mut self,
        xml: &EntryXml<'_, '_, '_>,
        node: Root<'_>,
        endpoints: &JumpEndpointRecords,
    ) -> Result<(), CatalogError> {
        for (ordinal, (name, text)) in node.attributes().enumerate() {
            self.attributes += 1;
            let candidate_name = relation_field(name);
            let endpoint_value =
                !name.eq_ignore_ascii_case("starMapRecord") && endpoint_text(text, endpoints);
            self.relation_named_attributes += usize::from(candidate_name);
            self.endpoint_guid_values += usize::from(endpoint_value);
            if candidate_name || endpoint_value {
                *self
                    .candidate_shapes
                    .entry(format!("{}/@{name}", node.name()))
                    .or_default() += 1;
                let coordinate = xml.attribute_at(node, ordinal)?;
                let complete = match coordinate {
                    Some(attribute) => attribute.selector_inputs()?.1,
                    None => false,
                };
                self.incomplete_coordinates += usize::from(!complete);
            }
        }
        Ok(())
    }
}
