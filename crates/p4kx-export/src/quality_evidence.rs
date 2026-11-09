// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    SourceCoordinateOmission, SourceLocation, SourceUse, SourceUseRole, SourceValue, SourcedValue,
    Value,
};

use crate::CatalogError;
use crate::catalog_input_limits::{MAX_DECISION_INPUT_BYTES, MAX_DECISION_INPUTS};
use crate::catalog_origins::CatalogOrigins;
use crate::catalog_read::CatalogRecordContext;
use crate::record_header_field::RecordHeaderInput;

/// Bounded actual original decision inputs, independent of reduced output values.
#[derive(Debug, Clone)]
pub(crate) struct Evidence {
    uses: Vec<SourceUse>,
    cut: Option<SourceUse>,
    bytes: usize,
    pub(crate) complete: bool,
}

impl Default for Evidence {
    fn default() -> Self {
        Self {
            uses: Vec::new(),
            cut: None,
            bytes: 0,
            complete: true,
        }
    }
}

impl Evidence {
    pub(crate) fn header(
        &mut self,
        context: &CatalogRecordContext<'_>,
        field: RecordHeaderInput,
        readable: bool,
    ) -> Result<(), CatalogError> {
        let mut actual = Vec::new();
        context.append_header_input(
            field,
            if readable {
                SourceUseRole::Selector
            } else {
                SourceUseRole::Attempted
            },
            &mut actual,
        )?;
        for source in actual {
            self.keep(source);
        }
        self.complete &= readable;
        Ok(())
    }

    pub(crate) fn field(
        &mut self,
        context: &CatalogRecordContext<'_>,
        path: &str,
        value: Option<&Value<'_>>,
    ) -> Result<(), CatalogError> {
        let readable = !matches!(value, Some(Value::Error(_)));
        self.keep(context.observe(
            path,
            if readable {
                SourceUseRole::Selector
            } else {
                SourceUseRole::Attempted
            },
        )?);
        self.complete &= readable;
        Ok(())
    }

    pub(crate) fn keep(&mut self, source: SourceUse) {
        let bytes =
            coordinate_bytes(&source).and_then(|bytes| bytes.checked_add(size_of::<SourceUse>()));
        if self.cut.is_some()
            || self.uses.len() == MAX_DECISION_INPUTS
            || bytes.is_none_or(|bytes| bytes > MAX_DECISION_INPUT_BYTES.saturating_sub(self.bytes))
        {
            if self.cut.is_none() {
                self.cut = Some(source);
            }
            self.complete = false;
            return;
        }
        if let Some(bytes) = bytes {
            self.bytes += bytes;
        }
        self.uses.push(source);
    }

    pub(crate) fn append(&mut self, other: &Self) {
        self.complete &= other.complete;
        for source in &other.uses {
            self.keep(source.clone());
            if self.cut.is_some() {
                break;
            }
        }
        if self.cut.is_none() {
            self.cut.clone_from(&other.cut);
        }
        self.complete &= self.cut.is_none();
    }

    pub(crate) fn sources(&self, origins: &CatalogOrigins) -> Result<Vec<SourceUse>, CatalogError> {
        let mut uses = self.uses.clone();
        if let Some(actual) = &self.cut {
            uses.push(
                origins
                    .uses
                    .opaque_use(actual, SourceCoordinateOmission::CaptureLimit)?,
            );
        }
        Ok(uses)
    }

    pub(crate) fn value<T>(
        &self,
        origins: &CatalogOrigins,
        value: T,
    ) -> Result<SourcedValue<T>, CatalogError> {
        Ok(origins.uses.value(
            SourceValue::Present(value),
            &self.sources(origins)?,
            self.complete,
        ))
    }
}

fn coordinate_bytes(source: &SourceUse) -> Option<usize> {
    match source.description().location() {
        SourceLocation::DataForge { field, .. } | SourceLocation::Metadata { field } => {
            Some(field.len())
        }
        SourceLocation::Subresource {
            namespace,
            indices,
            field,
        } => namespace.len().checked_add(field.len()).and_then(|bytes| {
            indices
                .len()
                .checked_mul(size_of::<u64>())
                .and_then(|indices| bytes.checked_add(indices))
        }),
        _ => Some(0),
    }
}
