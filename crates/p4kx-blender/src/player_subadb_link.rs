// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::cryxml::SourceElement;
use p4kx_extraction::{
    CancelToken, SourceUse, SourceUseRole, SourceUses, SourceValue, SourcedValue,
};
use schemars::JsonSchema;
use serde::Serialize;

use crate::soc_scene::SocSceneError;
use crate::soc_scene_class_mannequin::{children, string_attribute};
use crate::soc_scene_class_resources::Resource;
use crate::soc_scene_origin::{SocXmlPosition, XmlOrigin};
use crate::soc_scene_rooms::position;

use super::explicit_path;

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct DatabaseLink {
    position: SourcedValue<SocXmlPosition>,
    file: SourcedValue<String>,
    file_occurrences: Vec<SourcedValue<String>>,
    tags: SourcedValue<String>,
    target: SourcedValue<Resource<usize>>,
    traversal: SourcedValue<Visit>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(super) enum Visit {
    Reachable,
    Cycle,
    Unresolved,
}

pub(in super::super) struct SubDatabase {
    position: SourcedValue<SocXmlPosition>,
    file: SourcedValue<String>,
    file_occurrences: Vec<SourcedValue<String>>,
    tags: SourcedValue<String>,
    uses: Vec<SourceUse>,
}

impl SubDatabase {
    pub(in super::super) fn read_all(
        root: SourceElement<'_>,
        origin: &XmlOrigin<'_>,
        cancel: &CancelToken,
    ) -> Result<Vec<Self>, SocSceneError> {
        let mut links = Vec::new();
        for section in children(root, "SubADBs") {
            let section_use = origin.node(section, SourceUseRole::Selector)?;
            for node in children(section, "SubADB") {
                if cancel.is_cancelled() {
                    return Err(SocSceneError::Cancelled);
                }
                let node_use = origin.node(node, SourceUseRole::Selector)?;
                let mut uses = vec![section_use.clone(), node_use.clone()];
                let mut files = Vec::new();
                for attribute in node
                    .attributes()
                    .filter(|a| a.namespace().is_none() && a.name() == "File")
                {
                    let (_, selector) = origin.attribute(&attribute, SourceUseRole::Selector)?;
                    files.push(origin.sources.value(
                        SourceValue::Present(attribute.value().to_owned()),
                        std::slice::from_ref(&selector),
                        true,
                    ));
                    uses.push(selector);
                }
                let position = origin.sources.value(
                    SourceValue::Present(position(node)?),
                    std::slice::from_ref(&node_use),
                    true,
                );
                links.push(Self {
                    position,
                    file: string_attribute(node, "File", origin)?,
                    file_occurrences: files,
                    tags: string_attribute(node, "Tags", origin)?,
                    uses,
                });
            }
        }
        Ok(links)
    }

    pub(super) fn path(&self) -> Option<String> {
        explicit_path(self.file.value()?)
    }

    pub(super) fn selectors(&self) -> &[SourceUse] {
        &self.uses
    }

    pub(super) fn without_target(&self, sources: &SourceUses) -> SourcedValue<Resource<usize>> {
        let value = match self.file.value().map(String::as_str) {
            Some("") => Resource::EmptySelector,
            _ => Resource::InvalidSelector,
        };
        sources.value(SourceValue::Present(value), &self.uses, true)
    }

    pub(super) fn visit(
        &self,
        sources: &SourceUses,
        state: Visit,
        proof: Vec<SourceUse>,
        complete: bool,
    ) -> SourcedValue<Visit> {
        let mut uses = self.uses.clone();
        uses.extend(proof);
        sources.value(SourceValue::Present(state), &uses, complete)
    }

    pub(super) fn project(
        self,
        target: SourcedValue<Resource<usize>>,
        traversal: SourcedValue<Visit>,
    ) -> DatabaseLink {
        DatabaseLink {
            position: self.position,
            file: self.file,
            file_occurrences: self.file_occurrences,
            tags: self.tags,
            target,
            traversal,
        }
    }
}
