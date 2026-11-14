// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::cryxml::SourceElement;
use p4kx_extraction::{
    CancelToken, SourceScope, SourceUse, SourceUseRole, SourceUses, SourceValue, SourcedValue,
};
use schemars::JsonSchema;
use serde::Serialize;

use crate::soc_scene::SocSceneError;
use crate::soc_scene_class_mannequin::children;
#[path = "player_tag_definition_attribute.rs"]
mod attribute;

pub(super) use attribute::{Attribute, PendingAttribute};

use crate::soc_scene_origin::{SocXmlPosition, XmlOrigin};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(super) enum Kind {
    Tags,
    Fragments,
}

#[derive(Clone, Copy, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(super) enum Role {
    ControllerTags,
    ControllerFragments,
    DatabaseTags,
    DatabaseFragments,
    Import,
    FragmentTags,
}

impl Role {
    pub(super) const fn kind(self, owner: Option<Kind>) -> Option<Kind> {
        match self {
            Self::Import => owner,
            Self::ControllerTags | Self::DatabaseTags | Self::FragmentTags => Some(Kind::Tags),
            Self::ControllerFragments | Self::DatabaseFragments => Some(Kind::Fragments),
        }
    }
}

#[derive(Clone)]
pub(super) struct Owner {
    pub(super) scope: SourceScope,
    pub(super) document: usize,
    pub(super) edges: Vec<PendingEdge>,
}

#[derive(Clone)]
pub(super) struct PendingEdge {
    pub(super) role: Role,
    pub(super) position: SocXmlPosition,
    pub(super) selector: PendingAttribute,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct Definition {
    pub(super) position: SourcedValue<SocXmlPosition>,
    pub(super) root_name: SourcedValue<String>,
    pub(super) version: Attribute,
    pub(super) sections: Vec<Section>,
    #[serde(skip)]
    pub(super) evidence: Vec<SourceUse>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct Section {
    position: SourcedValue<SocXmlPosition>,
    members: Vec<Member>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(tag = "type", rename_all = "camelCase")]
enum Member {
    Group {
        position: SourcedValue<SocXmlPosition>,
        name: Attribute,
        tags: Vec<Tag>,
    },
    Tag(Tag),
}

struct PendingSection {
    position: SocXmlPosition,
    members: Vec<PendingMember>,
}

enum PendingMember {
    Group(PendingGroup),
    Tag(PendingTag),
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct Tag {
    pub(super) position: SourcedValue<SocXmlPosition>,
    pub(super) name: Attribute,
    #[serde(skip)]
    pub(super) name_uses: Vec<SourceUse>,
    priority: Attribute,
    sub_tag_def: Attribute,
}

pub(super) struct PendingDefinition {
    pub(super) owner: Owner,
    root: SocXmlPosition,
    root_name: String,
    known_root: bool,
    version: PendingAttribute,
    sections: Vec<PendingSection>,
}

struct PendingGroup {
    position: SocXmlPosition,
    name: PendingAttribute,
    tags: Vec<PendingTag>,
}

struct PendingTag {
    position: SocXmlPosition,
    name: PendingAttribute,
    priority: PendingAttribute,
    sub_tag_def: PendingAttribute,
}

impl Owner {
    pub(super) fn controller(
        document: usize,
        root: SourceElement<'_>,
        origin: &XmlOrigin<'_>,
    ) -> Result<Self, SocSceneError> {
        let mut edges = Vec::new();
        for child in root.elements().filter(|node| node.namespace().is_none()) {
            match child.name() {
                "Tags" => edges.push(PendingEdge::read(Role::ControllerTags, child, "filename")?),
                "Fragments" => edges.push(PendingEdge::read(
                    Role::ControllerFragments,
                    child,
                    "filename",
                )?),
                _ => {}
            }
        }
        Ok(Self {
            scope: origin.scope.clone(),
            document,
            edges,
        })
    }

    pub(super) fn database(
        document: usize,
        root: SourceElement<'_>,
        origin: &XmlOrigin<'_>,
    ) -> Result<Self, SocSceneError> {
        Ok(Self {
            scope: origin.scope.clone(),
            document,
            edges: vec![
                PendingEdge::read(Role::DatabaseTags, root, "TagDef")?,
                PendingEdge::read(Role::DatabaseFragments, root, "FragDef")?,
            ],
        })
    }
}

impl PendingDefinition {
    pub(super) fn read(
        document: usize,
        root: SourceElement<'_>,
        origin: &XmlOrigin<'_>,
        cancel: &CancelToken,
    ) -> Result<Self, SocSceneError> {
        let mut result = Self {
            owner: Owner {
                scope: origin.scope.clone(),
                document,
                edges: Vec::new(),
            },
            root: position(root)?,
            root_name: root.name().to_owned(),
            known_root: root.name() == "TagDefinition" && root.namespace().is_none(),
            version: PendingAttribute::read(root, "version")?,
            sections: Vec::new(),
        };
        if !result.known_root {
            return Ok(result);
        }
        for child in root.elements().filter(|node| node.namespace().is_none()) {
            match child.name() {
                "Imports" => {
                    for import in children(child, "Import") {
                        check(cancel)?;
                        result.owner.edges.push(PendingEdge::read(
                            Role::Import,
                            import,
                            "filename",
                        )?);
                    }
                }
                "Tags" => result.read_tags(child, cancel)?,
                _ => {}
            }
        }
        Ok(result)
    }

    fn read_tags(
        &mut self,
        list: SourceElement<'_>,
        cancel: &CancelToken,
    ) -> Result<(), SocSceneError> {
        let mut members = Vec::new();
        for row in list.elements().filter(|node| node.namespace().is_none()) {
            check(cancel)?;
            match row.name() {
                "Tag" => members.push(PendingMember::Tag(PendingTag::read(
                    row,
                    &mut self.owner.edges,
                )?)),
                "Group" => {
                    let mut tags = Vec::new();
                    for tag in children(row, "Tag") {
                        check(cancel)?;
                        tags.push(PendingTag::read(tag, &mut self.owner.edges)?);
                    }
                    members.push(PendingMember::Group(PendingGroup {
                        position: position(row)?,
                        name: PendingAttribute::read(row, "name")?,
                        tags,
                    }));
                }
                _ => {}
            }
        }
        self.sections.push(PendingSection {
            position: position(list)?,
            members,
        });
        Ok(())
    }

    pub(super) fn bind(
        &self,
        sources: &SourceUses,
        cancel: &CancelToken,
    ) -> Result<Definition, SocSceneError> {
        let origin = origin(sources, &self.owner.scope);
        let (position, root_use) = node(&origin, self.root.clone())?;
        let mut sections = Vec::with_capacity(self.sections.len());
        for section in &self.sections {
            check(cancel)?;
            let (position, _) = node(&origin, section.position.clone())?;
            let members = section
                .members
                .iter()
                .map(|member| member.bind(&origin, cancel))
                .collect::<Result<_, _>>()?;
            sections.push(Section { position, members });
        }
        let (version, evidence) = self
            .version
            .bind(&origin, &root_use, SourceUseRole::Payload)?;
        Ok(Definition {
            position,
            root_name: sources.value(
                SourceValue::Present(self.root_name.clone()),
                std::slice::from_ref(&root_use),
                true,
            ),
            version,
            sections,
            evidence,
        })
    }

    pub(super) const fn known_root(&self) -> bool {
        self.known_root
    }
}

impl PendingTag {
    fn read(tag: SourceElement<'_>, edges: &mut Vec<PendingEdge>) -> Result<Self, SocSceneError> {
        let sub_tag_def = PendingAttribute::read(tag, "subTagDef")?;
        if sub_tag_def.is_declared() {
            edges.push(PendingEdge::read(Role::FragmentTags, tag, "subTagDef")?);
        }
        Ok(Self {
            position: position(tag)?,
            name: PendingAttribute::read(tag, "name")?,
            priority: PendingAttribute::read(tag, "priority")?,
            sub_tag_def,
        })
    }
}

impl PendingMember {
    fn bind(&self, origin: &XmlOrigin<'_>, cancel: &CancelToken) -> Result<Member, SocSceneError> {
        check(cancel)?;
        Ok(match self {
            Self::Group(group) => {
                let (position, owner) = node(origin, group.position.clone())?;
                let tags = group
                    .tags
                    .iter()
                    .map(|tag| tag.bind(origin, cancel))
                    .collect::<Result<_, _>>()?;
                Member::Group {
                    position,
                    name: group.name.bind(origin, &owner, SourceUseRole::Payload)?.0,
                    tags,
                }
            }
            Self::Tag(tag) => Member::Tag(tag.bind(origin, cancel)?),
        })
    }
}

impl PendingTag {
    fn bind(&self, origin: &XmlOrigin<'_>, cancel: &CancelToken) -> Result<Tag, SocSceneError> {
        check(cancel)?;
        let (position, owner) = node(origin, self.position.clone())?;
        let (name, name_uses) = self.name.bind(origin, &owner, SourceUseRole::Payload)?;
        Ok(Tag {
            position,
            name,
            name_uses,
            priority: self
                .priority
                .bind(origin, &owner, SourceUseRole::Payload)?
                .0,
            sub_tag_def: self
                .sub_tag_def
                .bind(origin, &owner, SourceUseRole::Payload)?
                .0,
        })
    }
}

impl Definition {
    pub(super) fn tags(&self) -> impl Iterator<Item = &Tag> {
        self.sections.iter().flat_map(|section| {
            section.members.iter().flat_map(|member| match member {
                Member::Group { tags, .. } => tags.as_slice(),
                Member::Tag(tag) => std::slice::from_ref(tag),
            })
        })
    }
}

impl PendingEdge {
    fn read(
        role: Role,
        node: SourceElement<'_>,
        field: &'static str,
    ) -> Result<Self, SocSceneError> {
        Ok(Self {
            role,
            position: position(node)?,
            selector: PendingAttribute::read(node, field)?,
        })
    }
}

pub(super) fn node(
    origin: &XmlOrigin<'_>,
    position: SocXmlPosition,
) -> Result<(SourcedValue<SocXmlPosition>, SourceUse), SocSceneError> {
    let used = origin.at(&position, "$element", SourceUseRole::Payload)?;
    let value = origin.sources.value(
        SourceValue::Present(position),
        std::slice::from_ref(&used),
        true,
    );
    Ok((value, used))
}

pub(super) fn origin<'a>(sources: &'a SourceUses, scope: &SourceScope) -> XmlOrigin<'a> {
    XmlOrigin {
        sources,
        scope: scope.clone(),
        chunk: None,
        archive_member: None,
    }
}

pub(super) fn check(cancel: &CancelToken) -> Result<(), SocSceneError> {
    if cancel.is_cancelled() {
        Err(SocSceneError::Cancelled)
    } else {
        Ok(())
    }
}

fn position(node: SourceElement<'_>) -> Result<SocXmlPosition, SocSceneError> {
    SocXmlPosition::element(node.position())
}
