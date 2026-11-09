// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    SourceLocation, SourceUse, SourceUseRole, SourceValue, SourceValueState, SourcedValue,
};

use crate::CatalogError;
use crate::catalog_origins::CatalogOrigins;
use crate::record_header_field::{RecordHeaderField, RecordHeaderInput};

/// Actual inputs used by one Commodity choice or conversion.
#[must_use]
pub(crate) struct CommodityInputs<'context> {
    origins: &'context CatalogOrigins,
    uses: Vec<SourceUse>,
    complete: bool,
}

impl<'context> CommodityInputs<'context> {
    pub(crate) const fn new(origins: &'context CatalogOrigins) -> Self {
        Self {
            origins,
            uses: Vec::new(),
            complete: true,
        }
    }

    pub(crate) fn at(
        &mut self,
        record_index: u32,
        field: &str,
        role: SourceUseRole,
    ) -> Result<(), CatalogError> {
        self.uses.push(self.origins.dataforge.at(
            SourceLocation::DataForge {
                record_index,
                field: field.to_owned(),
            },
            role,
        )?);
        Ok(())
    }

    pub(crate) fn header(
        &mut self,
        record_index: u32,
        field: RecordHeaderField,
        role: SourceUseRole,
    ) -> Result<(), CatalogError> {
        self.uses.push(
            self.origins
                .record(record_index)
                .observe_header(field, role)?,
        );
        Ok(())
    }

    pub(crate) fn header_input(
        &mut self,
        record_index: u32,
        input: RecordHeaderInput,
        role: SourceUseRole,
    ) -> Result<(), CatalogError> {
        self.origins
            .record(record_index)
            .append_header_input(input, role, &mut self.uses)
    }

    pub(crate) fn field<T>(
        &mut self,
        record_index: u32,
        field: &str,
        value: &SourcedValue<T>,
        role: SourceUseRole,
    ) -> Result<(), CatalogError> {
        let role = if value.state() == SourceValueState::Unreadable {
            SourceUseRole::Attempted
        } else {
            role
        };
        self.at(record_index, field, role)?;
        self.complete &= value.origin().source_set_complete();
        Ok(())
    }
    pub(crate) fn extend(&mut self, uses: impl IntoIterator<Item = SourceUse>, complete: bool) {
        self.uses.extend(uses);
        self.complete &= complete;
    }

    pub(crate) const fn require_complete(&mut self, complete: bool) {
        self.complete &= complete;
    }

    pub(crate) fn value<T>(&self, value: T) -> SourcedValue<T> {
        self.origins
            .uses
            .value(SourceValue::Present(value), &self.uses, self.complete)
    }

    #[must_use]
    pub(crate) fn into_evidence(self) -> (Vec<SourceUse>, bool) {
        (self.uses, self.complete)
    }
}
