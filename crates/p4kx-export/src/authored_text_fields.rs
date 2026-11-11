// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceUseRole, SourceValue, SourcedValue, Value};

use super::{Fact, Observed};
use crate::CatalogError;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_read::original::OriginalValuePart;
use crate::catalog_text_contract::{GameTextRef, PublicNameRef};
use crate::source_fields::FieldError;

impl<'scope, 'read, 'source> Observed<'_, 'scope, 'read, 'source> {
    pub(in crate::catalog_v3) fn public_name(
        &self,
        name: &str,
        projection: &CatalogProjection<'_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<SourcedValue<PublicNameRef>, CatalogError> {
        self.locale(name, notes, |fact| projection.public_name_fact(fact))
    }

    pub(in crate::catalog_v3) fn protected_locale(
        &self,
        name: &str,
        projection: &CatalogProjection<'_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<SourcedValue<GameTextRef>, CatalogError> {
        self.locale(name, notes, |fact| projection.protected_locale_fact(fact))
    }

    fn locale<T>(
        &self,
        name: &str,
        notes: &mut Vec<FieldError>,
        retain: impl FnOnce(&Fact<'scope, 'read, 'source>) -> Result<T, CatalogError>,
    ) -> Result<SourcedValue<T>, CatalogError> {
        let Some((_, fact)) = self.originals.field(self.owner, name)? else {
            return self.absent();
        };
        let (state, role) = match fact.value() {
            Value::Locale(_) => (
                SourceValue::Present(retain(fact)?),
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
