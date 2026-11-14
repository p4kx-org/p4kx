// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::cryxml::{
    SourceAttribute, SourceAttributePosition, SourceElement, SourceElementPosition,
};
use p4kx_extraction::model::character_definition;
use p4kx_extraction::{InputScope, SourceUseRole};

use super::origins::{self, ModelOrigins};
use crate::origin_capture::OriginSite;
use crate::origin_contributions::OriginContributions;
use crate::origin_prepared::PreparedRecordOrigins;
use crate::{MediaError, MediaWriter};

pub(super) struct CharacterElement<'a, 'document> {
    element: SourceElement<'document>,
    origins: &'a ModelOrigins<'a>,
}

impl<'a, 'document> CharacterElement<'a, 'document> {
    pub(super) const fn new(
        element: SourceElement<'document>,
        origins: &'a ModelOrigins<'a>,
    ) -> Self {
        Self { element, origins }
    }

    pub(super) fn prepare(
        &self,
        writer: &MediaWriter<'_>,
    ) -> Result<PreparedRecordOrigins, MediaError> {
        let mut prepared = self.origins.prepared()?;
        let evidence = self.element_origin("element", writer)?;
        origins::at(&mut prepared, "/subresource/indices", evidence, writer)?;
        Ok(prepared)
    }

    pub(super) fn value(
        &self,
        name: &str,
        writer: &MediaWriter<'_>,
    ) -> Result<(Option<&'document str>, OriginContributions), MediaError> {
        let Some(attribute) = character_definition::attribute(self.element, name) else {
            return Ok((None, self.element_origin(name, writer)?));
        };
        let evidence = self.attribute_origin(&attribute, writer)?;
        Ok((Some(attribute.value()), evidence))
    }

    pub(super) fn field(
        &self,
        name: &str,
        path: &str,
        prepared: &mut PreparedRecordOrigins,
        writer: &MediaWriter<'_>,
    ) -> Result<Option<&'document str>, MediaError> {
        let (value, evidence) = self.value(name, writer)?;
        origins::at(prepared, path, evidence, writer)?;
        Ok(value)
    }

    pub(super) fn offset(
        &self,
        name: &str,
        pointer: &str,
        kind: &OriginContributions,
        prepared: &mut PreparedRecordOrigins,
        writer: &MediaWriter<'_>,
    ) -> Result<Option<&'document str>, MediaError> {
        let (authored, source) = self.value(name, writer)?;
        origins::at(prepared, pointer, source.clone(), writer)?;
        origins::at(
            prepared,
            &format!("{pointer}/authored"),
            source.clone(),
            writer,
        )?;
        let mut decoded = writer.capture_origins();
        decoded.include_original(&source)?;
        decoded.include_original(kind)?;
        origins::at(
            prepared,
            &format!("{pointer}/value"),
            decoded.finish(),
            writer,
        )?;
        Ok(authored)
    }
    fn attribute_origin(
        &self,
        attribute: &SourceAttribute<'_>,
        writer: &MediaWriter<'_>,
    ) -> Result<OriginContributions, MediaError> {
        let name = attribute.name();
        let role = SourceUseRole::Payload;
        match attribute.position() {
            SourceAttributePosition::Text(range) => {
                let indices = [range.start as u64, range.end as u64];
                self.capture(
                    OriginSite::new("cdf-xml-attribute", &indices, name),
                    role,
                    writer,
                )
            }
            SourceAttributePosition::CryXml {
                node_index,
                attribute_index,
            } => {
                let indices = [u64::from(*node_index), u64::from(*attribute_index)];
                self.capture(
                    OriginSite::new("cdf-cryxml-attribute", &indices, name),
                    role,
                    writer,
                )
            }
            _ => Err(MediaError::Contract(
                "unknown CDF attribute source coordinate".into(),
            )),
        }
    }

    pub(super) fn element_origin(
        &self,
        name: &str,
        writer: &MediaWriter<'_>,
    ) -> Result<OriginContributions, MediaError> {
        let role = SourceUseRole::Selector;
        match self.element.position() {
            SourceElementPosition::Text(range) => {
                let indices = [range.start as u64, range.end as u64];
                self.capture(
                    OriginSite::new("cdf-xml-element", &indices, name),
                    role,
                    writer,
                )
            }
            SourceElementPosition::CryXml(index) => {
                let indices = [u64::from(index)];
                self.capture(
                    OriginSite::new("cdf-cryxml-element", &indices, name),
                    role,
                    writer,
                )
            }
            _ => Err(MediaError::Contract(
                "unknown CDF element source coordinate".into(),
            )),
        }
    }

    fn capture(
        &self,
        site: OriginSite<'_>,
        role: SourceUseRole,
        writer: &MediaWriter<'_>,
    ) -> Result<OriginContributions, MediaError> {
        let mut capture = writer.capture_origins();
        capture.add(self.origins.input, InputScope::Payload, site, role, writer)?;
        Ok(capture.finish())
    }
}
