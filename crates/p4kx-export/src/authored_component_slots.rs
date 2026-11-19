// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceUseRole, SourceUses, SourceValue, SourcedValue, Value};

use super::Observed;
use crate::catalog_read::original::OriginalValuePart;
use crate::source_fields::FieldError;
use crate::{CatalogError, ExportAccountingError};

impl<'source> Observed<'_, '_, '_, 'source> {
    pub(in crate::catalog_v3) const fn sources(&self) -> &SourceUses {
        self.originals.sources()
    }

    /// Visits the readable component structs in physical slot order.
    pub(in crate::catalog_v3) fn visit_components(
        &self,
        notes: &mut Vec<FieldError>,
        mut visit: impl FnMut(&'source str, Self, &mut Vec<FieldError>) -> Result<(), CatalogError>,
    ) -> Result<SourcedValue<u32>, CatalogError> {
        let name = "Components";
        let Some((_, array)) = self.originals.field(self.owner, name)? else {
            return self.absent();
        };
        let state = match array.value() {
            Value::Array(values) => {
                for (index, original) in values.iter().enumerate() {
                    self.check_cancel()?;
                    match original {
                        Value::Struct(component) => {
                            let fact = self.originals.fact(original)?;
                            visit(component.type_name(), self.at_owner(fact), notes)?;
                        }
                        Value::Null => {}
                        other => {
                            self.failure::<()>(
                                &format!("{name}[{index}]"),
                                other,
                                "a nullable component structure",
                                notes,
                            )?;
                        }
                    }
                }
                let count = u32::try_from(values.len())
                    .map_err(|_| ExportAccountingError::ConflictingSourceDatum)?;
                SourceValue::Present(count)
            }
            Value::Null => SourceValue::Null,
            other => self.failure(name, other, "a nullable component array", notes)?,
        };
        self.bind_field(
            array,
            OriginalValuePart::Value,
            state,
            SourceUseRole::Selector,
        )
    }
}
