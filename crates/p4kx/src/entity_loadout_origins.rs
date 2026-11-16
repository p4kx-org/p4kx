// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::ops::ControlFlow;
use std::sync::Mutex;

use anyhow::{Result, anyhow};
use p4kx_assembly::{
    Attachment, EntityNode, LoadoutPath, LoadoutPathObserver, LoadoutPathOperation,
    LoadoutReadEvent, LoadoutReadObserver, LoadoutSelection, PortNode, XmlLoadoutPathSite,
};
use p4kx_extraction::{
    DataForge, DataForgeOrigin, EntryOrigin, FieldOrigin, FieldOrigins, FieldPath, InputScope,
    Record, RecordJsonLocation, RecordText, RecordTree, SourceInput, SourceInventory,
    SourceLocation, SourceScope, SourceUseRole, SourceUses,
};
use serde::Serialize;

use crate::entity_json::LoadoutJson;
use crate::json;

#[path = "entity_loadout_record_origins.rs"]
mod record_origins;

use record_origins::{RecordPaths, SelectedName};

pub(super) struct Capture<'forge> {
    forge: &'forge DataForge<'forge>,
    names: Mutex<HashMap<LoadoutSelection, AuthoredName>>,
}

enum AuthoredName {
    Record(SelectedName),
    Xml {
        entry: EntryOrigin,
        location: Option<SourceLocation>,
    },
}

impl<'forge> Capture<'forge> {
    pub(super) fn new(forge: &'forge DataForge<'forge>) -> Self {
        Self {
            forge,
            names: Mutex::new(HashMap::new()),
        }
    }

    pub(super) fn print(
        &self,
        payload: LoadoutJson<'_>,
        original: Option<&DataForgeOrigin>,
        tree: &EntityNode,
    ) -> Result<()> {
        let names = self
            .names
            .lock()
            .map_err(|_| anyhow!("loadout name evidence was interrupted"))?;
        let uses = SourceUses::with_compact_use_refs();
        let (input, complete) = match original {
            Some(DataForgeOrigin::Archive(entry)) => (
                SourceInput::Archive(entry),
                entry.ancestors().all(|ancestor| ancestor.is_some()),
            ),
            Some(DataForgeOrigin::File(file)) => (SourceInput::File(file), true),
            _ => (SourceInput::Unknown, false),
        };
        let forge_scope = uses.observe(input, InputScope::Payload)?;
        let mut output = Output {
            forge: self.forge,
            forge_scope,
            forge_complete: complete,
            uses,
            fields: FieldOrigins::new(),
            names: &names,
        };
        output.entry()?;
        output.entity(tree, "/entity")?;
        let Output { fields, uses, .. } = output;
        drop(names);
        json::print(&Report {
            payload,
            fields,
            provenance: uses.freeze()?,
        })?;
        Ok(())
    }
}

impl LoadoutReadObserver for Capture<'_> {
    fn observe(&self, _: LoadoutReadEvent<'_, '_>) -> ControlFlow<()> {
        ControlFlow::Continue(())
    }

    fn observes_port_names(&self) -> bool {
        true
    }

    fn with_record_paths(
        &self,
        record: &Record<'_>,
        tree: &RecordTree<'_>,
        parse: &mut dyn FnMut(Option<&dyn LoadoutPathObserver>) -> ControlFlow<()>,
    ) -> ControlFlow<()> {
        if !tree.is_from(record) || !record.is_from(self.forge) {
            return parse(None);
        }
        let sources = RecordPaths::new(self.forge);
        parse(Some(&sources))?;
        let sites = sources.positions(record, tree);
        if !sites.is_empty() {
            let Ok(mut names) = self.names.lock() else {
                return ControlFlow::Break(());
            };
            names.extend(
                sites
                    .into_iter()
                    .map(|(selection, source)| (selection, AuthoredName::Record(source))),
            );
        }
        ControlFlow::Continue(())
    }

    fn xml_path(
        &self,
        selection: &LoadoutSelection,
        site: &XmlLoadoutPathSite<'_>,
    ) -> ControlFlow<()> {
        if selection.operation() != LoadoutPathOperation::PortName {
            return ControlFlow::Continue(());
        }
        let file = site.observation();
        let Some(entry) = file
            .source()
            .entry_origin()
            .filter(|_| file.read_outcome().is_ok())
        else {
            return ControlFlow::Continue(());
        };
        let Ok(mut names) = self.names.lock() else {
            return ControlFlow::Break(());
        };
        names.insert(
            selection.clone(),
            AuthoredName::Xml {
                entry: entry.clone(),
                location: site.location(),
            },
        );
        ControlFlow::Continue(())
    }
}

#[derive(Serialize)]
struct Report<'a> {
    #[serde(flatten)]
    payload: LoadoutJson<'a>,
    #[serde(rename = "fieldOrigins")]
    fields: FieldOrigins,
    provenance: SourceInventory,
}

struct Output<'input, 'forge> {
    forge: &'forge DataForge<'forge>,
    forge_scope: SourceScope,
    forge_complete: bool,
    uses: SourceUses,
    fields: FieldOrigins,
    names: &'input HashMap<LoadoutSelection, AuthoredName>,
}

impl Output<'_, '_> {
    fn entry(&mut self) -> Result<()> {
        let scope = self
            .forge_scope
            .for_scope(InputScope::ArchiveDirectoryMetadata);
        let path = scope.at(
            SourceLocation::Metadata {
                field: "path".into(),
            },
            SourceUseRole::Payload,
        )?;
        self.put("/archive", self.uses.origin(&[], true))?;
        self.put("/entry", self.uses.origin(&[path], self.forge_complete))
    }

    fn entity(&mut self, entity: &EntityNode, address: &str) -> Result<()> {
        let record = entity
            .record_index()
            .and_then(|index| self.forge.record(index))
            .filter(|record| record.guid() == entity.guid());
        self.identity(record, address)?;
        self.ports(entity.ports(), &format!("{address}/ports"))
    }

    fn identity(&mut self, record: Option<Record<'_>>, address: &str) -> Result<()> {
        let names = ["guid", "record_path", "name", "class_name"];
        let Some(record) = record else {
            for member in names {
                self.put(&format!("{address}/{member}"), self.uses.origin(&[], false))?;
            }
            return Ok(());
        };
        let index = record.index();
        let guid = self.header(index, "guid", SourceUseRole::Payload, None, true)?;
        let file_name = record.stored_file_name();
        let raw_path = file_name
            .ok()
            .and_then(|text| RecordJsonLocation::borrowed_text(self.forge, text));
        let path = self.header(
            index,
            "fileName",
            SourceUseRole::Conversion,
            raw_path.clone(),
            raw_path.is_some(),
        )?;
        let (raw_name, name_known) = match record.stored_name() {
            Ok(RecordText::Present(name)) => {
                let span = RecordJsonLocation::borrowed_text(self.forge, name);
                let complete = span.is_some();
                (span, complete)
            }
            Ok(RecordText::Absent | RecordText::Null) => (None, true),
            _ => (None, false),
        };
        let name = self.header(
            index,
            "recordName",
            SourceUseRole::Payload,
            raw_name.clone(),
            name_known,
        )?;
        let class = self.header(
            index,
            "recordName",
            SourceUseRole::Conversion,
            raw_name,
            name_known,
        )?;
        for (member, origin) in names.into_iter().zip([guid, path, name, class]) {
            self.put(&format!("{address}/{member}"), origin)?;
        }
        Ok(())
    }

    fn header(
        &self,
        record_index: u32,
        member: &str,
        role: SourceUseRole,
        text: Option<SourceLocation>,
        complete: bool,
    ) -> Result<FieldOrigin> {
        let field = self.forge_scope.at(
            SourceLocation::DataForge {
                record_index,
                field: member.to_owned(),
            },
            role,
        )?;
        let mut sources = vec![field];
        if let Some(location) = text {
            sources.push(self.forge_scope.at(location, role)?);
        }
        Ok(self.uses.origin(&sources, complete && self.forge_complete))
    }

    fn ports(&mut self, ports: &[PortNode], collection: &str) -> Result<()> {
        for (ordinal, port) in ports.iter().enumerate() {
            let address = format!("{collection}/{ordinal}");
            let name = self.port_name(port)?;
            self.put(&format!("{address}/name"), name)?;
            self.ports(port.nested(), &format!("{address}/nested"))?;
            if let Attachment::Entity(child) = port.attachment() {
                self.entity(child, &format!("{address}/entity"))?;
            }
        }
        Ok(())
    }

    fn port_name(&self, port: &PortNode) -> Result<FieldOrigin> {
        let source = port
            .name_proof()
            .filter(|path| path.value() == port.name())
            .and_then(LoadoutPath::selection)
            .and_then(|selection| self.names.get(selection));
        match source {
            Some(AuthoredName::Record(site)) => {
                let selector = self.forge_scope.at(
                    SourceLocation::Subresource {
                        namespace: "dataforgeValue".into(),
                        indices: vec![u64::from(site.record), site.ordinal],
                        field: "$value".into(),
                    },
                    SourceUseRole::Selector,
                )?;
                let mut sources = vec![selector];
                if let Some(text) = &site.text {
                    sources.push(self.forge_scope.at(text.clone(), SourceUseRole::Payload)?);
                }
                Ok(self
                    .uses
                    .origin(&sources, site.text.is_some() && self.forge_complete))
            }
            Some(AuthoredName::Xml { entry, location }) => {
                let scope = self
                    .uses
                    .observe(SourceInput::Archive(entry), InputScope::Payload)?;
                let Some(location) = location else {
                    let opaque = scope.at_opaque(
                        SourceUseRole::Payload,
                        p4kx_extraction::SourceCoordinateOmission::Unavailable,
                    )?;
                    return Ok(self.uses.origin(&[opaque], false));
                };
                let original = scope.at(location.clone(), SourceUseRole::Payload)?;
                Ok(self.uses.origin(
                    &[original],
                    entry.ancestors().all(|ancestor| ancestor.is_some()),
                ))
            }
            None => Ok(self.uses.origin(&[], false)),
        }
    }

    fn put(&mut self, pointer: &str, origin: FieldOrigin) -> Result<()> {
        Ok(self.fields.insert(FieldPath::parse(pointer)?, origin)?)
    }
}
