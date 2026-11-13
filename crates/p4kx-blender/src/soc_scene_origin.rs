// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::cryxml::{
    SourceAttribute, SourceAttributePosition, SourceElement, SourceElementPosition,
};
use p4kx_extraction::{
    DecimalU64, EntryOrigin, FieldOrigin, InputScope, PhysicalSourceRef, SourceInput,
    SourceLocation, SourceScope, SourceUse, SourceUseRole, SourceUses, SourceValue, SourcedValue,
};
use p4kx_model::SocSourceValue;
use schemars::JsonSchema;
use serde::Serialize;

use crate::soc_scene::SocSceneError;

/// Original position within one XML document or one SOC chunk's XML body.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum SocXmlPosition {
    /// The half-open original byte range in text XML.
    Text {
        /// Start byte.
        start: DecimalU64,
        /// End byte.
        end: DecimalU64,
    },
    /// A physical node or attribute table location.
    CryXml {
        /// Original node table index.
        node: u32,
        /// Original attribute table index when the coordinate addresses an attribute.
        attribute: Option<u32>,
    },
}

impl SocXmlPosition {
    pub(crate) const fn element(position: SourceElementPosition) -> Result<Self, SocSceneError> {
        match position {
            SourceElementPosition::Text(range) => Ok(Self::text(range)),
            SourceElementPosition::CryXml(node) => Ok(Self::CryXml {
                node,
                attribute: None,
            }),
            _ => Err(SocSceneError::Position),
        }
    }

    pub(crate) fn attribute(position: &SourceAttributePosition) -> Result<Self, SocSceneError> {
        match position {
            SourceAttributePosition::Text(range) => Ok(Self::text(range.clone())),
            SourceAttributePosition::CryXml {
                node_index,
                attribute_index,
            } => Ok(Self::CryXml {
                node: *node_index,
                attribute: Some(*attribute_index),
            }),
            _ => Err(SocSceneError::Position),
        }
    }

    const fn text(range: std::ops::Range<usize>) -> Self {
        Self::Text {
            start: DecimalU64::new(range.start as u64),
            end: DecimalU64::new(range.end as u64),
        }
    }

    fn location(&self, chunk: Option<usize>, field: String) -> SourceLocation {
        let mut indices = Vec::new();
        let document = chunk.map_or("socXml", |chunk| {
            indices.push(chunk as u64);
            "socChunk"
        });
        let coordinate = match self {
            Self::Text { start, end } => {
                indices.extend([start.get(), end.get()]);
                "text"
            }
            Self::CryXml { node, attribute } => {
                indices.push(u64::from(*node));
                indices.extend(attribute.map(u64::from));
                "cryXml"
            }
        };
        SourceLocation::Subresource {
            namespace: format!("{document}.{coordinate}"),
            indices,
            field,
        }
    }
}

pub(crate) struct XmlOrigin<'a> {
    pub(crate) sources: &'a SourceUses,
    pub(crate) scope: SourceScope,
    pub(crate) chunk: Option<usize>,
}

impl<'a> XmlOrigin<'a> {
    pub(crate) fn new(
        sources: &'a SourceUses,
        input: &EntryOrigin,
        chunk: Option<usize>,
    ) -> Result<Self, SocSceneError> {
        let scope = sources.observe(SourceInput::Archive(input), InputScope::Payload)?;
        Ok(Self {
            sources,
            scope,
            chunk,
        })
    }

    pub(crate) fn source(&self) -> PhysicalSourceRef {
        self.scope.source_ref().clone()
    }

    pub(crate) fn node(
        &self,
        element: SourceElement<'_>,
        role: SourceUseRole,
    ) -> Result<SourceUse, SocSceneError> {
        let position = SocXmlPosition::element(element.position())?;
        self.at(&position, "$element", role)
    }

    pub(crate) fn attribute(
        &self,
        attribute: &SourceAttribute<'_>,
        role: SourceUseRole,
    ) -> Result<(SocXmlPosition, SourceUse), SocSceneError> {
        let position = SocXmlPosition::attribute(attribute.position())?;
        let used = self.at(&position, attribute.name(), role)?;
        Ok((position, used))
    }

    pub(crate) fn at(
        &self,
        position: &SocXmlPosition,
        field: &str,
        role: SourceUseRole,
    ) -> Result<SourceUse, SocSceneError> {
        Ok(self
            .scope
            .at(position.location(self.chunk, field.to_owned()), role)?)
    }

    pub(crate) fn field<'document, T>(
        &self,
        element: SourceElement<'_>,
        attributes: impl IntoIterator<Item = SourceAttribute<'document>>,
        value: SourceValue<T>,
    ) -> Result<SourcedValue<T>, SocSceneError> {
        let mut uses = vec![self.node(element, SourceUseRole::Payload)?];
        for attribute in attributes {
            uses.push(self.attribute(&attribute, SourceUseRole::Payload)?.1);
        }
        Ok(self.sources.value(value, &uses, true))
    }

    pub(crate) fn named<T>(
        &self,
        element: SourceElement<'_>,
        name: &str,
        value: SocSourceValue<T>,
    ) -> Result<SourcedValue<T>, SocSceneError> {
        let matching = element
            .attributes()
            .filter(|attr| attr.namespace().is_none() && attr.name() == name);
        self.field(element, matching, source_value(value))
    }

    pub(crate) fn node_origin(
        &self,
        element: SourceElement<'_>,
    ) -> Result<FieldOrigin, SocSceneError> {
        let used = self.node(element, SourceUseRole::Payload)?;
        Ok(self.sources.origin(&[used], true))
    }
}

pub(crate) fn source_value<T>(value: SocSourceValue<T>) -> SourceValue<T> {
    match value {
        SocSourceValue::Present(value) => SourceValue::Present(value),
        SocSourceValue::Absent => SourceValue::Absent,
        _ => SourceValue::Invalid,
    }
}
