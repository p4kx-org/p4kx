// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::cryxml::{SourceDocument, SourceElement};
use p4kx_extraction::{CancelToken, SourceScope};

use crate::soc_scene::SocSceneError;
use crate::soc_scene_class_mannequin::children;
use crate::soc_scene_classes::check;
use crate::soc_scene_origin::{SocXmlPosition, XmlOrigin};
use crate::soc_scene_rooms::position;

pub(crate) struct Draft {
    pub(super) scope: SourceScope,
    pub(super) root: SocXmlPosition,
    pub(super) lossy: bool,
    pub(super) contexts: Section<Node>,
    pub(super) scopes: Section<Node>,
    pub(super) fragments: Vec<Fragment>,
}

pub(super) struct Section<T> {
    pub(super) positions: Vec<SocXmlPosition>,
    pub(super) members: Vec<T>,
}

pub(super) struct Node {
    pub(super) position: SocXmlPosition,
    pub(super) name: String,
    pub(super) attributes: Vec<Attribute>,
}

pub(super) struct Attribute {
    pub(super) position: SocXmlPosition,
    pub(super) name: String,
    pub(super) namespace: Option<String>,
    pub(super) value: String,
}

pub(super) struct Fragment {
    pub(super) declaration: Node,
    pub(super) overrides: Vec<Node>,
}

impl Draft {
    pub(crate) fn read(
        source: &SourceDocument<'_>,
        origin: &XmlOrigin<'_>,
        cancel: &CancelToken,
    ) -> Result<Self, SocSceneError> {
        let root = source.root();
        Ok(Self {
            scope: origin.scope.clone(),
            root: position(root)?,
            lossy: source.is_lossy(),
            contexts: Section::read(root, "ScopeContextDefs", cancel, Node::read)?,
            scopes: Section::read(root, "ScopeDefs", cancel, Node::read)?,
            fragments: Section::read(root, "FragmentDefs", cancel, |node, cancel| {
                Ok(Fragment {
                    declaration: Node::read(node, cancel)?,
                    overrides: children(node, "Override")
                        .map(|node| Node::read(node, cancel))
                        .collect::<Result<_, _>>()?,
                })
            })?
            .members,
        })
    }
}

impl<T> Section<T> {
    fn read(
        root: SourceElement<'_>,
        name: &'static str,
        cancel: &CancelToken,
        mut project: impl FnMut(SourceElement<'_>, &CancelToken) -> Result<T, SocSceneError>,
    ) -> Result<Self, SocSceneError> {
        let mut positions = Vec::new();
        let mut members = Vec::new();
        for section in children(root, name) {
            check(cancel)?;
            positions.push(position(section)?);
            for child in section
                .elements()
                .filter(|child| child.namespace().is_none())
            {
                check(cancel)?;
                members.push(project(child, cancel)?);
            }
        }
        Ok(Self { positions, members })
    }
}

impl Node {
    fn read(node: SourceElement<'_>, cancel: &CancelToken) -> Result<Self, SocSceneError> {
        check(cancel)?;
        let attributes = node
            .attributes()
            .map(|attribute| {
                check(cancel)?;
                Ok(Attribute {
                    position: SocXmlPosition::attribute(attribute.position())?,
                    name: attribute.name().into(),
                    namespace: attribute.namespace().map(str::to_owned),
                    value: attribute.value().into(),
                })
            })
            .collect::<Result<_, SocSceneError>>()?;
        Ok(Self {
            position: position(node)?,
            name: node.name().into(),
            attributes,
        })
    }

    pub(super) fn attributes(&self, name: &str) -> impl Iterator<Item = &Attribute> {
        self.attributes
            .iter()
            .filter(move |attribute| attribute.namespace.is_none() && attribute.name == name)
    }
}
