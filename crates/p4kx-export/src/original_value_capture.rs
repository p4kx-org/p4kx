// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceUse, SourceUseRole, SourceValue};

use super::{OriginalDatum, OriginalValueFact, OriginalValuePart, OriginalValueSite};
use crate::{CatalogError, ExportAccountingError};

impl OriginalValueFact<'_, '_, '_> {
    /// Binds a retained fact to the original input used at this exact visit.
    pub(crate) fn bind<T>(
        &self,
        part: OriginalValuePart,
        value: SourceValue<T>,
        role: SourceUseRole,
        complete: bool,
    ) -> Result<OriginalDatum<T>, CatalogError> {
        let readable = readable(&value)?;
        let role = if readable {
            role
        } else {
            SourceUseRole::Attempted
        };
        let input = self.observe(part, role)?;
        Ok(OriginalDatum::observed(
            value,
            vec![input],
            complete && readable,
        ))
    }

    pub(crate) fn observe(
        &self,
        part: OriginalValuePart,
        role: SourceUseRole,
    ) -> Result<SourceUse, CatalogError> {
        Ok(self.scope().dataforge().at(self.location(part), role)?)
    }
}

impl OriginalValueSite<'_, '_, '_, '_> {
    /// Binds a value through the fact minted from this exact original visit.
    pub(crate) fn bind<T>(
        &self,
        part: OriginalValuePart,
        value: SourceValue<T>,
        role: SourceUseRole,
        complete: bool,
    ) -> Result<OriginalDatum<T>, CatalogError> {
        self.fact().bind(part, value, role, complete)
    }
}

const fn readable<T>(value: &SourceValue<T>) -> Result<bool, ExportAccountingError> {
    match value {
        SourceValue::Present(_)
        | SourceValue::Absent
        | SourceValue::Null
        | SourceValue::Unsupported
        | SourceValue::Invalid => Ok(true),
        SourceValue::Unreadable => Ok(false),
        _ => Err(ExportAccountingError::ConflictingSourceDatum),
    }
}
