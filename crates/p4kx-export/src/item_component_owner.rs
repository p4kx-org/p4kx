// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{RecordTree, SourceUseRole, SourceValue, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use crate::catalog_read::CatalogRecordContext;
use crate::catalog_read::original::{
    OriginalFlowFields, OriginalValueEdge, OriginalValueFact, OriginalValuePart,
};
use crate::guid_text::CatalogGuid;
use crate::record_header_field::RecordHeaderField;
use crate::schema_fields::every_field_required;
use crate::{CatalogError, ExportAccountingError};

/// The shared physical Record and original Components slot of a configuration.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct ComponentOwner {
    owner_guid: SourcedValue<CatalogGuid>,
    owner_record_index: SourcedValue<u32>,
    component_ordinal: SourcedValue<u32>,
}

impl ComponentOwner {
    pub(super) fn read<'scope, 'read, 'source>(
        tree: &RecordTree<'source>,
        context: &CatalogRecordContext<'_>,
        original: &OriginalFlowFields<'scope, 'read, 'source>,
        component: &OriginalValueFact<'scope, 'read, 'source>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            owner_guid: context.header_value(
                RecordHeaderField::Guid,
                SourceValue::Present(CatalogGuid(tree.guid())),
            )?,
            owner_record_index: context.header_selector(
                RecordHeaderField::RecordIndex,
                SourceValue::Present(tree.record_index()),
            )?,
            component_ordinal: Self::ordinal(original, component)?,
        })
    }

    pub(super) fn ordinal<'scope, 'read, 'source>(
        original: &OriginalFlowFields<'scope, 'read, 'source>,
        component: &OriginalValueFact<'scope, 'read, 'source>,
    ) -> Result<SourcedValue<u32>, CatalogError> {
        original.span(component)?;
        let OriginalValueEdge::Element { ordinal } = component.edge() else {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        };
        let ordinal = u32::try_from(ordinal).map_err(|_| ExportAccountingError::CounterOverflow)?;
        Ok(component
            .bind(
                OriginalValuePart::Value,
                SourceValue::Present(ordinal),
                SourceUseRole::Conversion,
                true,
            )?
            .into_cell(original.sources()))
    }

    pub(super) const fn key(&self) -> (Option<u32>, Option<u32>) {
        (
            self.owner_record_index.value().copied(),
            self.component_ordinal.value().copied(),
        )
    }
}
