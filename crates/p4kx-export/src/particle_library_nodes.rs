// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, SourceUseRole, SourceValue, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::check_cancel;
use crate::catalog_read::original::{EntryXml, SourcedXmlAttribute, XmlNodeSource};
use crate::schema_fields::every_field_required;
use crate::starmap_xml::{Node, Root, attribute};
use crate::{CatalogError, ExportAccountingError};

const PARTICLES: &str = "Particles";
const PARAMS: &str = "Params";
const RAIN_OCCLUSION: &str = "RainOcclusion";

/// One original particle-library element and its source-bound values.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct ParticleLibraryElement {
    pub(super) node_index: SourcedValue<u32>,
    pub(super) parent_node_index: SourcedValue<u32>,
    pub(super) name: SourcedValue<String>,
    pub(super) attributes: Vec<SourcedXmlAttribute>,
    #[schemars(extend("x-gameText" = true))]
    pub(super) text: Vec<SourcedValue<String>>,
    pub(super) rain_occlusion: Option<SourcedValue<bool>>,
}

struct Parent {
    source: XmlNodeSource,
    is_particles: bool,
}

pub(super) fn read(
    xml: &EntryXml<'_, '_, '_>,
    cancel: Option<&CancelToken>,
) -> Result<Vec<ParticleLibraryElement>, CatalogError> {
    let mut pending = vec![(xml.root(), None::<Parent>)];
    let mut elements = Vec::new();
    while let Some((node, parent)) = pending.pop() {
        check_cancel(cancel)?;
        let source = xml.node_source(node)?;
        elements.push(element(xml, node, &source, parent.as_ref())?);
        let first_child = pending.len();
        let is_particles = node.name().eq_ignore_ascii_case(PARTICLES);
        pending.extend(node.children().map(|child| {
            (
                child,
                Some(Parent {
                    source: source.clone(),
                    is_particles,
                }),
            )
        }));
        pending
            .get_mut(first_child..)
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?
            .reverse();
    }
    Ok(elements)
}

fn element(
    xml: &EntryXml<'_, '_, '_>,
    node: Root<'_>,
    source: &XmlNodeSource,
    parent: Option<&Parent>,
) -> Result<ParticleLibraryElement, CatalogError> {
    let sources = xml.capture().source_uses();
    let selected = source.selected()?;
    let node_use = std::slice::from_ref(&selected);
    let complete = !xml.is_lossy();
    let parent_node_index = if let Some(parent) = parent {
        sources.value(
            SourceValue::Present(parent.source.index()),
            &[selected.clone(), parent.source.selected()?],
            complete,
        )
    } else {
        sources.value(SourceValue::Absent, node_use, complete)
    };
    let rain_occlusion = (node.name().eq_ignore_ascii_case(PARAMS)
        && parent.is_some_and(|parent| parent.is_particles))
    .then(|| rain_occlusion(xml, node))
    .transpose()?;
    Ok(ParticleLibraryElement {
        node_index: sources.value(SourceValue::Present(source.index()), node_use, complete),
        parent_node_index,
        name: xml
            .name(node)?
            .retain_in(xml.capture())?
            .into_cell(sources)
            .map(|text| text.as_str().to_owned()),
        attributes: xml.sourced_attributes(node)?,
        text: xml.sourced_text(node)?,
        rain_occlusion,
    })
}

fn rain_occlusion(
    xml: &EntryXml<'_, '_, '_>,
    node: Root<'_>,
) -> Result<SourcedValue<bool>, CatalogError> {
    let source = xml.attribute_source(node, RAIN_OCCLUSION)?;
    let value = match attribute(node, RAIN_OCCLUSION).map(str::trim) {
        None => SourceValue::Absent,
        Some("1" | "true") => SourceValue::Present(true),
        Some("0" | "false") => SourceValue::Present(false),
        Some(_) => SourceValue::Invalid,
    };
    let uses = [source.observe(SourceUseRole::Conversion)?];
    Ok(xml
        .capture()
        .source_uses()
        .value(value, &uses, !xml.is_lossy()))
}
