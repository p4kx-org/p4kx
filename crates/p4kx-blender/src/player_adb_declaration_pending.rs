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
    pub(super) branches: Vec<Branch>,
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
    pub(super) tags: Selector,
    pub(super) local_tags: Selector,
    pub(super) layers: Vec<Layer>,
}

pub(super) struct Branch {
    pub(super) ordinal: usize,
    pub(super) position: SocXmlPosition,
    pub(super) tags: Selector,
    pub(super) fragments: Vec<BranchFragment>,
}

pub(super) struct BranchFragment {
    pub(super) ordinal: usize,
    pub(super) position: SocXmlPosition,
    pub(super) name: Selector,
}

pub(super) struct Layer {
    pub(super) ordinal: usize,
    pub(super) position: SocXmlPosition,
    pub(super) animations: Vec<Animation>,
    pub(super) blends: Vec<Blend>,
}

pub(super) struct Animation {
    pub(super) ordinal: usize,
    pub(super) position: SocXmlPosition,
    pub(super) name: Selector,
    pub(super) flags: Selector,
    pub(super) preceding_blend: Option<usize>,
}

pub(super) struct Blend {
    pub(super) ordinal: usize,
    pub(super) position: SocXmlPosition,
    pub(super) duration: Selector,
    pub(super) start_time: Selector,
    pub(super) exit_time: Selector,
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
            branches: Vec::new(),
        }
    }

    pub(crate) fn branches(
        &mut self,
        root: SourceElement<'_>,
        cancel: &CancelToken,
    ) -> Result<(), SocSceneError> {
        for (_, section) in ordered_children(root, "SubADBs") {
            for (_, branch) in ordered_children(section, "SubADB") {
                check(cancel)?;
                let fragments = ordered_children(branch, "FragmentID")
                    .map(|(ordinal, fragment)| {
                        check(cancel)?;
                        Ok(BranchFragment {
                            ordinal,
                            position: position(fragment)?,
                            name: Selector::read(fragment, "Name")?,
                        })
                    })
                    .collect::<Result<_, SocSceneError>>()?;
                let ordinal = self.branches.len();
                self.branches.push(Branch {
                    ordinal,
                    position: position(branch)?,
                    tags: Selector::read(branch, "Tags")?,
                    fragments,
                });
            }
        }
        Ok(())
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
            tags: Selector::read(option, "Tags")?,
            local_tags: Selector::read(option, "FragTags")?,
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
        let mut blends = Vec::new();
        for (ordinal, node) in layer.elements().enumerate() {
            check(cancel)?;
            if node.namespace().is_some() {
                continue;
            }
            match node.name() {
                "Blend" => blends.push(Blend::read(ordinal, node)?),
                "Animation" => animations.push(Animation {
                    ordinal,
                    position: position(node)?,
                    name: Selector::read(node, "name")?,
                    flags: Selector::read(node, "flags")?,
                    preceding_blend: blends.len().checked_sub(1),
                }),
                _ => {}
            }
        }
        Ok(Self {
            ordinal,
            position: position(layer)?,
            animations,
            blends,
        })
    }
}

impl Blend {
    fn read(ordinal: usize, node: SourceElement<'_>) -> Result<Self, SocSceneError> {
        Ok(Self {
            ordinal,
            position: position(node)?,
            duration: Selector::read(node, "Duration")?,
            start_time: Selector::read(node, "StartTime")?,
            exit_time: Selector::read(node, "ExitTime")?,
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
