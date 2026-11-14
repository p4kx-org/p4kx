// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::cryxml::{Format, SourceDocument, SourceElement};
use std::collections::BTreeMap;

use p4kx_extraction::{FieldOrigin, PhysicalSourceRef};
use schemars::JsonSchema;
use serde::Serialize;

use crate::soc_scene::SocSceneError;
use crate::soc_scene_origin::{SocXmlPosition, XmlOrigin};
use crate::{PlayerDocument, PlayerDocumentSink};

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
pub(crate) struct XmlDocuments<'sink> {
    indices: BTreeMap<(PhysicalSourceRef, Option<usize>), usize>,
    rows: Vec<XmlDocument>,
    sink: Option<&'sink mut dyn PlayerDocumentSink>,
}

impl<'sink> XmlDocuments<'sink> {
    pub(crate) fn stream_to(sink: &'sink mut dyn PlayerDocumentSink) -> Self {
        Self {
            sink: Some(sink),
            ..Self::default()
        }
    }

    pub(crate) fn bind(
        &mut self,
        document: &SourceDocument<'_>,
        origin: &XmlOrigin<'_>,
    ) -> Result<usize, SocSceneError> {
        let key = (origin.source(), origin.chunk);
        if let Some(&index) = self.indices.get(&key) {
            return Ok(index);
        }
        let index = self.indices.len();
        if let Some(sink) = self.sink.as_mut() {
            let member = origin.archive_member.ok_or(SocSceneError::Position)?;
            let bytes = member.payload().map_err(|_| SocSceneError::Position)?;
            sink.document(PlayerDocument {
                index,
                source: origin.source(),
                original: member.origin(),
                origin: origin.node_origin(document.root())?,
                format: source_format(document),
                lossy: document.is_lossy(),
                bytes,
            })
            .map_err(SocSceneError::DocumentSink)?;
        } else {
            self.rows.push(XmlDocument::read(document, origin)?);
        }
        self.indices.insert(key, index);
        Ok(index)
    }

    pub(crate) fn finish(self) -> Vec<XmlDocument> {
        self.rows
    }
}

const fn source_format(document: &SourceDocument<'_>) -> &'static str {
    match document.format() {
        Format::CryXml(signature) => signature.as_str(),
        Format::Text => "textXml",
    }
}

impl XmlDocument {
    pub(crate) fn read(
        document: &SourceDocument<'_>,
        origin: &XmlOrigin<'_>,
    ) -> Result<Self, SocSceneError> {
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
            format: source_format(document).to_owned(),
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
