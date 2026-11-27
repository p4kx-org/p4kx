// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_cryxml::{SourceAttributePosition, SourceElementPosition};

use crate::mtl_xml::XmlElement;

/// One authored group of entity-link declarations in the owning `Object`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct PrefabEntityLinkGroup {
    /// Direct `Link` children in source order.
    pub links: Vec<PrefabEntityLink>,
}

/// Literal attributes of one authored Prefab link.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct PrefabEntityLink {
    /// The original `Name` attribute, including an explicit empty value.
    pub name: Option<String>,
    /// The original `TargetId` attribute as authoring text.
    pub target_id: Option<String>,
    /// The original `Version` attribute as authoring text.
    pub version: Option<String>,
}

/// Original XML coordinates of one `EntityLinks` group.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct PrefabEntityLinkGroupSource {
    /// The group element enclosing the direct `Link` children.
    pub element: SourceElementPosition,
    /// Child positions in the order of the projected links.
    pub links: Vec<PrefabEntityLinkSource>,
    /// Whether an excluded direct child has no retained selector position.
    pub has_other_children: bool,
}

/// Original XML coordinates of the groups owned by one `Object`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct PrefabEntityLinksSource {
    /// Group positions in the order of the projected groups.
    pub groups: Vec<PrefabEntityLinkGroupSource>,
    /// Whether an excluded direct child has no retained selector position.
    pub has_other_children: bool,
}

/// Original XML coordinates of one `Link`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct PrefabEntityLinkSource {
    /// The `Link` element, including the scope of absent attributes.
    pub element: SourceElementPosition,
    /// The original `Name` attribute when present.
    pub name: Option<SourceAttributePosition>,
    /// The original `TargetId` attribute when present.
    pub target_id: Option<SourceAttributePosition>,
    /// The original `Version` attribute when present.
    pub version: Option<SourceAttributePosition>,
}

pub(crate) fn read<'a>(
    object: impl XmlElement<'a>,
    capture: bool,
) -> (
    Box<[PrefabEntityLinkGroup]>,
    Option<Box<PrefabEntityLinksSource>>,
) {
    let mut groups = Vec::new();
    let mut positions = Vec::new();
    let mut has_other_children = false;
    for child in object.children() {
        if !child.name().eq_ignore_ascii_case("EntityLinks") {
            has_other_children = true;
            continue;
        }
        let (group, source) = read_group(child, capture);
        groups.push(group);
        positions.extend(source);
    }
    let source = (capture && !groups.is_empty()).then(|| {
        Box::new(PrefabEntityLinksSource {
            groups: positions,
            has_other_children,
        })
    });
    (groups.into_boxed_slice(), source)
}

fn read_group<'a>(
    group: impl XmlElement<'a>,
    capture: bool,
) -> (PrefabEntityLinkGroup, Option<PrefabEntityLinkGroupSource>) {
    let mut links = Vec::new();
    let mut positions = Vec::new();
    let mut has_other_children = false;
    for child in group.children() {
        if !child.name().eq_ignore_ascii_case("Link") {
            has_other_children = true;
            continue;
        }
        links.push(PrefabEntityLink {
            name: child.attribute("Name").map(str::to_owned),
            target_id: child.attribute("TargetId").map(str::to_owned),
            version: child.attribute("Version").map(str::to_owned),
        });
        if capture {
            positions.push(PrefabEntityLinkSource {
                element: child.position(),
                name: child.attribute_position("Name"),
                target_id: child.attribute_position("TargetId"),
                version: child.attribute_position("Version"),
            });
        }
    }
    let source = capture.then(|| PrefabEntityLinkGroupSource {
        element: group.position(),
        links: positions,
        has_other_children,
    });
    (PrefabEntityLinkGroup { links }, source)
}
