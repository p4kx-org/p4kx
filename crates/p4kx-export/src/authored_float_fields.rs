// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourcedValue, Value};

use super::{Fact, Observed};
use crate::CatalogError;
use crate::catalog_number_capture::CapturedNumber;
use crate::resource_float::{ResourceFloat32, ResourceFloat64};
use crate::source_fields::FieldError;

impl<'scope, 'read, 'source> Observed<'_, 'scope, 'read, 'source> {
    pub(in crate::catalog_v3) fn single(
        &self,
        name: &str,
        notes: &mut Vec<FieldError>,
    ) -> Result<SourcedValue<ResourceFloat32>, CatalogError> {
        self.single_observed(name, notes, |_| {})
    }

    pub(super) fn single_observed(
        &self,
        name: &str,
        notes: &mut Vec<FieldError>,
        observed: impl FnOnce(&Value<'source>),
    ) -> Result<SourcedValue<ResourceFloat32>, CatalogError> {
        self.scalar(name, "a nullable Single", notes, |fact| {
            let value = fact.value();
            observed(value);
            self.original_single(fact)
        })
    }

    pub(super) fn original_single(
        &self,
        fact: &Fact<'scope, 'read, 'source>,
    ) -> Result<Option<ResourceFloat32>, CatalogError> {
        let Value::Single(number) = fact.value() else {
            return Ok(None);
        };
        if let Some(number) =
            CapturedNumber::authored_single(self.originals, fact, self.number_owner)?
        {
            self.numbers.borrow_mut().push(number);
        }
        Ok(Some(ResourceFloat32::new(*number)))
    }

    pub(in crate::catalog_v3) fn double(
        &self,
        name: &str,
        notes: &mut Vec<FieldError>,
    ) -> Result<SourcedValue<ResourceFloat64>, CatalogError> {
        self.scalar(name, "a nullable Double", notes, |fact| {
            let Value::Double(value) = fact.value() else {
                return Ok(None);
            };
            if let Some(number) =
                CapturedNumber::authored_double(self.originals, fact, self.number_owner)?
            {
                self.numbers.borrow_mut().push(number);
            }
            Ok(Some(ResourceFloat64::new(*value)))
        })
    }
}
