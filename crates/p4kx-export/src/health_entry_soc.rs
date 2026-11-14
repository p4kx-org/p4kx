// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::cryxml::{SourceAttribute, SourceAttributePosition, SourceElementPosition};
use p4kx_extraction::model::{SocEntity, SocEntityChunk, SocEntityFile, SocEntitySourceError};
use p4kx_extraction::{SourceLocation, SourceUse, SourceUseRole, SourceValue};

use super::health_entry_metadata::EntryMetadata;
use super::{EntryCapture, EntryRead, OriginalDatum};
use crate::{CatalogError, ExportAccountingError};

/// Original SOC entities bound to one admitted archive payload.
pub(crate) struct EntrySoc<'read> {
    file: SocEntityFile<'read>,
    origin: &'read EntryMetadata,
}

impl<'read> EntrySoc<'read> {
    pub(crate) fn parse(
        read: &'read EntryRead,
        capture: &EntryCapture<'_>,
    ) -> Result<Result<Self, SocEntitySourceError>, CatalogError> {
        read.receipt().check_capture(capture)?;
        let origin = read.receipt().payload_entry()?;
        Ok(SocEntityFile::parse(read.complete_bytes()?).map(|file| Self { file, origin }))
    }

    pub(crate) fn chunks(
        &self,
    ) -> impl Iterator<Item = Result<EntrySocChunk<'_>, SocEntitySourceError>> {
        self.file.entity_chunks().map(|chunk| {
            chunk.map(|chunk| EntrySocChunk {
                chunk,
                origin: self.origin,
            })
        })
    }
}

pub(crate) struct EntrySocChunk<'read> {
    chunk: SocEntityChunk<'read>,
    origin: &'read EntryMetadata,
}

impl EntrySocChunk<'_> {
    pub(crate) const fn is_lossy(&self) -> bool {
        self.chunk.document().is_lossy()
    }

    pub(crate) fn entities(&self) -> impl Iterator<Item = EntrySocEntity<'_>> {
        self.chunk.entities().map(|entity| EntrySocEntity {
            entity,
            origin: self.origin,
            index: self.chunk.chunk_index(),
            offset: self.chunk.header().offset(),
            complete: !self.is_lossy(),
        })
    }
}

pub(crate) struct EntrySocEntity<'read> {
    entity: SocEntity<'read>,
    origin: &'read EntryMetadata,
    index: usize,
    offset: u64,
    complete: bool,
}

pub(crate) struct SocSourceFields {
    pub(crate) chunk_index: u32,
    pub(crate) entity_ordinal: u32,
    pub(crate) selection: SourceUse,
    pub(crate) class_selection: SourceUse,
    pub(crate) complete: bool,
    pub(crate) attributes: Vec<SocSourceAttribute>,
    pub(crate) local_transform: OriginalDatum<[[f32; 4]; 3]>,
}

pub(crate) struct SocSourceAttribute {
    pub(crate) name: OriginalDatum<String>,
    pub(crate) value: OriginalDatum<String>,
}

impl<'read> EntrySocEntity<'read> {
    pub(crate) fn class_guids(&self) -> impl Iterator<Item = &'read str> {
        self.entity.element().attributes().filter_map(|attribute| {
            (attribute.namespace().is_none() && attribute.name() == "EntityClassGUID")
                .then_some(attribute.value())
        })
    }

    pub(crate) fn fields(&self) -> Result<SocSourceFields, CatalogError> {
        let chunk_index = ordinal(self.index)?;
        let entity_ordinal = ordinal(self.entity.ordinal())?;
        let node = self.node_location(chunk_index)?;
        let selection = self
            .origin
            .payload_at(node.clone(), SourceUseRole::Selector)?;
        let mut transform_uses = vec![self.origin.payload_at(node, SourceUseRole::Conversion)?];
        let mut class_selection = None;
        let mut attributes = Vec::new();
        for attribute in self.entity.element().attributes() {
            let name = attribute.name();
            let location = self.attribute_location(chunk_index, name, attribute.position())?;
            attributes.push(self.authored_attribute(&attribute, location.clone())?);
            if attribute.namespace().is_none() {
                match name {
                    "EntityClassGUID" => {
                        let used = self.origin.payload_at(location, SourceUseRole::Selector)?;
                        if class_selection.replace(used).is_some() {
                            return Err(ExportAccountingError::ConflictingSourceDatum.into());
                        }
                    }
                    "Pos" | "Rotate" | "Scale" => {
                        transform_uses.push(
                            self.origin
                                .payload_at(location, SourceUseRole::Conversion)?,
                        );
                    }
                    _ => {}
                }
            }
        }
        let class_selection =
            class_selection.ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        let value = self
            .entity
            .local_transform()
            .map_or(SourceValue::Invalid, |matrix| {
                SourceValue::Present(matrix.rows)
            });
        Ok(SocSourceFields {
            chunk_index,
            entity_ordinal,
            selection,
            class_selection,
            complete: self.complete,
            attributes,
            local_transform: OriginalDatum::observed(value, transform_uses, self.complete),
        })
    }

    fn authored_attribute(
        &self,
        attribute: &SourceAttribute<'_>,
        location: SourceLocation,
    ) -> Result<SocSourceAttribute, CatalogError> {
        let source = self.origin.payload_at(location, SourceUseRole::Payload)?;
        Ok(SocSourceAttribute {
            name: OriginalDatum::observed(
                SourceValue::Present(attribute.name().to_owned()),
                vec![source.clone()],
                self.complete,
            ),
            value: OriginalDatum::observed(
                SourceValue::Present(attribute.value().to_owned()),
                vec![source],
                self.complete,
            ),
        })
    }

    fn node_location(&self, chunk: u32) -> Result<SourceLocation, CatalogError> {
        Ok(match self.entity.element().position() {
            SourceElementPosition::CryXml(node) => soc_location(
                "socCryXml",
                vec![u64::from(chunk), u64::from(node)],
                "$node",
            ),
            SourceElementPosition::Text(range) => self.text_location(chunk, range, "$node")?,
            _ => return Err(ExportAccountingError::ConflictingSourceDatum.into()),
        })
    }

    fn attribute_location(
        &self,
        chunk: u32,
        name: &str,
        position: &SourceAttributePosition,
    ) -> Result<SourceLocation, CatalogError> {
        Ok(match position {
            SourceAttributePosition::CryXml {
                node_index,
                attribute_index,
            } => soc_location(
                "socCryXml",
                vec![
                    u64::from(chunk),
                    u64::from(*node_index),
                    u64::from(*attribute_index),
                ],
                name,
            ),
            SourceAttributePosition::Text(range) => {
                self.text_location(chunk, range.clone(), name)?
            }
            _ => return Err(ExportAccountingError::ConflictingSourceDatum.into()),
        })
    }

    fn text_location(
        &self,
        chunk: u32,
        range: std::ops::Range<usize>,
        field: &str,
    ) -> Result<SourceLocation, CatalogError> {
        let absolute = |offset| {
            u64::try_from(offset)
                .ok()
                .and_then(|offset| self.offset.checked_add(offset))
                .ok_or(ExportAccountingError::ConflictingSourceDatum)
        };
        Ok(soc_location(
            "socTextXml",
            vec![
                u64::from(chunk),
                absolute(range.start)?,
                absolute(range.end)?,
            ],
            field,
        ))
    }
}

fn ordinal(index: usize) -> Result<u32, ExportAccountingError> {
    u32::try_from(index).map_err(|_| ExportAccountingError::ConflictingSourceDatum)
}

fn soc_location(namespace: &str, indices: Vec<u64>, field: &str) -> SourceLocation {
    SourceLocation::Subresource {
        namespace: namespace.to_owned(),
        indices,
        field: field.to_owned(),
    }
}
