// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use anyhow::Result;
use p4kx_assembly::{
    EntityNode, LoadoutTargetGuid, LoadoutXmlClassUse, PortNode, PortSource, Target, Unresolved,
};
use p4kx_extraction::{FieldOrigin, SourceUseRole};

use super::Output;
use super::bound_origins::{Authored, Xml};

impl Output<'_, '_> {
    pub(super) fn configured_port(
        &mut self,
        ports: &[&PortNode],
        address: &str,
    ) -> Result<Option<FieldOrigin>> {
        let Some(port) = ports.last() else {
            return Ok(None);
        };
        if !matches!(port.source(), PortSource::Excluded | PortSource::Replaced) {
            return Ok(None);
        }
        let path = ports
            .iter()
            .map(|port| {
                self.path_origin(
                    port.name_proof().filter(|name| name.value() == port.name()),
                    SourceUseRole::Selector,
                )
            })
            .collect::<Result<Vec<_>>>()?;
        let origin = self.uses.origin_with_origins(&path, true)?;
        self.put(&format!("{address}/source"), origin.clone())?;
        if port.source() == PortSource::Excluded {
            self.put(&format!("{address}/entity"), origin.clone())?;
            self.put(&format!("{address}/error"), origin.clone())?;
        }
        Ok(Some(origin))
    }

    pub(super) fn attached(
        &self,
        child: &EntityNode,
        configured: Option<FieldOrigin>,
    ) -> Result<FieldOrigin> {
        if let Some(configured) = configured {
            return Ok(configured);
        }
        let Some(target) = child
            .attachment_target()
            .filter(|target| child.guid() == target.guid())
        else {
            return Ok(self.uses.origin(&[], false));
        };
        self.selected_target(target, child.record_index())
    }

    fn selected_target(
        &self,
        target: &LoadoutTargetGuid,
        record: Option<u32>,
    ) -> Result<FieldOrigin> {
        let Some(class) = target.class_selection() else {
            return self.reference_origin(
                target.selection(),
                target.guid(),
                SourceUseRole::Selector,
            );
        };
        let source = self.facts.classes.get(class);
        let authored = self.authored(source, SourceUseRole::Selector)?;
        let selected = record.is_some_and(|record| self.facts.class_was_selected(class, record));
        let Some(Authored::Xml(xml)) = source else {
            return Ok(self.uses.origin_with_origins([&authored], selected)?);
        };
        let Some(route) = target.class_xml_use() else {
            return Ok(self.uses.origin_with_origins([&authored], false)?);
        };
        self.xml_target(authored, xml, route, selected)
    }

    fn xml_target(
        &self,
        authored: FieldOrigin,
        actual: &Xml,
        route: &LoadoutXmlClassUse,
        mut complete: bool,
    ) -> Result<FieldOrigin> {
        let mut current = route.original_root();
        let mut inputs = vec![
            authored,
            self.path_origin(Some(route.caller()), SourceUseRole::Selector)?,
        ];
        for (parent, child, path) in route.includes() {
            complete &= parent.is_same_operation(current);
            let path_source = path
                .selection()
                .and_then(|token| self.facts.paths.get(token));
            complete &= matches!(
                path_source,
                Some(Authored::Xml(site)) if site.file.is_same_operation(parent)
            );
            inputs.push(self.authored(path_source, SourceUseRole::Selector)?);
            current = child;
        }
        complete &= current.is_same_operation(&actual.file);
        Ok(self.uses.origin_with_origins(&inputs, complete)?)
    }

    pub(super) fn unresolved(&mut self, error: &Unresolved, address: &str) -> Result<()> {
        let reference = match error.target() {
            Target::Reference(guid) => {
                self.reference_origin(error.reference_selection(), *guid, SourceUseRole::Payload)?
            }
            _ => self.uses.origin(&[], false),
        };
        let diagnostic = self.uses.origin_with_origins([&reference], false)?;
        let error_path = format!("{address}/error");
        for field in ["kind", "reason", "class_name"] {
            self.put(&format!("{error_path}/{field}"), diagnostic.clone())?;
        }
        self.put(&format!("{error_path}/reference"), reference)?;
        self.put(&error_path, diagnostic.clone())?;
        self.put(&format!("{address}/entity"), diagnostic)
    }
}
