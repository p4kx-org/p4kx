// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    Archive, CancelToken, Entry, InputScope, SourceInput, SourceLocation, SourceUse, SourceUseRole,
    SourceValue, SourcedValue,
};
use schemars::JsonSchema;
use serde::Serialize;

use crate::catalog_read::original::{
    EntryCapture, EntryDiagnosis, EntryRead, EntryXml, PhysicalEntryRef,
};
use crate::entry_source::EntrySource;
use crate::export_diagnostic::ExportDiagnosticCode;
use crate::export_ledger::{ExportLedger, SourceDisposition};
use crate::guid_text::CatalogGuid;
use crate::schema_fields::every_field_required;
use crate::starmap_index::{self, SOCPAK};
use crate::starmap_xml::Node;
use crate::{CatalogError, ExportAccountingError, Kind};

#[path = "starmap_jump_gate_objects.rs"]
mod objects;

#[cfg(test)]
#[path = "starmap_jump_gate_survey.rs"]
pub(crate) mod survey;

use super::jump_operating::{OperatingConfiguration, OperatingEntity, OperatingResolver};
use objects::{entities, objects};

const COMPONENT: &str = "EntityComponentJumpPoint";
const META_PATH: &str = "metadata\\entitycomponentjumppoint.xml";

#[derive(Clone, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct JumpGate {
    marker: SourcedValue<bool>,
    metadata: SourcedValue<PhysicalEntryRef>,
    editor: SourcedValue<PhysicalEntryRef>,
    entities: Vec<JumpGateEntity>,
    #[serde(skip)]
    #[schemars(skip)]
    partner_inputs: Vec<SourceUse>,
}

impl JumpGate {
    pub(super) fn with_operating(
        mut self,
        resolver: &mut OperatingResolver<'_, '_, '_>,
    ) -> Result<Self, CatalogError> {
        for entity in &mut self.entities {
            let selected = entity.object.value().and_then(|ordinal| {
                entity
                    .matches
                    .value()?
                    .iter()
                    .find(|row| row.node_index.value() == Some(ordinal))
            });
            let state = selected.map_or_else(|| entity.object.state(), |row| row.class.state());
            let class = selected.and_then(|row| row.class.value().map(String::as_str));
            let mut uses = std::mem::take(&mut entity.object_uses);
            if let Some(selected) = selected {
                uses.push(selected.class_use.clone());
            }
            let joined = resolver.resolve(
                state,
                entity.object.origin().source_set_complete(),
                class,
                &uses,
            )?;
            entity.operating_entity = joined.entity;
            entity.operating_configurations = joined.configurations;
        }
        Ok(self)
    }
    pub(crate) fn marker_definitively_absent(&self) -> bool {
        self.marker.value() == Some(&false)
    }

    pub(crate) fn partner_evidence(&self) -> &[SourceUse] {
        &self.partner_inputs
    }

    pub(crate) const fn partner_scope_complete(&self) -> bool {
        match self.marker.value() {
            Some(false) => true,
            Some(true) => self.metadata.value().is_some() && self.editor.value().is_some(),
            None => false,
        }
    }
}

#[derive(Clone, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct JumpGateEntity {
    node_index: SourcedValue<u32>,
    cry_guid: SourcedValue<CatalogGuid>,
    cry_guid_text: SourcedValue<String>,
    offset_from_pivot: SourcedValue<String>,
    matches: SourcedValue<Vec<JumpGateObject>>,
    object: SourcedValue<u32>,
    operating_entity: SourcedValue<OperatingEntity>,
    operating_configurations: SourcedValue<Vec<OperatingConfiguration>>,
    #[serde(skip)]
    #[schemars(skip)]
    object_uses: Vec<SourceUse>,
}

#[derive(Clone, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct JumpGateObject {
    node_index: SourcedValue<u32>,
    guid: SourcedValue<CatalogGuid>,
    class: SourcedValue<String>,
    #[serde(skip)]
    #[schemars(skip)]
    class_use: SourceUse,
    name: SourcedValue<String>,
    tags: SourcedValue<String>,
}

pub(crate) struct GateFiles<'a, 'input> {
    pub(crate) archive: &'a Archive,
    pub(crate) outer: &'a Entry,
    pub(crate) container: &'a Archive,
    pub(crate) capture: &'a EntryCapture<'input>,
    pub(crate) cancel: Option<&'a CancelToken>,
}

impl GateFiles<'_, '_> {
    pub(crate) fn read(
        &self,
        main: &EntryXml<'_, '_, '_>,
        ledger: &mut ExportLedger,
    ) -> Result<JumpGate, CatalogError> {
        let sources = self.capture.source_uses();
        let (state, marker_uses) = marker(main)?;
        let present = matches!(state, SourceValue::Present(true));
        let marker = sources.value(state, &marker_uses, !main.is_lossy());
        if !present {
            let state = if main.is_lossy() {
                SourceValue::Unreadable
            } else {
                SourceValue::Absent
            };
            let absent = sources.value(state, &marker_uses, !main.is_lossy());
            return Ok(JumpGate {
                marker,
                metadata: absent.clone(),
                editor: absent,
                entities: Vec::new(),
                partner_inputs: marker_uses,
            });
        }

        let (nested_meta, editor_path) = self.member_paths()?;
        let (metadata, occurrences) = self.member(&[META_PATH, &nested_meta], ledger, |xml| {
            entities(xml, self.cancel)
        })?;
        let (editor, document) =
            self.member(&[&editor_path], ledger, |xml| objects(xml, self.cancel))?;
        let mut partner_inputs = marker_uses;
        partner_inputs.extend(metadata.uses.iter().chain(&editor.uses).cloned());
        let editor = sources.value(editor.state, &editor.uses, editor.complete);
        let metadata = sources.value(metadata.state, &metadata.uses, metadata.complete);
        let entities = occurrences
            .unwrap_or_default()
            .into_iter()
            .map(|entity| entity.link(sources, document.as_ref(), &editor))
            .collect();
        Ok(JumpGate {
            marker,
            metadata,
            editor,
            entities,
            partner_inputs,
        })
    }

    fn member_paths(&self) -> Result<(String, String), CatalogError> {
        let stem = container_stem(self.outer)?;
        Ok((format!("{stem}\\{META_PATH}"), format!("{stem}_editor.xml")))
    }

    fn member<T>(
        &self,
        paths: &[&str],
        ledger: &mut ExportLedger,
        read: impl FnOnce(&EntryXml<'_, '_, '_>) -> Result<Option<T>, CatalogError>,
    ) -> Result<(Member, Option<T>), CatalogError> {
        let mut directory = self
            .container
            .entries()
            .iter()
            .filter(|entry| paths.contains(&starmap_index::key(entry.path()).as_str()));
        let first = directory.next();
        let proof = self.directory()?;
        let mut outcome = Member {
            state: SourceValue::Absent,
            uses: vec![proof],
            complete: true,
        };
        let Some(entry) = first else {
            return Ok((outcome, None));
        };
        if let Some(second) = directory.next() {
            outcome.state = SourceValue::Invalid;
            for candidate in [entry, second].into_iter().chain(directory) {
                let source = self.member_source(candidate)?;
                let reference = source.receipt().payload_reference(self.capture)?;
                outcome.uses.extend_from_slice(reference.contributions().0);
                source.diagnosed(
                    ledger,
                    Kind::StarmapObjects,
                    SourceDisposition::ProjectionFailed,
                    EntryDiagnosis::technical(
                        ExportDiagnosticCode::AmbiguousIdentity,
                        COMPONENT,
                        "XML member path",
                    ),
                );
            }
            ledger.incomplete_inventory(Kind::StarmapObjects);
            return Ok((outcome, None));
        }
        self.read_member(entry, self.member_source(entry)?, outcome, ledger, read)
    }

    fn member_source(&self, entry: &Entry) -> Result<EntrySource, CatalogError> {
        Ok(EntrySource::inner(
            self.capture,
            self.archive,
            self.outer,
            self.container,
            entry,
        )?
        .dependency())
    }

    fn read_member<T>(
        &self,
        entry: &Entry,
        source: EntrySource,
        mut outcome: Member,
        ledger: &mut ExportLedger,
        read: impl FnOnce(&EntryXml<'_, '_, '_>) -> Result<Option<T>, CatalogError>,
    ) -> Result<(Member, Option<T>), CatalogError> {
        let reference = source.receipt().payload_reference(self.capture)?;
        outcome.uses.extend_from_slice(reference.contributions().0);
        let Ok(bytes) =
            EntryRead::read_verified(source.receipt().clone(), self.container, entry, self.cancel)?
        else {
            source.record(ledger, Kind::StarmapObjects, SourceDisposition::ReadFailed);
            ledger.incomplete_inventory(Kind::StarmapObjects);
            outcome.state = SourceValue::Unreadable;
            outcome.complete = false;
            return Ok((outcome, None));
        };
        let document = match EntryXml::parse(&bytes, self.capture)? {
            Ok(xml) if !xml.is_lossy() => {
                outcome.uses.push(xml.node_source(xml.root())?.selected()?);
                read(&xml)?
            }
            _ => None,
        };
        if document.is_some() {
            outcome.state = reference.value().clone();
            source.record(ledger, Kind::StarmapObjects, SourceDisposition::Projected);
        } else {
            outcome.state = SourceValue::Invalid;
            source.diagnosed(
                ledger,
                Kind::StarmapObjects,
                SourceDisposition::ProjectionFailed,
                EntryDiagnosis::technical(
                    ExportDiagnosticCode::MalformedSource,
                    COMPONENT,
                    "XML member",
                ),
            );
        }
        Ok((outcome, document))
    }

    fn directory(&self) -> Result<SourceUse, CatalogError> {
        let outer = self
            .archive
            .observed_entry(self.outer)
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        let scope = self
            .capture
            .source_uses()
            .observe(SourceInput::ArchiveView(outer), InputScope::Payload)?;
        let count = u64::try_from(self.container.entries().len())
            .map_err(|_| ExportAccountingError::CounterOverflow)?;
        Ok(scope.at(
            SourceLocation::Subresource {
                namespace: "containerDirectory".to_owned(),
                indices: vec![0, count],
                field: "paths".to_owned(),
            },
            SourceUseRole::Selector,
        )?)
    }
}

struct Member {
    state: SourceValue<PhysicalEntryRef>,
    uses: Vec<SourceUse>,
    complete: bool,
}

fn container_stem(entry: &Entry) -> Result<String, CatalogError> {
    starmap_index::key(entry.path())
        .rsplit('\\')
        .next()
        .and_then(|name| name.strip_suffix(SOCPAK))
        .map(str::to_owned)
        .ok_or_else(|| ExportAccountingError::ConflictingSourceDatum.into())
}

fn marker(
    main: &EntryXml<'_, '_, '_>,
) -> Result<(SourceValue<bool>, Vec<SourceUse>), CatalogError> {
    let root = main.root();
    let mut uses = vec![main.node_source(root)?.selected()?];
    if main.is_lossy() {
        return Ok((SourceValue::Unreadable, uses));
    }
    if root.name() != "ObjectContainer" {
        return Ok((SourceValue::Unsupported, uses));
    }
    let mut count = 0_u8;
    for metadata in root
        .children()
        .filter(|node| node.name() == "ComponentMetadata")
    {
        uses.push(main.node_source(metadata)?.selected()?);
        for mark in metadata.children().filter(|node| node.name() == COMPONENT) {
            count = count.saturating_add(1);
            uses.push(main.node_source(mark)?.selected()?);
        }
    }
    Ok((
        match count {
            0 => SourceValue::Present(false),
            1 => SourceValue::Present(true),
            _ => SourceValue::Invalid,
        },
        uses,
    ))
}
