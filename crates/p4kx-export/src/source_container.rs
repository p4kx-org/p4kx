// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceUseRole, SourceUses, SourceValue, SourcedValue, Value};

use crate::catalog_read::original::{
    OriginalContainerKind, OriginalDatum, OriginalValueEdge, OriginalValuePart, OriginalValueSite,
};
use crate::{CatalogError, ExportAccountingError};

use super::{SourceField, SourceNode, SourcePayload, SourceValueRef};

/// One unfinished container whose original contributions survive delayed child assembly.
pub(super) struct SourceContainer<'source> {
    pub(super) ordinal: u64,
    pub(super) index: usize,
    original: OriginalDatum<()>,
    contents: Contents<'source>,
}

enum Contents<'source> {
    Struct {
        source_type: SourcedValue<&'source str>,
        fields: Vec<SourceField<'source>>,
    },
    Array {
        elements: Vec<SourceValueRef>,
    },
}

impl<'source> SourceContainer<'source> {
    pub(super) fn read(
        site: &OriginalValueSite<'_, '_, '_, 'source>,
        index: usize,
        sources: &SourceUses,
    ) -> Result<Option<Self>, CatalogError> {
        let contents = match site.value() {
            Value::Struct(source) => Contents::Struct {
                source_type: site
                    .bind(
                        OriginalValuePart::ConcreteType,
                        SourceValue::Present(source.type_name()),
                        SourceUseRole::Selector,
                        true,
                    )?
                    .into_cell(sources),
                fields: Vec::new(),
            },
            Value::Array(_) => Contents::Array {
                elements: Vec::new(),
            },
            _ => return Ok(None),
        };
        Ok(Some(Self {
            ordinal: site.ordinal(),
            index,
            original: site.bind(
                OriginalValuePart::Value,
                SourceValue::Present(()),
                SourceUseRole::Payload,
                true,
            )?,
            contents,
        }))
    }

    pub(super) fn push(
        &mut self,
        edge: OriginalValueEdge,
        value: SourceValueRef,
        field: Option<SourceField<'source>>,
    ) -> Result<(), CatalogError> {
        match (&mut self.contents, edge, field) {
            (
                Contents::Struct { fields, .. },
                OriginalValueEdge::Field { ordinal },
                Some(field),
            ) if u64::try_from(fields.len()).ok() == Some(ordinal) => fields.push(field),
            (Contents::Array { elements }, OriginalValueEdge::Element { ordinal }, None)
                if u64::try_from(elements.len()).ok() == Some(ordinal) =>
            {
                elements.push(value);
            }
            _ => return Err(ExportAccountingError::ConflictingSourceDatum.into()),
        }
        Ok(())
    }

    pub(super) fn finish(
        self,
        kind: OriginalContainerKind,
        sources: &SourceUses,
    ) -> Result<SourceNode<'source>, CatalogError> {
        let payload = match (self.contents, kind) {
            (
                Contents::Struct {
                    source_type,
                    fields,
                },
                OriginalContainerKind::Struct,
            ) => SourcePayload::Struct {
                source_type,
                fields,
            },
            (Contents::Array { elements }, OriginalContainerKind::Array) => {
                SourcePayload::Array { elements }
            }
            _ => return Err(ExportAccountingError::ConflictingSourceDatum.into()),
        };
        Ok(SourceNode {
            value: self.original.map(|()| payload)?.into_cell(sources),
            original_error: None,
        })
    }
}
