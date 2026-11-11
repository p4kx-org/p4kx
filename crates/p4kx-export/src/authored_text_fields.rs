// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceUseRole, SourceValue, SourcedValue, Value};

use super::Observed;
use crate::CatalogError;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_read::original::OriginalValuePart;
use crate::catalog_text_contract::PublicNameRef;
use crate::source_fields::FieldError;

impl<'source> Observed<'_, '_, '_, 'source> {
    pub(in crate::catalog_v3) fn public_name(
        &self,
        name: &str,
        projection: &CatalogProjection<'_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<SourcedValue<PublicNameRef>, CatalogError> {
        let Some((_, fact)) = self.originals.field(self.owner, name)? else {
            return self.absent();
        };
        let (state, role) = match fact.value() {
            Value::Locale(_) => (
                SourceValue::Present(projection.public_name_fact(fact)?),
                SourceUseRole::Conversion,
            ),
            Value::Null => (SourceValue::Null, SourceUseRole::Selector),
            value => (
                self.failure(name, value, "a nullable Locale", notes)?,
                SourceUseRole::Selector,
            ),
        };
        self.bind_field(fact, OriginalValuePart::Value, state, role)
    }
}
