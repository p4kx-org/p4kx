// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::cryxml::{Format, SourceDocument, SourceElement};
use std::collections::{BTreeMap, BTreeSet};

use p4kx_extraction::{CancelToken, FieldOrigin, PhysicalSourceRef, SourceUse, SourceUses};
use schemars::JsonSchema;
use serde::Serialize;

use crate::player_adb_declarations::{AdbDeclarations, Draft};
use crate::player_source_motion::{ControllerDraft, ControllerTopology};
use crate::player_tag_definitions::{Seeds, TagDefinitions};
use crate::soc_scene::SocSceneError;
use crate::soc_scene_class_resources::Resources;
use crate::soc_scene_origin::{SocXmlPosition, XmlOrigin};
use crate::{PlayerDocument, PlayerDocumentRejection, PlayerDocumentSink, RejectedPlayerDocument};

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
    rejected: BTreeSet<(PhysicalSourceRef, PlayerDocumentRejection)>,
    adb_declarations: BTreeMap<usize, Draft>,
    controller_topology: BTreeMap<usize, ControllerDraft>,
    tag_definitions: Option<Seeds>,
    sink: Option<&'sink mut dyn PlayerDocumentSink>,
}

impl<'sink> XmlDocuments<'sink> {
    pub(crate) fn stream_to(sink: &'sink mut dyn PlayerDocumentSink) -> Self {
        Self {
            sink: Some(sink),
            tag_definitions: Some(Seeds::default()),
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

    pub(crate) fn reject_malformed(
        &mut self,
        origin: &XmlOrigin<'_>,
        payload: &SourceUse,
    ) -> Result<(), SocSceneError> {
        self.reject(origin, payload, PlayerDocumentRejection::MalformedXml)
    }

    pub(crate) fn reject_root(
        &mut self,
        document: &SourceDocument<'_>,
        origin: &XmlOrigin<'_>,
        root: &SourceUse,
    ) -> Result<(), SocSceneError> {
        self.reject(
            origin,
            root,
            PlayerDocumentRejection::UnexpectedRoot {
                format: source_format(document),
                lossy: document.is_lossy(),
            },
        )
    }

    fn reject(
        &mut self,
        origin: &XmlOrigin<'_>,
        evidence: &SourceUse,
        rejection: PlayerDocumentRejection,
    ) -> Result<(), SocSceneError> {
        let Some(sink) = self.sink.as_mut() else {
            return Ok(());
        };
        let source = origin.source();
        if !self.rejected.insert((source.clone(), rejection)) {
            return Ok(());
        }
        let member = origin.archive_member.ok_or(SocSceneError::Position)?;
        let bytes = member.payload().map_err(|_| SocSceneError::Position)?;
        sink.rejected_document(RejectedPlayerDocument {
            source,
            original: member.origin(),
            origin: origin.sources.origin(std::slice::from_ref(evidence), true),
            rejection,
            bytes,
        })
        .map_err(SocSceneError::DocumentSink)
    }

    pub(crate) fn adb_declaration(&self, origin: &XmlOrigin<'_>) -> Option<Draft> {
        self.sink.as_ref()?;
        let key = (origin.source(), origin.chunk);
        if self
            .indices
            .get(&key)
            .is_some_and(|id| self.adb_declarations.contains_key(id))
        {
            return None;
        }
        Some(Draft::new(origin.scope.clone()))
    }

    pub(crate) fn retain_adb_declaration(&mut self, index: usize, draft: Draft) {
        self.adb_declarations.insert(index, draft);
    }

    pub(crate) fn bind_adb_declarations(
        &mut self,
        sources: &SourceUses,
        cancel: &CancelToken,
    ) -> Result<Vec<AdbDeclarations>, SocSceneError> {
        std::mem::take(&mut self.adb_declarations)
            .into_iter()
            .map(|(index, draft)| AdbDeclarations::bind(index, draft, sources, cancel))
            .collect()
    }

    pub(crate) const fn is_streaming(&self) -> bool {
        self.sink.is_some()
    }

    pub(crate) fn retain_controller_topology(
        &mut self,
        index: usize,
        document: &SourceDocument<'_>,
        origin: &XmlOrigin<'_>,
        cancel: &CancelToken,
    ) -> Result<(), SocSceneError> {
        if self.is_streaming()
            && let std::collections::btree_map::Entry::Vacant(entry) =
                self.controller_topology.entry(index)
        {
            entry.insert(ControllerDraft::read(document, origin, cancel)?);
        }
        Ok(())
    }

    pub(crate) fn bind_controller_topology(
        &mut self,
        sources: &SourceUses,
        cancel: &CancelToken,
    ) -> Result<Vec<ControllerTopology>, SocSceneError> {
        std::mem::take(&mut self.controller_topology)
            .into_iter()
            .map(|(index, draft)| ControllerTopology::bind(index, draft, sources, cancel))
            .collect()
    }

    pub(crate) const fn tag_definitions(&mut self) -> Option<&mut Seeds> {
        self.tag_definitions.as_mut()
    }

    pub(crate) fn bind_tag_definitions(
        &mut self,
        resources: &Resources<'_>,
    ) -> Result<TagDefinitions, SocSceneError> {
        self.tag_definitions.take().map_or_else(
            || Ok(TagDefinitions::default()),
            |seeds| seeds.finish(resources, self),
        )
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
