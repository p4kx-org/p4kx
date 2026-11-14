// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::cryxml::SourceElement;
use p4kx_extraction::{CancelToken, SourceScope, SourceUse};

use crate::soc_scene::SocSceneError;
use crate::soc_scene_origin::SocXmlPosition;
use crate::soc_scene_rooms::position;

pub(crate) struct Draft {
    pub(super) scope: SourceScope,
    pub(super) fragments: Vec<Fragment>,
}

pub(crate) struct FragmentSite {
    pub(crate) list: usize,
    pub(crate) group: usize,
    pub(crate) option: usize,
}

pub(crate) struct FragmentInput<'document> {
    pub(crate) site: FragmentSite,
    pub(crate) group: SourceElement<'document>,
    pub(crate) option: SourceElement<'document>,
    pub(crate) group_use: SourceUse,
    pub(crate) node_use: SourceUse,
}

pub(super) struct Fragment {
    pub(super) site: FragmentSite,
    pub(super) group_position: SocXmlPosition,
    pub(super) group_use: SourceUse,
    pub(super) name: String,
    pub(super) position: SocXmlPosition,
    pub(super) node_use: SourceUse,
    pub(super) guid: Selector,
    pub(super) layers: Vec<Layer>,
}

pub(super) struct Layer {
    pub(super) ordinal: usize,
    pub(super) position: SocXmlPosition,
    pub(super) animations: Vec<Animation>,
}

pub(super) struct Animation {
    pub(super) ordinal: usize,
    pub(super) position: SocXmlPosition,
    pub(super) name: Selector,
}

pub(super) struct Selector {
    pub(super) name: &'static str,
    pub(super) occurrences: Vec<Attribute>,
}

pub(super) struct Attribute {
    pub(super) position: SocXmlPosition,
    pub(super) value: String,
}

impl Draft {
    pub(crate) const fn new(scope: SourceScope) -> Self {
        Self {
            scope,
            fragments: Vec::new(),
        }
    }

    pub(crate) fn fragment(
        &mut self,
        input: FragmentInput<'_>,
        cancel: &CancelToken,
    ) -> Result<(), SocSceneError> {
        let FragmentInput {
            site,
            group,
            option,
            group_use,
            node_use,
        } = input;
        let layers = ordered_children(option, "AnimLayer")
            .map(|(ordinal, layer)| Layer::read(ordinal, layer, cancel))
            .collect::<Result<_, _>>()?;
        self.fragments.push(Fragment {
            site,
            group_position: position(group)?,
            group_use,
            name: group.name().to_owned(),
            position: position(option)?,
            node_use,
            guid: Selector::read(option, "GUID")?,
            layers,
        });
        Ok(())
    }
}

impl Layer {
    fn read(
        ordinal: usize,
        layer: SourceElement<'_>,
        cancel: &CancelToken,
    ) -> Result<Self, SocSceneError> {
        check(cancel)?;
        let mut animations = Vec::new();
        for (ordinal, node) in ordered_children(layer, "Animation") {
            check(cancel)?;
            animations.push(Animation {
                ordinal,
                position: position(node)?,
                name: Selector::read(node, "name")?,
            });
        }
        Ok(Self {
            ordinal,
            position: position(layer)?,
            animations,
        })
    }
}

impl Selector {
    fn read(node: SourceElement<'_>, name: &'static str) -> Result<Self, SocSceneError> {
        let occurrences = node
            .attributes()
            .filter(|attribute| attribute.namespace().is_none() && attribute.name() == name)
            .map(|attribute| {
                Ok(Attribute {
                    position: SocXmlPosition::attribute(attribute.position())?,
                    value: attribute.value().to_owned(),
                })
            })
            .collect::<Result<_, SocSceneError>>()?;
        Ok(Self { name, occurrences })
    }
}

pub(crate) fn ordered_children<'document>(
    parent: SourceElement<'document>,
    name: &'static str,
) -> impl Iterator<Item = (usize, SourceElement<'document>)> {
    parent
        .elements()
        .enumerate()
        .filter(move |(_, node)| node.namespace().is_none() && node.name() == name)
}

pub(super) fn check(cancel: &CancelToken) -> Result<(), SocSceneError> {
    if cancel.is_cancelled() {
        Err(SocSceneError::Cancelled)
    } else {
        Ok(())
    }
}
