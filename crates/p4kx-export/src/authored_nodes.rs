// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::VecDeque;

use super::authored_fields::Observed;
use crate::source_fields::FieldError;
use crate::{CatalogError, ExportAccountingError};

/// Assigns local indices in discovery order for original owned-node occurrences.
#[derive(Default)]
pub(super) struct AuthoredNodes<'reader, 'scope, 'read, 'source> {
    pending: VecDeque<Observed<'reader, 'scope, 'read, 'source>>,
    next: u32,
}

impl<'reader, 'scope, 'read, 'source> AuthoredNodes<'reader, 'scope, 'read, 'source> {
    pub(super) fn push(
        &mut self,
        source: Observed<'reader, 'scope, 'read, 'source>,
    ) -> Result<u32, CatalogError> {
        let index = self.next;
        self.next = index
            .checked_add(1)
            .ok_or(ExportAccountingError::CounterOverflow)?;
        self.pending.push_back(source);
        Ok(index)
    }

    pub(super) fn read<T>(
        mut self,
        notes: &mut Vec<FieldError>,
        mut decode: impl FnMut(
            Observed<'reader, 'scope, 'read, 'source>,
            &mut Self,
            &mut Vec<FieldError>,
        ) -> Result<T, CatalogError>,
    ) -> Result<Vec<T>, CatalogError> {
        let mut values = Vec::new();
        while let Some(source) = self.pending.pop_front() {
            source.check_cancel()?;
            values.push(decode(source, &mut self, notes)?);
        }
        Ok(values)
    }
}
