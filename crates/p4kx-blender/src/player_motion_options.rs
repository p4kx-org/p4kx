// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;

use p4kx_extraction::cryxml::{SourceDocument, SourceElement};
use p4kx_extraction::{
    CancelToken, PhysicalSourceRef, SourceUse, SourceUseRole, SourceUses, SourceValue, SourcedValue,
};
use schemars::JsonSchema;
use serde::Serialize;

use crate::soc_scene::SocSceneError;
use crate::soc_scene_class_mannequin::{children, string_attribute};
use crate::soc_scene_class_resources::{Resource, ResourceKind, Resources, resource_path};
use crate::soc_scene_origin::{SocXmlPosition, XmlOrigin};
use crate::soc_scene_rooms::position;
use crate::soc_scene_xml::XmlDocuments;

#[path = "player_motion_controller.rs"]
pub(super) mod controller;

use controller::{Controller, Fragment, ScopeTarget};

use super::subadb_graph::SubDatabase;

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct MotionOptions {
    controller_fragment_file: SourcedValue<String>,
    database_fragment_file: SourcedValue<String>,
    shared_fragment_file: SourcedValue<Resource<usize>>,
    options: Vec<OptionRow>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct OptionRow {
    position: SocXmlPosition,
    fragment: SourcedValue<String>,
    tags: SourcedValue<String>,
    fragment_tags: SourcedValue<String>,
    scopes: SourcedValue<Vec<SourcedValue<ScopeTarget>>>,
    controller_fragments: SourcedValue<Vec<SourcedValue<SocXmlPosition>>>,
}

pub(super) struct Database {
    pub(super) source: PhysicalSourceRef,
    pub(super) root_use: SourceUse,
    pub(super) links: Vec<SubDatabase>,
    pub(super) lossy: bool,
    pub(super) tag_file: SourcedValue<String>,
    pub(super) file: AuthoredFile,
    options: Vec<OptionInput>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct BranchOption {
    position: SourcedValue<SocXmlPosition>,
    fragment: SourcedValue<String>,
    tags: SourcedValue<String>,
    fragment_tags: SourcedValue<String>,
}

struct OptionInput {
    position: SocXmlPosition,
    name: String,
    group_use: SourceUse,
    tags: SourcedValue<String>,
    fragment_tags: SourcedValue<String>,
    node_use: SourceUse,
}

pub(super) struct AuthoredFile {
    pub(super) value: SourcedValue<String>,
    uses: Vec<SourceUse>,
}

impl AuthoredFile {
    pub(super) fn at<'document>(
        nodes: impl IntoIterator<Item = SourceElement<'document>>,
        parent: SourceElement<'_>,
        field: &str,
        origin: &XmlOrigin<'_>,
    ) -> Result<Self, SocSceneError> {
        let mut uses = vec![origin.node(parent, SourceUseRole::Selector)?];
        let mut values = Vec::new();
        for node in nodes {
            uses.push(origin.node(node, SourceUseRole::Selector)?);
            for attribute in node
                .attributes()
                .filter(|attribute| attribute.namespace().is_none() && attribute.name() == field)
            {
                uses.push(origin.attribute(&attribute, SourceUseRole::Selector)?.1);
                values.push(attribute.value().to_owned());
            }
        }
        let value = match values.len() {
            0 => SourceValue::Absent,
            1 => values
                .pop()
                .map_or(SourceValue::Absent, SourceValue::Present),
            _ => SourceValue::Invalid,
        };
        Ok(Self {
            value: origin.sources.value(value, &uses, true),
            uses,
        })
    }

    fn archive_path(&self) -> Option<String> {
        resource_path(self.value.value()?, ResourceKind::DataRelative)
    }
}

impl Database {
    pub(super) fn read(
        document: &SourceDocument<'_>,
        origin: &XmlOrigin<'_>,
        cancel: &CancelToken,
    ) -> Result<Self, SocSceneError> {
        let root = document.root();
        let file = AuthoredFile::at([root], root, "FragDef", origin)?;
        let tag_file = string_attribute(root, "TagDef", origin)?;
        let links = SubDatabase::read_all(root, origin, cancel)?;
        let mut options = Vec::new();
        for list in children(root, "FragmentList") {
            for group in list.elements().filter(|node| node.namespace().is_none()) {
                check(cancel)?;
                let group_use = origin.node(group, SourceUseRole::Selector)?;
                for option in children(group, "Fragment") {
                    check(cancel)?;
                    options.push(OptionInput {
                        position: position(option)?,
                        name: group.name().to_owned(),
                        group_use: group_use.clone(),
                        tags: string_attribute(option, "Tags", origin)?,
                        fragment_tags: string_attribute(option, "FragTags", origin)?,
                        node_use: origin.node(option, SourceUseRole::Payload)?,
                    });
                }
            }
        }
        Ok(Self {
            source: origin.source(),
            root_use: origin.node(root, SourceUseRole::Payload)?,
            links,
            lossy: document.is_lossy(),
            file,
            tag_file,
            options,
        })
    }
    pub(super) fn branch_options(&self, sources: &SourceUses) -> Vec<BranchOption> {
        self.options
            .iter()
            .map(|option| option.branch(sources))
            .collect()
    }
}

impl MotionOptions {
    pub(super) fn read(
        resources: &Resources<'_>,
        controller: Controller,
        database: Database,
        selectors: [&SourceUse; 2],
        documents: &mut XmlDocuments<'_>,
    ) -> Result<Self, SocSceneError> {
        let mut uses: Vec<_> = selectors.into_iter().cloned().collect();
        uses.extend(controller.file.uses.iter().cloned());
        uses.extend(database.file.uses.iter().cloned());
        let shared_fragment_file = shared_fragment_file(
            resources,
            &mut uses,
            &controller.file,
            &database.file,
            documents,
        )?;
        let proven = matches!(shared_fragment_file.value(), Some(Resource::Present { .. }));
        let mut by_name: HashMap<&str, Vec<&Fragment>> = HashMap::new();
        for fragment in &controller.fragments {
            by_name.entry(&fragment.name).or_default().push(fragment);
        }
        let mut options = Vec::new();
        for option in database.options {
            check(resources.cancel)?;
            let candidates: Option<&[&Fragment]> = if proven {
                Some(
                    by_name
                        .get(option.name.as_str())
                        .map_or(&[] as &[&Fragment], Vec::as_slice),
                )
            } else {
                None
            };
            options.push(option.project(resources, &uses, candidates));
        }
        Ok(Self {
            controller_fragment_file: controller.file.value,
            database_fragment_file: database.file.value,
            shared_fragment_file,
            options,
        })
    }
}

impl OptionInput {
    fn branch(&self, sources: &SourceUses) -> BranchOption {
        BranchOption {
            position: sources.value(
                SourceValue::Present(self.position.clone()),
                std::slice::from_ref(&self.node_use),
                true,
            ),
            fragment: sources.value(
                SourceValue::Present(self.name.clone()),
                std::slice::from_ref(&self.group_use),
                true,
            ),
            tags: self.tags.clone(),
            fragment_tags: self.fragment_tags.clone(),
        }
    }

    fn project(
        self,
        resources: &Resources<'_>,
        shared: &[SourceUse],
        candidates: Option<&[&Fragment]>,
    ) -> OptionRow {
        let sources = resources.sources;
        let fragment = sources.value(
            SourceValue::Present(self.name),
            &[self.group_use.clone(), self.node_use.clone()],
            true,
        );
        let mut uses = shared.to_vec();
        uses.extend([self.group_use, self.node_use]);
        let controller_fragments = if let Some(candidates) = candidates {
            uses.extend(
                candidates
                    .iter()
                    .map(|candidate| candidate.node_use.clone()),
            );
            let coordinates = candidates
                .iter()
                .map(|candidate| {
                    sources.value(
                        SourceValue::Present(candidate.position.clone()),
                        std::slice::from_ref(&candidate.node_use),
                        true,
                    )
                })
                .collect();
            sources.value(SourceValue::Present(coordinates), &uses, true)
        } else {
            sources.value(SourceValue::Unsupported, &uses, false)
        };
        let scopes = candidate_scopes(sources, candidates, &mut uses);
        OptionRow {
            position: self.position,
            fragment,
            tags: self.tags,
            fragment_tags: self.fragment_tags,
            scopes,
            controller_fragments,
        }
    }
}

fn candidate_scopes(
    sources: &SourceUses,
    candidates: Option<&[&Fragment]>,
    uses: &mut Vec<SourceUse>,
) -> SourcedValue<Vec<SourcedValue<ScopeTarget>>> {
    let Some([candidate]) = candidates else {
        return sources.value(SourceValue::Unsupported, uses, false);
    };
    uses.extend(candidate.scopes.uses.iter().cloned());
    if !candidate.scopes.valid {
        return sources.value(SourceValue::Invalid, uses, true);
    }
    let targets = candidate
        .scopes
        .targets
        .iter()
        .map(|target| {
            sources.value(
                SourceValue::Present(target.data.clone()),
                &target.uses,
                true,
            )
        })
        .collect();
    sources.value(SourceValue::Present(targets), uses, true)
}

fn shared_fragment_file(
    resources: &Resources<'_>,
    uses: &mut Vec<SourceUse>,
    controller: &AuthoredFile,
    database: &AuthoredFile,
    documents: &mut XmlDocuments<'_>,
) -> Result<SourcedValue<Resource<usize>>, SocSceneError> {
    let (Some(controller_path), Some(database_path)) =
        (controller.archive_path(), database.archive_path())
    else {
        return Ok(resources
            .sources
            .value(SourceValue::Unsupported, uses, false));
    };
    if !controller_path.eq_ignore_ascii_case(&database_path) {
        return Ok(resources.sources.value(SourceValue::Invalid, uses, true));
    }
    let mut target = None;
    let value = resources
        .capture_with_documents(
            SourceValue::Present(controller_path),
            ResourceKind::DataRelative,
            documents,
            |document, origin, documents| {
                target = Some(origin.node(document.root(), SourceUseRole::Selector)?);
                documents.bind(document, origin).map(Some)
            },
        )?
        .into_bound(resources.sources, uses);
    uses.extend(target);
    Ok(value)
}

fn check(cancel: &CancelToken) -> Result<(), SocSceneError> {
    if cancel.is_cancelled() {
        Err(SocSceneError::Cancelled)
    } else {
        Ok(())
    }
}
