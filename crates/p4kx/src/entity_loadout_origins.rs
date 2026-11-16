// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::ControlFlow;
use std::sync::Mutex;

use anyhow::{Result, anyhow};
use p4kx_assembly::{
    Attachment, EntityNode, LoadoutClassSelection, LoadoutPathObserver, LoadoutPathOperation,
    LoadoutReadEvent, LoadoutReadObserver, LoadoutSelection, PortNode, XmlLoadoutClassSite,
    XmlLoadoutPathSite,
};
use p4kx_extraction::{
    DataForge, DataForgeOrigin, FieldOrigin, FieldOrigins, FieldPath, InputScope, Record,
    RecordJsonLocation, RecordTree, SourceInput, SourceInventory, SourceLocation, SourceScope,
    SourceUseRole, SourceUses,
};
use serde::Serialize;

use crate::entity_json::LoadoutJson;
use crate::json;

#[path = "entity_loadout_attachment_origins.rs"]
mod attachment_origins;
#[path = "entity_loadout_bound_origins.rs"]
mod bound_origins;
#[path = "entity_loadout_record_origins.rs"]
mod record_origins;

use bound_origins::{Authored, Facts, Xml};
use record_origins::RecordPaths;

pub(super) struct Capture<'forge> {
    forge: &'forge DataForge<'forge>,
    facts: Mutex<Facts>,
}

impl<'forge> Capture<'forge> {
    pub(super) fn new(forge: &'forge DataForge<'forge>) -> Self {
        Self {
            forge,
            facts: Mutex::new(Facts::default()),
        }
    }

    pub(super) fn print(
        &self,
        payload: LoadoutJson<'_>,
        original: Option<&DataForgeOrigin>,
        tree: &EntityNode,
    ) -> Result<()> {
        let facts = self
            .facts
            .lock()
            .map_err(|_| anyhow!("loadout source evidence was interrupted"))?;
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
            facts: &facts,
        };
        output.entry()?;
        output.entity(tree, "/entity", None, &mut Vec::new())?;
        let Output { fields, uses, .. } = output;
        drop(facts);
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
            let Ok(mut facts) = self.facts.lock() else {
                return ControlFlow::Break(());
            };
            for (selection, source) in sites {
                facts.retain(selection, source);
            }
        }
        ControlFlow::Continue(())
    }

    fn xml_path(
        &self,
        selection: &LoadoutSelection,
        site: &XmlLoadoutPathSite<'_>,
    ) -> ControlFlow<()> {
        if !matches!(
            selection.operation(),
            LoadoutPathOperation::Copy | LoadoutPathOperation::PortName
        ) {
            return ControlFlow::Continue(());
        }
        let file = site.observation();
        if file.read_outcome().is_err() {
            return ControlFlow::Continue(());
        }
        let Ok(mut facts) = self.facts.lock() else {
            return ControlFlow::Break(());
        };
        facts.paths.insert(
            selection.clone(),
            Authored::Xml(Xml {
                file: file.clone(),
                location: site.location(),
            }),
        );
        ControlFlow::Continue(())
    }

    fn xml_class(
        &self,
        selection: &LoadoutClassSelection,
        site: &XmlLoadoutClassSite<'_>,
    ) -> ControlFlow<()> {
        let file = site.observation();
        if file.read_outcome().is_err() {
            return ControlFlow::Continue(());
        }
        let Ok(mut facts) = self.facts.lock() else {
            return ControlFlow::Break(());
        };
        facts.classes.insert(
            selection.clone(),
            Authored::Xml(Xml {
                file: file.clone(),
                location: site.location(),
            }),
        );
        ControlFlow::Continue(())
    }

    fn class_record(
        &self,
        selection: &LoadoutClassSelection,
        record: &Record<'_>,
    ) -> ControlFlow<()> {
        if !record.is_from(self.forge) {
            return ControlFlow::Continue(());
        }
        let Ok(mut facts) = self.facts.lock() else {
            return ControlFlow::Break(());
        };
        facts.class_record(selection, record.index());
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
    facts: &'input Facts,
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

    fn entity<'node>(
        &mut self,
        entity: &'node EntityNode,
        address: &str,
        incoming: Option<&FieldOrigin>,
        ancestors: &mut Vec<&'node PortNode>,
    ) -> Result<()> {
        let record = entity
            .record_index()
            .and_then(|index| self.forge.record(index))
            .filter(|record| record.guid() == entity.guid());
        self.identity(record, address, incoming)?;
        self.ports(entity.ports(), &format!("{address}/ports"), ancestors)
    }

    fn identity(
        &mut self,
        record: Option<Record<'_>>,
        address: &str,
        incoming: Option<&FieldOrigin>,
    ) -> Result<()> {
        let identity = RecordJsonLocation::identity(
            self.forge,
            record.as_ref(),
            &self.uses,
            &self.forge_scope,
            self.forge_complete,
        )?;
        for (member, mut origin) in identity {
            if let Some(selector) = incoming {
                origin = self.uses.origin_with_origins([&origin, selector], true)?;
            }
            self.put(&format!("{address}/{member}"), origin)?;
        }
        Ok(())
    }

    fn ports<'node>(
        &mut self,
        ports: &'node [PortNode],
        collection: &str,
        ancestors: &mut Vec<&'node PortNode>,
    ) -> Result<()> {
        for (ordinal, port) in ports.iter().enumerate() {
            let address = format!("{collection}/{ordinal}");
            let name = self.path_origin(
                port.name_proof().filter(|path| path.value() == port.name()),
                SourceUseRole::Payload,
            )?;
            self.put(&format!("{address}/name"), name)?;
            ancestors.push(port);
            let configured = self.configured_port(ancestors, &address)?;
            self.ports(port.nested(), &format!("{address}/nested"), ancestors)?;
            match port.attachment() {
                Attachment::Entity(child) => {
                    let incoming = self.attached(child, configured)?;
                    self.entity(
                        child,
                        &format!("{address}/entity"),
                        Some(&incoming),
                        ancestors,
                    )?;
                }
                Attachment::Unresolved(error) => self.unresolved(error, &address)?,
                Attachment::Empty => {}
            }
            ancestors.pop();
        }
        Ok(())
    }

    fn put(&mut self, pointer: &str, origin: FieldOrigin) -> Result<()> {
        Ok(self.fields.insert(FieldPath::parse(pointer)?, origin)?)
    }
}
