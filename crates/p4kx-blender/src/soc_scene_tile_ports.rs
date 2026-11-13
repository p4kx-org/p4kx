// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::{SocTilePortIndex, SocTilePortSite, SocTilePortTarget};
use p4kx_extraction::cryxml::SourceElement;
use p4kx_extraction::{SourceUseRole, SourceValue, SourcedValue};
use p4kx_model::{SocContainerDescriptor, SocSourceValue, SocTileAttachment};
use schemars::JsonSchema;
use serde::Serialize;

use crate::soc_scene::SocSceneError;
use crate::soc_scene_origin::{SocXmlPosition, XmlOrigin};
use crate::soc_scene_rooms::position;

pub(crate) struct TilePorts<'a> {
    index: SocTilePortIndex<'a>,
    root: SourceElement<'a>,
    origin: &'a XmlOrigin<'a>,
    complete: bool,
}

impl<'a> TilePorts<'a> {
    pub(crate) fn new(
        descriptor: &'a SocContainerDescriptor<'_>,
        origin: &'a XmlOrigin<'a>,
    ) -> Self {
        Self {
            index: SocTilePortIndex::read(descriptor),
            root: descriptor.document().root(),
            origin,
            complete: !descriptor.document().is_lossy(),
        }
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TileAttachment {
    position: SocXmlPosition,
    port_name: SourcedValue<String>,
    port_index: SourcedValue<p4kx_extraction::DecimalU64>,
    attachment_point_suffix: SourcedValue<String>,
    target: SourcedValue<TileTarget>,
}

impl TileAttachment {
    pub(crate) fn read(
        attachment: SocTileAttachment<'_>,
        ports: Option<&TilePorts<'_>>,
        origin: &XmlOrigin<'_>,
    ) -> Result<Self, SocSceneError> {
        let element = attachment.element();
        Ok(Self {
            position: position(element)?,
            port_name: origin
                .named(element, "ItemPortName", attachment.item_port())?
                .map(str::to_owned),
            port_index: origin
                .named(element, "ItemPortIndex", attachment.item_port_index())?
                .map(p4kx_extraction::DecimalU64::new),
            attachment_point_suffix: origin
                .named(
                    element,
                    "ItemPortName",
                    attachment.attachment_point_suffix(),
                )?
                .map(str::to_owned),
            target: target(attachment, ports, origin)?,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(tag = "type", rename_all = "camelCase")]
enum TileTarget {
    DescriptorUnavailable,
    Unmatched,
    Matched { port: Port },
    Ambiguous { ports: Vec<Port> },
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct Port {
    position: SocXmlPosition,
    interaction_offset: SourcedValue<[f64; 3]>,
}

impl Port {
    fn read(site: &SocTilePortSite<'_>, origin: &XmlOrigin<'_>) -> Result<Self, SocSceneError> {
        Ok(Self {
            position: position(site.port.element())?,
            interaction_offset: origin.field(
                site.port.element(),
                site.port.offset_attributes(),
                crate::soc_scene_origin::source_value(site.offset),
            )?,
        })
    }
}

fn target(
    attachment: SocTileAttachment<'_>,
    ports: Option<&TilePorts<'_>>,
    origin: &XmlOrigin<'_>,
) -> Result<SourcedValue<TileTarget>, SocSceneError> {
    let mut uses = vec![origin.node(attachment.element(), SourceUseRole::Selector)?];
    for attribute in attachment.item_port_attributes() {
        uses.push(origin.attribute(&attribute, SourceUseRole::Selector)?.1);
    }
    let (value, complete) = if let Some(ports) = ports {
        let (value, sites) = project(ports.index.resolve(attachment), ports.origin)?;
        if matches!(value, SourceValue::Present(_)) {
            uses.push(ports.origin.node(ports.root, SourceUseRole::Payload)?);
        }
        for site in sites {
            for attribute in site.port.name_attributes() {
                uses.push(
                    ports
                        .origin
                        .attribute(&attribute, SourceUseRole::Payload)?
                        .1,
                );
            }
        }
        (value, ports.complete)
    } else {
        match attachment.item_port() {
            SocSourceValue::Present(_) => (
                SourceValue::Present(TileTarget::DescriptorUnavailable),
                false,
            ),
            SocSourceValue::Absent => (SourceValue::Absent, true),
            _ => (SourceValue::Invalid, true),
        }
    };
    Ok(origin.sources.value(value, &uses, complete))
}

fn project<'a, 'document>(
    target: SocTilePortTarget<'a, 'document>,
    origin: &XmlOrigin<'_>,
) -> Result<(SourceValue<TileTarget>, &'a [SocTilePortSite<'document>]), SocSceneError> {
    let projected = match target {
        SocTilePortTarget::Absent => return Ok((SourceValue::Absent, &[])),
        SocTilePortTarget::Invalid => return Ok((SourceValue::Invalid, &[])),
        SocTilePortTarget::Unmatched => (TileTarget::Unmatched, &[][..]),
        SocTilePortTarget::Matched(site) => (
            TileTarget::Matched {
                port: Port::read(site, origin)?,
            },
            std::slice::from_ref(site),
        ),
        SocTilePortTarget::Ambiguous(sites) => (
            TileTarget::Ambiguous {
                ports: sites
                    .iter()
                    .map(|site| Port::read(site, origin))
                    .collect::<Result<_, _>>()?,
            },
            sites,
        ),
        _ => return Ok((SourceValue::Unsupported, &[])),
    };
    Ok((SourceValue::Present(projected.0), projected.1))
}
