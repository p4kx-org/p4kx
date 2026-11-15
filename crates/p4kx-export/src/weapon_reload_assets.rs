// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;

use p4kx_extraction::cryxml::tag_definition::{Member, Section, TagDefinition};
use p4kx_extraction::{
    Archive, CancelToken, SourceLocation, SourceValue, SourcedValue, archive_animation_path,
};
use schemars::JsonSchema;
use serde::Serialize;

use crate::catalog_read::loose_archive_path::{LooseArchivePath, Resolver};
use crate::catalog_read::original::{
    EntryCapture, EntryRead, EntrySourceXml, PhysicalEntryIdentity, PhysicalEntryRef,
    SourceXmlAttribute,
};
use crate::entry_source::EntrySource;
use crate::export_entry_ledger::EntryOperation;
use crate::export_ledger::{ExportLedger, SourceDisposition};
use crate::schema_fields::every_field_required;
use crate::{CatalogError, ExportAccountingError, Kind};

use super::document::Database;

#[derive(Debug, Clone, Copy, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(extend("x-p4kx-vocabularyClass" = "control"))]
pub(super) enum AssetState {
    Unresolved,
    Unreadable,
    MalformedXml,
    UnsupportedRoot,
    Read,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct AssetLink {
    path: LooseArchivePath,
    state: SourcedValue<AssetState>,
    pub(super) source: Option<PhysicalEntryRef>,
}

#[derive(Clone, Copy)]
pub(super) enum FileSelector {
    Direct,
    SubDatabase,
}

impl AssetLink {
    pub(super) const fn origin(&self) -> &p4kx_extraction::FieldOrigin {
        self.state.origin()
    }
}
pub(super) struct Assets<'archive, 'capture, 'input> {
    archive: &'archive Archive,
    capture: &'capture EntryCapture<'input>,
    cancel: Option<&'archive CancelToken>,
    resolver: Resolver<'archive, 'capture, 'input>,
    reads: BTreeMap<PhysicalEntryRef, SourcedValue<AssetState>>,
    pub(super) databases: BTreeMap<PhysicalEntryRef, Database>,
    pub(super) controllers: BTreeMap<PhysicalEntryRef, Controller>,
    pub(super) definitions: BTreeMap<PhysicalEntryRef, Definition>,
    pub(super) ledger: ExportLedger,
}

impl<'archive, 'capture, 'input> Assets<'archive, 'capture, 'input> {
    pub(super) fn new(
        archive: &'archive Archive,
        capture: &'capture EntryCapture<'input>,
        cancel: Option<&'archive CancelToken>,
    ) -> Self {
        Self {
            archive,
            capture,
            cancel,
            resolver: Resolver::new(archive, capture, cancel),
            reads: BTreeMap::new(),
            databases: BTreeMap::new(),
            controllers: BTreeMap::new(),
            definitions: BTreeMap::new(),
            ledger: ExportLedger::default(),
        }
    }

    pub(super) fn select(
        &mut self,
        path: &SourcedValue<String>,
        selector: FileSelector,
    ) -> Result<AssetLink, CatalogError> {
        let path = normalize(self.capture, path, selector)?;
        let lookup = self.resolver.resolve(&path, |_| true)?;
        let source = match lookup.targets.as_slice() {
            [only] => only.value().cloned(),
            _ => None,
        };
        let mut origins = vec![lookup.state.origin()];
        let read = source
            .as_ref()
            .map(|target| self.read(target))
            .transpose()?;
        if let Some(read) = &read {
            origins.push(read.origin());
        }
        let state = read
            .as_ref()
            .and_then(SourcedValue::value)
            .copied()
            .unwrap_or(AssetState::Unresolved);
        let state = self
            .capture
            .source_uses()
            .value_with_origins(SourceValue::Present(state), &origins)?;
        Ok(AssetLink {
            path: lookup,
            state,
            source,
        })
    }

    fn read(
        &mut self,
        target: &PhysicalEntryRef,
    ) -> Result<SourcedValue<AssetState>, CatalogError> {
        if let Some(state) = self.reads.get(target) {
            return Ok(state.clone());
        }
        let index = usize::try_from(target.entry_index)
            .map_err(|_| ExportAccountingError::ConflictingSourceDatum)?;
        let entry = self
            .archive
            .entries()
            .get(index)
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        let source = EntrySource::outer(self.capture, self.archive, entry)?
            .operation(EntryOperation::SourceXml)
            .dependency();
        let state = match EntryRead::read_verified(
            source.receipt().clone(),
            self.archive,
            entry,
            self.cancel,
        )? {
            Ok(read) => match EntrySourceXml::parse(&read, self.capture)? {
                Ok(xml) => self.project(target, &xml)?,
                Err(_) => AssetState::MalformedXml,
            },
            Err(_) => AssetState::Unreadable,
        };
        let disposition = match state {
            AssetState::Unreadable => SourceDisposition::ReadFailed,
            AssetState::MalformedXml | AssetState::UnsupportedRoot => {
                SourceDisposition::ProjectionFailed
            }
            AssetState::Read => SourceDisposition::Projected,
            AssetState::Unresolved => {
                return Err(ExportAccountingError::ConflictingSourceDatum.into());
            }
        };
        let state = source
            .receipt()
            .payload_read_result(SourceValue::Present(state), disposition.read_succeeded())?;
        source.record(&mut self.ledger, Kind::Items, disposition);
        self.reads.insert(target.clone(), state.clone());
        Ok(state)
    }

    fn project(
        &mut self,
        target: &PhysicalEntryRef,
        xml: &EntrySourceXml<'_>,
    ) -> Result<AssetState, CatalogError> {
        if let Some(document) = Database::read(xml, self.cancel)? {
            self.databases.insert(target.clone(), document);
        } else if let Some(document) = Definition::read(xml, self.cancel)? {
            self.definitions.insert(target.clone(), document);
        } else if let Some(document) = Controller::read(xml)? {
            self.controllers.insert(target.clone(), document);
        } else {
            return Ok(AssetState::UnsupportedRoot);
        }
        Ok(AssetState::Read)
    }
}

fn normalize(
    capture: &EntryCapture<'_>,
    path: &SourcedValue<String>,
    selector: FileSelector,
) -> Result<SourcedValue<String>, CatalogError> {
    let Some(authored) = path.value() else {
        return Ok(path.clone());
    };
    if authored.is_empty() {
        return Ok(path.clone());
    }
    let normalized = match selector {
        FileSelector::Direct => direct_path(authored),
        FileSelector::SubDatabase => sub_database_path(authored),
    };
    let value = normalized.map_or(SourceValue::Invalid, SourceValue::Present);
    Ok(capture
        .source_uses()
        .value_with_origins(value, &[path.origin()])?)
}

fn direct_path(path: &str) -> Option<String> {
    let canonical = path.trim().replace('\\', "/");
    if !crate::catalog_read::loose_archive_path::valid(&canonical) {
        return None;
    }
    Some(
        if crate::selection::starts_with_folder(&canonical, "data/") {
            canonical
        } else {
            format!("Data/{canonical}")
        },
    )
}

fn sub_database_path(path: &str) -> Option<String> {
    let (_, extension) = path.rsplit_once('.')?;
    if extension.eq_ignore_ascii_case("adb") {
        archive_animation_path(path)
    } else {
        None
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct Controller {
    source: PhysicalEntryIdentity,
    pub(super) tags: Vec<SelectorNode>,
    pub(super) fragments: Vec<SelectorNode>,
}

impl Controller {
    fn read(xml: &EntrySourceXml<'_>) -> Result<Option<Self>, CatalogError> {
        let root = xml.root();
        if root.namespace().is_some() || root.name() != "ControllerDef" {
            return Ok(None);
        }
        let mut tags = Vec::new();
        let mut fragments = Vec::new();
        for element in root.elements().filter(|node| node.namespace().is_none()) {
            match element.name() {
                "Tags" => tags.push(SelectorNode::read(xml, element)?),
                "Fragments" => fragments.push(SelectorNode::read(xml, element)?),
                _ => {}
            }
        }
        Ok(Some(Self {
            source: xml.identity()?,
            tags,
            fragments,
        }))
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct Definition {
    source: PhysicalEntryIdentity,
    pub(super) version: SourcedValue<String>,
    pub(super) imports: Vec<SelectorNode>,
    pub(super) tags: Vec<SelectorNode>,
}

impl Definition {
    pub(super) fn readable_version(&self) -> bool {
        self.version.value().is_some_and(|value| value == "2")
    }
    fn read(
        xml: &EntrySourceXml<'_>,
        cancel: Option<&CancelToken>,
    ) -> Result<Option<Self>, CatalogError> {
        let Some(definition) = TagDefinition::from_root(xml.root()) else {
            return Ok(None);
        };
        let mut imports = Vec::new();
        let mut tags = Vec::new();
        for section in definition.sections() {
            super::document::check_cancel(cancel)?;
            match section {
                Section::Imports(list) => {
                    for node in list.elements() {
                        imports.push(SelectorNode::read(xml, node)?);
                    }
                }
                Section::Tags(list) => {
                    for member in list.members() {
                        match member {
                            Member::Group(group) => {
                                for node in group.tags() {
                                    tags.push(SelectorNode::read(xml, node)?);
                                }
                            }
                            Member::Tag(node) => tags.push(SelectorNode::read(xml, node)?),
                        }
                    }
                }
            }
        }
        let position = xml.position(xml.root())?;
        let version = super::graph::attribute(
            &xml.attributes(xml.root())?,
            "version",
            &position,
            xml.sources(),
        )?;
        Ok(Some(Self {
            source: xml.identity()?,
            version,
            imports,
            tags,
        }))
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct SelectorNode {
    pub(super) position: SourcedValue<SourceLocation>,
    pub(super) attributes: Vec<SourceXmlAttribute>,
}

impl SelectorNode {
    fn read(
        xml: &EntrySourceXml<'_>,
        element: p4kx_extraction::cryxml::SourceElement<'_>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            position: xml.position(element)?,
            attributes: xml.attributes(element)?,
        })
    }
}
