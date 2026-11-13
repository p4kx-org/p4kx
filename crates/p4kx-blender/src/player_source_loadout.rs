// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::{PositionedLoadout, PositionedLoadoutNode, SocArchiveMember, SocCaptureError};
use p4kx_extraction::cryxml::{SourceDocument, SourceElement, SourceReadOptions};
use p4kx_extraction::{
    Archive, CancelToken, Entry, PhysicalSourceRef, SourceLocation, SourceUse, SourceUseRole,
    SourceUses, SourceValue, SourcedValue,
};
use schemars::JsonSchema;
use serde::Serialize;

use crate::player_source::{PlayerSourceError, check};
use crate::player_source_classes::{self, ClassTarget, Classes};
use crate::soc_scene_origin::{SocXmlPosition, XmlOrigin};
use crate::soc_scene_xml::XmlDocuments;

const DEFAULT_LOADOUT_ROOT: &str = "data/scripts/loadouts/player/defaultloadouts/";

#[derive(Debug, Serialize, JsonSchema)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub(crate) enum Loadout {
    Unreadable {
        source: PhysicalSourceRef,
    },
    InvalidXml {
        source: PhysicalSourceRef,
        document: Option<usize>,
    },
    Present {
        document: usize,
        declarations: Vec<Declaration>,
    },
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub(crate) enum Declaration {
    Items {
        parent: Option<usize>,
        position: SocXmlPosition,
    },
    Item {
        parent: Option<usize>,
        position: SocXmlPosition,
        port: SourcedValue<String>,
        item_name: SourcedValue<String>,
        target: Box<SourcedValue<ClassTarget>>,
    },
}

pub(crate) fn read(
    archive: &Archive,
    sources: &SourceUses,
    directory: &SourceUse,
    classes: Classes<'_, '_>,
    documents: &mut XmlDocuments,
    cancel: &CancelToken,
) -> Result<Vec<SourcedValue<Loadout>>, PlayerSourceError> {
    let mut rows = Vec::new();
    for entry in archive.entries() {
        check(cancel)?;
        if !selected(entry.path()) {
            continue;
        }
        let mut uses = vec![directory.clone()];
        let value = project(
            archive, entry, sources, classes, documents, cancel, &mut uses,
        )?;
        let complete = !matches!(value, Loadout::Unreadable { .. });
        rows.push(sources.value(SourceValue::Present(value), &uses, complete));
    }
    Ok(rows)
}

fn selected(path: &str) -> bool {
    let path = path.replace('\\', "/");
    path.get(..DEFAULT_LOADOUT_ROOT.len())
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case(DEFAULT_LOADOUT_ROOT))
        && std::path::Path::new(&path)
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("xml"))
}

fn project(
    archive: &Archive,
    entry: &Entry,
    sources: &SourceUses,
    classes: Classes<'_, '_>,
    documents: &mut XmlDocuments,
    cancel: &CancelToken,
    uses: &mut Vec<SourceUse>,
) -> Result<Loadout, PlayerSourceError> {
    let member = SocArchiveMember::read(archive, entry, || {
        if cancel.is_cancelled() {
            Err(SocCaptureError::Cancelled)
        } else {
            Ok(())
        }
    })?;
    let origin = XmlOrigin::new(sources, member.origin(), None)?;
    let source = origin.source();
    let payload = |role| {
        origin.scope.at(
            SourceLocation::Metadata {
                field: "$payload".into(),
            },
            role,
        )
    };
    let Ok(bytes) = member.payload() else {
        uses.push(payload(SourceUseRole::Attempted)?);
        return Ok(Loadout::Unreadable { source });
    };
    let Ok(xml) = SourceDocument::parse(bytes, SourceReadOptions::default()) else {
        uses.push(payload(SourceUseRole::Payload)?);
        return Ok(Loadout::InvalidXml {
            source,
            document: None,
        });
    };
    uses.push(origin.node(xml.root(), SourceUseRole::Payload)?);
    let document = documents.bind(&xml, &origin)?;
    let Some(loadout) = PositionedLoadout::read(&xml) else {
        return Ok(Loadout::InvalidXml {
            source,
            document: Some(document),
        });
    };
    let mut declarations = Vec::new();
    for node in loadout.nodes() {
        check(cancel)?;
        declarations.push(Declaration::read(node, &origin, classes)?);
    }
    Ok(Loadout::Present {
        document,
        declarations,
    })
}

impl Declaration {
    fn read(
        node: PositionedLoadoutNode<'_>,
        origin: &XmlOrigin<'_>,
        classes: Classes<'_, '_>,
    ) -> Result<Self, PlayerSourceError> {
        let element = node.element();
        let position = SocXmlPosition::element(element.position())?;
        let parent = node.parent();
        if node.is_group() {
            return Ok(Self::Items { parent, position });
        }
        let port = crate::soc_scene_class_mannequin::string_attribute(element, "portName", origin)?;
        let item = item_attribute(element, origin)?;
        let target = if let Some((name, selector)) = item.selector {
            player_source_classes::by_name(origin.sources, classes, &selector, name)?
        } else {
            let site = origin.node(element, SourceUseRole::Selector)?;
            let state = if item.value.state() == p4kx_extraction::SourceValueState::Invalid {
                SourceValue::Invalid
            } else {
                SourceValue::Absent
            };
            origin.sources.value(state, &[site], true)
        };
        let item_name = item.value;
        Ok(Self::Item {
            parent,
            position,
            port,
            item_name,
            target: Box::new(target),
        })
    }
}

struct ItemAttribute<'document> {
    value: SourcedValue<String>,
    selector: Option<(&'document str, SourceUse)>,
}

fn item_attribute<'document>(
    element: SourceElement<'document>,
    origin: &XmlOrigin<'_>,
) -> Result<ItemAttribute<'document>, PlayerSourceError> {
    let name = "itemName";
    let attributes = || {
        element
            .attributes()
            .filter(|attribute| attribute.namespace().is_none() && attribute.name() == name)
    };
    let mut matches = attributes();
    let selected = match (matches.next(), matches.next()) {
        (None, _) => SourceValue::Absent,
        (Some(attribute), None) => SourceValue::Present(attribute),
        _ => SourceValue::Invalid,
    };
    let (value, selector) = match selected {
        SourceValue::Present(attribute) => {
            let selector = origin.attribute(&attribute, SourceUseRole::Selector)?.1;
            (
                SourceValue::Present(attribute.value().to_owned()),
                Some((attribute.value(), selector)),
            )
        }
        SourceValue::Absent => (SourceValue::Absent, None),
        _ => (SourceValue::Invalid, None),
    };
    Ok(ItemAttribute {
        value: origin.field(element, attributes(), value)?,
        selector,
    })
}
