// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, CancelToken, SourceUse, SourceValue, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use crate::catalog_read::original::{AuthoredEntryPath, EntryCapture, EntryXml, PhysicalEntryRef};
use crate::guid_text::CatalogGuid;
use crate::schema_fields::every_field_required;
use crate::starmap_index::Index;
use crate::{CatalogError, ExportAccountingError};

#[path = "loot_entity_container_nodes.rs"]
mod nodes;
#[path = "loot_entity_container_targets.rs"]
mod targets;

use nodes::LootEntityLinkNode;
use targets::LootContainerTargets;
pub(crate) use targets::{SelectedTarget, literal_candidates};

/// One original `EntXML` document with container-link attributes reached by the Loot walk.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct LootEntityContainerDocument {
    containing_container: SourcedValue<PhysicalEntryRef>,
    document: SourcedValue<PhysicalEntryRef>,
    nodes: SourcedValue<Vec<LootEntityLinkNode>>,
    links: SourcedValue<Vec<LootEntityContainerLink>>,
    config_references: SourcedValue<Vec<LootEntityConfigReference>>,
}

/// One original objectContainer attribute occurrence and its exact Archive lookup.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct LootEntityContainerLink {
    node_index: SourcedValue<u32>,
    attribute_ordinal: SourcedValue<u64>,
    declared_path: SourcedValue<String>,
    #[serde(flatten)]
    targets: LootContainerTargets,
}

/// One original multiConfigRef attribute occurrence, without an inferred link pairing.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct LootEntityConfigReference {
    node_index: SourcedValue<u32>,
    attribute_ordinal: SourcedValue<u64>,
    guid: SourcedValue<CatalogGuid>,
}

pub(crate) struct CapturedDocument {
    containing_container: SourcedValue<PhysicalEntryRef>,
    document: SourcedValue<PhysicalEntryRef>,
    nodes: SourcedValue<Vec<LootEntityLinkNode>>,
    links: Vec<CapturedLink>,
    config_references: SourcedValue<Vec<LootEntityConfigReference>>,
    survey: SourceUse,
    complete: bool,
}

struct CapturedLink {
    node_index: SourcedValue<u32>,
    attribute_ordinal: SourcedValue<u64>,
    declared_path: SourcedValue<String>,
    selector: AuthoredEntryPath,
}

pub(crate) fn capture(
    xml: &EntryXml<'_, '_, '_>,
    cancel: Option<&CancelToken>,
) -> Result<Option<CapturedDocument>, CatalogError> {
    let captured = nodes::capture(xml, cancel)?;
    if captured.links.is_empty() {
        return Ok(None);
    }
    let sources = xml.capture().source_uses();
    let survey = xml.document_source()?;
    let complete = !xml.is_lossy();
    Ok(Some(CapturedDocument {
        containing_container: xml
            .containing_entry()?
            .physical_reference(xml.capture())?
            .into_cell(sources),
        document: xml.payload_reference()?.into_cell(sources),
        nodes: sources.value(
            SourceValue::Present(captured.nodes),
            std::slice::from_ref(&survey),
            complete,
        ),
        links: captured.links,
        config_references: sources.value(
            SourceValue::Present(captured.configs),
            std::slice::from_ref(&survey),
            complete,
        ),
        survey,
        complete,
    }))
}

pub(crate) struct ProjectedDocument<'index> {
    pub(crate) document: LootEntityContainerDocument,
    pub(crate) selected_target: Option<SelectedTarget<'index>>,
}

impl CapturedDocument {
    pub(crate) fn project<'index>(
        self,
        archive: &Archive,
        index: &'index Index<'_>,
        directory: Option<&SourceUse>,
        capture: &EntryCapture<'_>,
        cancel: Option<&CancelToken>,
        selected: Option<&AuthoredEntryPath>,
    ) -> Result<ProjectedDocument<'index>, CatalogError> {
        let selected = selected
            .filter(|selected| {
                self.links.iter().all(|link| {
                    link.selector.as_str().is_empty() || link.selector.as_str() == selected.as_str()
                })
            })
            .map(AuthoredEntryPath::selector_inputs)
            .transpose()?;
        let selected = selected
            .as_ref()
            .filter(|(uses, complete)| *complete && !uses.is_empty())
            .map(|(uses, _)| uses.as_slice());
        let mut selected_target = None;
        let mut links = Vec::with_capacity(self.links.len());
        for original in self.links {
            checkpoint(cancel)?;
            let resolved = targets::resolve(
                archive,
                index,
                directory,
                capture,
                &original.selector,
                self.complete,
                selected.is_some(),
            )?;
            if let Some(current) = selected.and_then(|uses| resolved.selected_target(uses)) {
                selected_target = Some(current);
            }
            links.push(LootEntityContainerLink {
                node_index: original.node_index,
                attribute_ordinal: original.attribute_ordinal,
                declared_path: original.declared_path,
                targets: resolved.targets,
            });
        }
        Ok(ProjectedDocument {
            document: LootEntityContainerDocument {
                containing_container: self.containing_container,
                document: self.document,
                nodes: self.nodes,
                links: capture.source_uses().value(
                    SourceValue::Present(links),
                    &[self.survey],
                    self.complete,
                ),
                config_references: self.config_references,
            },
            selected_target,
        })
    }
}

impl LootEntityContainerDocument {
    pub(crate) fn physical_index(&self) -> Result<u32, CatalogError> {
        let original = self
            .document
            .value()
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        Ok(original.entry_index())
    }
}

fn checkpoint(cancel: Option<&CancelToken>) -> Result<(), CatalogError> {
    if cancel.is_some_and(CancelToken::is_cancelled) {
        return Err(CatalogError::Cancelled);
    }
    Ok(())
}

#[cfg(test)]
#[path = "loot_entity_container_document_tests.rs"]
mod tests;
