// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceUse, SourceUses, SourceValue, SourcedValue};

use crate::ExportAccountingError;

/// One retained value and the live contributions captured by its original operation.
#[derive(Debug, Clone)]
pub(crate) struct OriginalDatum<T> {
    value: SourceValue<T>,
    inputs: Vec<SourceUse>,
    complete: bool,
}

impl<T> OriginalDatum<T> {
    pub(super) const fn observed(
        value: SourceValue<T>,
        inputs: Vec<SourceUse>,
        complete: bool,
    ) -> Self {
        Self {
            value,
            inputs,
            complete,
        }
    }

    pub(crate) const fn value(&self) -> &SourceValue<T> {
        &self.value
    }

    pub(crate) fn contributions(&self) -> (&[SourceUse], bool) {
        (&self.inputs, self.complete)
    }

    pub(crate) fn contribute(&mut self, input: SourceUse) {
        self.inputs.push(input);
    }

    pub(crate) fn into_cell(self, sources: &SourceUses) -> SourcedValue<T> {
        sources.value(self.value, &self.inputs, self.complete)
    }

    /// Changes the retained representation without rehydrating its original contributions.
    pub(crate) fn map<U>(
        self,
        map: impl FnOnce(T) -> U,
    ) -> Result<OriginalDatum<U>, ExportAccountingError> {
        self.try_map_observed(|value, _, _| Ok(map(value)))
    }

    pub(crate) fn try_map<U, E>(
        self,
        map: impl FnOnce(T) -> Result<U, E>,
    ) -> Result<OriginalDatum<U>, E>
    where
        E: From<ExportAccountingError>,
    {
        self.try_map_observed(|value, _, _| map(value))
    }

    pub(super) fn try_map_observed<U, E>(
        self,
        map: impl FnOnce(T, &[SourceUse], bool) -> Result<U, E>,
    ) -> Result<OriginalDatum<U>, E>
    where
        E: From<ExportAccountingError>,
    {
        let value = match self.value {
            SourceValue::Present(value) => {
                SourceValue::Present(map(value, &self.inputs, self.complete)?)
            }
            SourceValue::Absent => SourceValue::Absent,
            SourceValue::Null => SourceValue::Null,
            SourceValue::Unreadable => SourceValue::Unreadable,
            SourceValue::Unsupported => SourceValue::Unsupported,
            SourceValue::Invalid => SourceValue::Invalid,
            _ => return Err(ExportAccountingError::ConflictingSourceDatum.into()),
        };
        Ok(OriginalDatum::observed(value, self.inputs, self.complete))
    }
}
