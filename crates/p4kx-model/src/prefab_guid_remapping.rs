// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_cryxml::{SourceAttributePosition, SourceElementPosition};

use crate::mtl_xml::XmlElement;

/// One authored entry in a nested prefab's GUID remapping.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct PrefabGuidRemapping {
    /// The original `defaultGUID` of the target's object, when authored.
    pub default_guid: Option<String>,
    /// The original `instanceGUID` for this placement, when authored.
    pub instance_guid: Option<String>,
}

/// Original XML sites of one remapping block.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct PrefabGuidRemappingBlockSource {
    /// The owning `GUIDRemapping` element.
    pub element: SourceElementPosition,
    /// The original pair elements in file order.
    pub pairs: Vec<PrefabGuidRemappingSource>,
}

/// Original XML sites of one authored GUID pair.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct PrefabGuidRemappingSource {
    /// The owning `Object` element.
    pub element: SourceElementPosition,
    /// The original `defaultGUID` attribute.
    pub default_guid: Option<SourceAttributePosition>,
    /// The original `instanceGUID` attribute.
    pub instance_guid: Option<SourceAttributePosition>,
}

pub(crate) fn read<'a>(
    instance: impl XmlElement<'a>,
    mut sources: Option<&mut Vec<PrefabGuidRemappingBlockSource>>,
) -> Vec<Vec<PrefabGuidRemapping>> {
    let mut blocks = Vec::new();
    for block in instance
        .children()
        .filter(|node| node.name().eq_ignore_ascii_case("GUIDRemapping"))
    {
        let mut pairs = Vec::new();
        let mut positions = Vec::new();
        for pair in block
            .children()
            .filter(|node| node.name().eq_ignore_ascii_case("Object"))
        {
            pairs.push(PrefabGuidRemapping {
                default_guid: pair.attribute("defaultGUID").map(str::to_owned),
                instance_guid: pair.attribute("instanceGUID").map(str::to_owned),
            });
            if sources.is_some() {
                positions.push(PrefabGuidRemappingSource {
                    element: pair.position(),
                    default_guid: pair.attribute_position("defaultGUID"),
                    instance_guid: pair.attribute_position("instanceGUID"),
                });
            }
        }
        blocks.push(pairs);
        if let Some(sources) = sources.as_deref_mut() {
            sources.push(PrefabGuidRemappingBlockSource {
                element: block.position(),
                pairs: positions,
            });
        }
    }
    blocks
}
