// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceUseRole, SourceValue, SourcedValue, Value};

use super::{Observed, OriginalValuePart};
use crate::CatalogError;

impl<'source> Observed<'_, '_, '_, 'source> {
    pub(in crate::catalog_v3) fn predicate(
        &self,
        name: &str,
        read: impl FnOnce(&Value<'source>) -> bool,
    ) -> Result<SourcedValue<bool>, CatalogError> {
        let Some((_, fact)) = self.originals.field(self.owner, name)? else {
            return self.absent();
        };
        let state = match fact.value() {
            Value::Error(_) => SourceValue::Unreadable,
            value => SourceValue::Present(read(value)),
        };
        self.bind_field(
            fact,
            OriginalValuePart::Value,
            state,
            SourceUseRole::Selector,
        )
    }
}
