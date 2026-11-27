// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;

use super::Observed;
use crate::CatalogError;
use crate::catalog_number_capture::CapturedNumber;
use crate::resource_number::ResourceNumber;
use crate::source_fields::FieldError;

impl Observed<'_, '_, '_, '_> {
    pub(in crate::catalog_v3) fn authored_number(
        &self,
        name: &str,
        notes: &mut Vec<FieldError>,
    ) -> Result<SourcedValue<ResourceNumber>, CatalogError> {
        self.scalar(name, "a nullable number", notes, |fact| {
            let value = ResourceNumber::read(fact.value());
            if let Some(number) =
                CapturedNumber::authored_raw(self.originals, fact, self.number_owner)?
            {
                self.numbers.borrow_mut().push(number);
            }
            Ok(value)
        })
    }

    pub(in crate::catalog_v3) fn authored_integer(
        &self,
        name: &str,
        notes: &mut Vec<FieldError>,
    ) -> Result<SourcedValue<ResourceNumber>, CatalogError> {
        self.scalar(name, "a nullable integer", notes, |fact| {
            Ok(ResourceNumber::read(fact.value()).filter(|number| {
                !matches!(
                    number,
                    ResourceNumber::Single(_) | ResourceNumber::Double(_)
                )
            }))
        })
    }
}
