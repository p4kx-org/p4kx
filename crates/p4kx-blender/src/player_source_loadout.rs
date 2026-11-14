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
use crate::player_source_port_binding::{self, PortBinding};
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
        definition: Box<SourcedValue<PortBinding>>,
    },
}

pub(crate) fn read(
    archive: &Archive,
    sources: &SourceUses,
    directory: &SourceUse,
    classes: Classes<'_, '_>,
    documents: &mut XmlDocuments<'_>,
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
    documents: &mut XmlDocuments<'_>,
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
    let origin = XmlOrigin::with_archive_member(sources, &member)?;
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
        let payload = payload(SourceUseRole::Payload)?;
        documents.reject_malformed(&origin, &payload)?;
        uses.push(payload);
        return Ok(Loadout::InvalidXml {
            source,
            document: None,
        });
    };
    let root = origin.node(xml.root(), SourceUseRole::Payload)?;
    let document = documents.bind(&xml, &origin)?;
    let loadout = PositionedLoadout::read(&xml);
    if loadout.is_none() {
        documents.reject_root(&xml, &origin, &root)?;
    }
    uses.push(root);
    let Some(loadout) = loadout else {
        return Ok(Loadout::InvalidXml {
            source,
            document: Some(document),
        });
    };
    let declarations = declarations(&loadout, &origin, classes, cancel)?;
    Ok(Loadout::Present {
        document,
        declarations,
    })
}

fn declarations(
    loadout: &PositionedLoadout<'_>,
    origin: &XmlOrigin<'_>,
    classes: Classes<'_, '_>,
    cancel: &CancelToken,
) -> Result<Vec<Declaration>, PlayerSourceError> {
    let mut declarations = Vec::new();
    let mut original = Vec::new();
    for node in loadout.nodes() {
        check(cancel)?;
        let owner = node
            .parent()
            .and_then(|group| original.get(group))
            .and_then(|group: &PositionedLoadoutNode<'_>| group.parent())
            .and_then(|parent| original.get(parent).copied());
        declarations.push(Declaration::read(node, owner, origin, classes)?);
        original.push(node);
    }
    Ok(declarations)
}

impl Declaration {
    fn read(
        node: PositionedLoadoutNode<'_>,
        owner: Option<PositionedLoadoutNode<'_>>,
        origin: &XmlOrigin<'_>,
        classes: Classes<'_, '_>,
    ) -> Result<Self, PlayerSourceError> {
        let element = node.element();
        let position = SocXmlPosition::element(element.position())?;
        let parent = node.parent();
        if node.is_group() {
            return Ok(Self::Items { parent, position });
        }
        let port = attribute(element, "portName", origin)?;
        let item = attribute(element, "itemName", origin)?;
        let definition = read_definition(element, owner, &port, origin, classes)?;
        let target = if let Some((name, selector)) = item.selector {
            player_source_classes::by_name(origin.sources, classes, &selector, name)?
        } else {
            let site = origin.node(element, SourceUseRole::Selector)?;
            origin
                .sources
                .value(attribute_state(&item.value), &[site], true)
        };
        let item_name = item.value;
        Ok(Self::Item {
            parent,
            position,
            port: port.value,
            item_name,
            target: Box::new(target),
            definition: Box::new(definition),
        })
    }
}

fn read_definition(
    item: SourceElement<'_>,
    owner: Option<PositionedLoadoutNode<'_>>,
    port: &Attribute<'_>,
    origin: &XmlOrigin<'_>,
    classes: Classes<'_, '_>,
) -> Result<SourcedValue<PortBinding>, PlayerSourceError> {
    let Some((name, selector)) = &port.selector else {
        let used = origin.node(item, SourceUseRole::Selector)?;
        return Ok(origin
            .sources
            .value(attribute_state(&port.value), &[used], true));
    };
    let Some(owner) = owner else {
        return Ok(origin.sources.value(
            SourceValue::Present(PortBinding::NoOwner),
            std::slice::from_ref(selector),
            true,
        ));
    };
    let original = owner.element();
    let owner = attribute(original, "itemName", origin)?;
    if let Some((owner, owner_selector)) = owner.selector {
        return player_source_port_binding::named(
            origin.sources,
            classes,
            owner,
            name,
            &owner_selector,
            selector,
        );
    }
    let owner_selector = origin.node(original, SourceUseRole::Selector)?;
    let value = if owner.value.state() == p4kx_extraction::SourceValueState::Invalid {
        SourceValue::Invalid
    } else {
        SourceValue::Present(PortBinding::UndeclaredOwner)
    };
    Ok(origin
        .sources
        .value(value, &[selector.clone(), owner_selector], true))
}

fn attribute_state<T>(source: &SourcedValue<String>) -> SourceValue<T> {
    if source.state() == p4kx_extraction::SourceValueState::Invalid {
        SourceValue::Invalid
    } else {
        SourceValue::Absent
    }
}

struct Attribute<'document> {
    value: SourcedValue<String>,
    selector: Option<(&'document str, SourceUse)>,
}

fn attribute<'document>(
    element: SourceElement<'document>,
    name: &str,
    origin: &XmlOrigin<'_>,
) -> Result<Attribute<'document>, PlayerSourceError> {
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
    Ok(Attribute {
        value: origin.field(element, attributes(), value)?,
        selector,
    })
}
