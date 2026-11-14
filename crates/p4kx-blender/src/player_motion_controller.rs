// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;

use p4kx_extraction::cryxml::{SourceDocument, SourceElement};
use p4kx_extraction::{CancelToken, SourceUse, SourceUseRole};

use crate::soc_scene::SocSceneError;
use crate::soc_scene_class_mannequin::children;
use crate::soc_scene_origin::{SocXmlPosition, XmlOrigin};
use crate::soc_scene_rooms::position;

use super::{AuthoredFile, check};

pub(in super::super) struct Controller {
    pub(super) file: AuthoredFile,
    pub(super) fragments: Vec<Fragment>,
}

pub(super) struct Fragment {
    pub(super) name: String,
    pub(super) position: SocXmlPosition,
    pub(super) node_use: SourceUse,
    pub(super) scopes: Scopes,
}

pub(super) struct Scopes {
    pub(super) targets: Vec<ObservedScope>,
    pub(super) uses: Vec<SourceUse>,
    pub(super) valid: bool,
}

pub(super) struct ObservedScope {
    pub(super) data: ScopeTarget,
    pub(super) uses: Vec<SourceUse>,
}

#[derive(Clone, Debug, serde::Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct ScopeTarget {
    selector: String,
    candidates: Vec<SocXmlPosition>,
}

struct Definition {
    position: SocXmlPosition,
    node_use: SourceUse,
}

impl Controller {
    pub(in super::super) fn read(
        document: &SourceDocument<'_>,
        origin: &XmlOrigin<'_>,
        cancel: &CancelToken,
    ) -> Result<Self, SocSceneError> {
        let root = document.root();
        let file = AuthoredFile::at(children(root, "Fragments"), root, "filename", origin)?;
        let mut by_name = HashMap::<&str, Vec<Definition>>::new();
        for scope in section_members(root, "ScopeDefs") {
            check(cancel)?;
            by_name.entry(scope.name()).or_default().push(Definition {
                position: position(scope)?,
                node_use: origin.node(scope, SourceUseRole::Selector)?,
            });
        }
        let mut fragments = Vec::new();
        for node in section_members(root, "FragmentDefs") {
            check(cancel)?;
            fragments.push(Fragment {
                name: node.name().to_owned(),
                position: position(node)?,
                node_use: origin.node(node, SourceUseRole::Selector)?,
                scopes: Scopes::read(node, origin, &by_name)?,
            });
        }
        Ok(Self { file, fragments })
    }
}

impl Scopes {
    fn read(
        node: SourceElement<'_>,
        origin: &XmlOrigin<'_>,
        definitions: &HashMap<&str, Vec<Definition>>,
    ) -> Result<Self, SocSceneError> {
        let attributes: Vec<_> = node
            .attributes()
            .filter(|attribute| attribute.namespace().is_none() && attribute.name() == "scopes")
            .collect();
        let mut uses = vec![origin.node(node, SourceUseRole::Selector)?];
        for attribute in &attributes {
            uses.push(origin.attribute(attribute, SourceUseRole::Selector)?.1);
        }
        let mut result = Self {
            targets: Vec::new(),
            uses,
            valid: true,
        };
        match attributes.as_slice() {
            [] => {}
            [selector] if selector.value().is_empty() => {}
            [selector] => {
                let selector_use = origin.attribute(selector, SourceUseRole::Selector)?.1;
                for token in selector.value().split('+') {
                    let mut uses = vec![selector_use.clone()];
                    let definitions: &[Definition] =
                        definitions.get(token).map_or(&[], Vec::as_slice);
                    let candidates = definitions
                        .iter()
                        .map(|definition| {
                            uses.push(definition.node_use.clone());
                            definition.position.clone()
                        })
                        .collect();
                    result.uses.extend(uses.iter().cloned());
                    result.targets.push(ObservedScope {
                        data: ScopeTarget {
                            selector: token.to_owned(),
                            candidates,
                        },
                        uses,
                    });
                }
            }
            _ => {
                return Ok(Self {
                    targets: Vec::new(),
                    uses: result.uses,
                    valid: false,
                });
            }
        }
        Ok(result)
    }
}

fn section_members<'document>(
    root: SourceElement<'document>,
    section: &'static str,
) -> impl Iterator<Item = SourceElement<'document>> {
    children(root, section)
        .flat_map(|section| section.elements().filter(|node| node.namespace().is_none()))
}
