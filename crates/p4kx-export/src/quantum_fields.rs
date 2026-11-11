// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceValueState, SourcedValue};

use super::authored_fields::Observed;
use crate::CatalogError;
use crate::resource_number::ResourceNumber;
use crate::source_fields::FieldError;
use crate::source_value_catalog::SourceArenaOutput;

pub(super) struct QuantumFields<'reader, 'scope, 'read, 'source, 'notes> {
    source: Observed<'reader, 'scope, 'read, 'source>,
    notes: &'notes mut Vec<FieldError>,
    projected: Vec<(&'static str, SourceValueState)>,
}

impl<'reader, 'scope, 'read, 'source, 'notes>
    QuantumFields<'reader, 'scope, 'read, 'source, 'notes>
{
    pub(super) const fn new(
        source: Observed<'reader, 'scope, 'read, 'source>,
        notes: &'notes mut Vec<FieldError>,
    ) -> Self {
        Self {
            source,
            notes,
            projected: Vec::new(),
        }
    }

    pub(super) fn source_type(&self) -> Result<SourcedValue<&'source str>, CatalogError> {
        self.source.source_type_value()
    }

    pub(super) fn number(
        &mut self,
        name: &'static str,
    ) -> Result<SourcedValue<ResourceNumber>, CatalogError> {
        let cell = self.source.authored_number(name, self.notes)?;
        self.projected.push((name, cell.state()));
        Ok(cell)
    }

    pub(super) fn member(
        &mut self,
        name: &'static str,
    ) -> Result<SourcedValue<SourceArenaOutput<'source>>, CatalogError> {
        let cell = self.source.authored_member(name, self.notes)?;
        self.projected.push((name, cell.state()));
        Ok(cell)
    }

    pub(super) fn class<T>(
        &mut self,
        name: &'static str,
        decode: impl FnOnce(
            Observed<'reader, 'scope, 'read, 'source>,
            &mut Vec<FieldError>,
        ) -> Result<T, CatalogError>,
    ) -> Result<SourcedValue<T>, CatalogError> {
        let cell = self.source.declared_class(name, self.notes, decode)?;
        self.projected.push((name, cell.state()));
        Ok(cell)
    }

    pub(super) fn additional_fields(self) -> Result<SourceArenaOutput<'source>, CatalogError> {
        self.source.authored_members_except(self.projected)
    }
}
