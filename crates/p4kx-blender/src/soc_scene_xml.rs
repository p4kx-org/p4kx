// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::cryxml::{Format, SourceDocument, SourceElement};
use std::collections::BTreeMap;

use p4kx_extraction::{FieldOrigin, PhysicalSourceRef};
use schemars::JsonSchema;
use serde::Serialize;

use crate::soc_scene::SocSceneError;
use crate::soc_scene_origin::{SocXmlPosition, XmlOrigin};

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct XmlDocument {
    source: PhysicalSourceRef,
    chunk: Option<usize>,
    format: String,
    lossy: bool,
    #[serde(flatten)]
    origin: FieldOrigin,
    nodes: Vec<XmlNode>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct XmlNode {
    position: SocXmlPosition,
    parent: Option<usize>,
    ordinal: usize,
    name: String,
    namespace: Option<String>,
    contains_encrypted_source: Option<bool>,
    attributes: Vec<XmlAttribute>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct XmlAttribute {
    position: SocXmlPosition,
    name: String,
    namespace: Option<String>,
    contains_encrypted_source: Option<bool>,
    value: String,
}

#[derive(Default)]
pub(crate) struct XmlDocuments {
    indices: BTreeMap<(PhysicalSourceRef, Option<usize>), usize>,
    rows: Vec<XmlDocument>,
}

impl XmlDocuments {
    pub(crate) fn bind(
        &mut self,
        document: &SourceDocument<'_>,
        origin: &XmlOrigin<'_>,
    ) -> Result<usize, SocSceneError> {
        let key = (origin.source(), origin.chunk);
        if let Some(&index) = self.indices.get(&key) {
            return Ok(index);
        }
        let index = self.rows.len();
        self.rows.push(XmlDocument::read(document, origin)?);
        self.indices.insert(key, index);
        Ok(index)
    }

    pub(crate) fn finish(self) -> Vec<XmlDocument> {
        self.rows
    }
}

impl XmlDocument {
    pub(crate) fn read(
        document: &SourceDocument<'_>,
        origin: &XmlOrigin<'_>,
    ) -> Result<Self, SocSceneError> {
        let format = match document.format() {
            Format::CryXml(signature) => signature.as_str(),
            Format::Text => "textXml",
        };
        let document_origin = origin.node_origin(document.root())?;
        let contains_encrypted_source = document_origin.contains_encrypted_source();
        let mut pending = vec![(None, 0, document.root())];
        let mut nodes = Vec::new();
        while let Some((parent, ordinal, element)) = pending.pop() {
            let position = nodes.len();
            nodes.push(XmlNode::read(
                element,
                parent,
                ordinal,
                contains_encrypted_source,
            )?);
            let start = pending.len();
            pending.extend(
                element
                    .elements()
                    .enumerate()
                    .map(|(ordinal, child)| (Some(position), ordinal, child)),
            );
            if let Some(children) = pending.get_mut(start..) {
                children.reverse();
            }
        }
        Ok(Self {
            source: origin.source(),
            chunk: origin.chunk,
            format: format.to_owned(),
            lossy: document.is_lossy(),
            origin: document_origin,
            nodes,
        })
    }
}

impl XmlNode {
    fn read(
        element: SourceElement<'_>,
        parent: Option<usize>,
        ordinal: usize,
        contains_encrypted_source: Option<bool>,
    ) -> Result<Self, SocSceneError> {
        let attributes = element
            .attributes()
            .map(|attribute| {
                Ok(XmlAttribute {
                    position: SocXmlPosition::attribute(attribute.position())?,
                    name: attribute.name().to_owned(),
                    namespace: attribute.namespace().map(str::to_owned),
                    contains_encrypted_source,
                    value: attribute.value().to_owned(),
                })
            })
            .collect::<Result<_, SocSceneError>>()?;
        Ok(Self {
            position: SocXmlPosition::element(element.position())?,
            parent,
            ordinal,
            name: element.name().to_owned(),
            namespace: element.namespace().map(str::to_owned),
            contains_encrypted_source,
            attributes,
        })
    }
}
