// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceValue, SourceValueState, SourcedValue};

use crate::CatalogError;
use crate::catalog_read::CatalogRecordContext;
use crate::commodity_authoring_inputs;
use crate::record_header_field::RecordHeaderField;

/// An explicitly typed original selector used to find a text candidate.
#[derive(Debug, Clone)]
pub(crate) enum CommodityTextSelector {
    Body(String),
    Header(RecordHeaderField),
}

/// A raw text key and the original selectors used to find its component.
#[derive(Debug, Clone)]
pub(crate) struct CommodityTextInput<'source> {
    pub(crate) key: SourcedValue<&'source str>,
    pub(crate) record_index: u32,
    pub(crate) field: String,
    pub(crate) selectors: Vec<CommodityTextSelector>,
    pub(crate) selectors_complete: bool,
}

impl<'source> CommodityTextInput<'source> {
    pub(crate) const fn new(
        key: SourcedValue<&'source str>,
        record_index: u32,
        field: String,
    ) -> Self {
        Self {
            key,
            record_index,
            field,
            selectors: Vec::new(),
            selectors_complete: true,
        }
    }

    pub(crate) fn with_selectors(
        mut self,
        selectors: &[CommodityTextSelector],
        complete: bool,
    ) -> Self {
        self.selectors = selectors.to_vec();
        self.selectors_complete = complete;
        self
    }

    pub(crate) fn parent(
        context: &CatalogRecordContext<'_>,
        record_index: u32,
        field: &str,
        state: SourceValueState,
        selectors: Vec<String>,
        complete: bool,
    ) -> Result<Self, CatalogError> {
        let value =
            commodity_authoring_inputs::without_payload(state).unwrap_or(SourceValue::Absent);
        Ok(Self {
            key: context.value(field, value)?,
            record_index,
            field: field.to_owned(),
            selectors: selectors
                .into_iter()
                .map(CommodityTextSelector::Body)
                .collect(),
            selectors_complete: complete,
        })
    }
}
