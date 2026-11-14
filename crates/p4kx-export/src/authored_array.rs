// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataType, SourceUseRole, SourceValue, SourcedValue, Value};

use super::{Fact, Observed};
use crate::CatalogError;
use crate::catalog_read::original::OriginalValuePart;
use crate::resource_float::ResourceFloat32;
use crate::source_fields::FieldError;

impl<'reader, 'scope, 'read, 'source> Observed<'reader, 'scope, 'read, 'source> {
    pub(in crate::catalog_v3) fn single_array(
        &self,
        name: &str,
        notes: &mut Vec<FieldError>,
    ) -> Result<SourcedValue<Vec<SourcedValue<ResourceFloat32>>>, CatalogError> {
        let Some((_, fact)) = self.originals.field(self.owner, name)? else {
            return self.absent();
        };
        let storage = fact.field().storage();
        let expected = "a nullable Single array";
        let state = if storage.data_type() == DataType::Single && storage.is_array() {
            self.array_elements(fact, name, expected, notes, |index, element, notes| {
                self.single_element(name, index, element, notes)
            })?
        } else {
            self.failure(name, fact.value(), expected, notes)?
        };
        self.bind_storage(fact, state)
    }

    fn single_element(
        &self,
        name: &str,
        index: usize,
        fact: &Fact<'scope, 'read, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<SourcedValue<ResourceFloat32>, CatalogError> {
        let (state, role) = match self.original_single(fact)? {
            Some(value) => (SourceValue::Present(value), SourceUseRole::Payload),
            None => match fact.value() {
                Value::Null => (SourceValue::Null, SourceUseRole::Selector),
                other => (
                    self.failure(
                        &format!("{name}[{index}]"),
                        other,
                        "a nullable Single",
                        notes,
                    )?,
                    SourceUseRole::Selector,
                ),
            },
        };
        self.bind_field(fact, OriginalValuePart::Value, state, role)
    }

    pub(super) fn array_elements<T>(
        &self,
        fact: &'reader Fact<'scope, 'read, 'source>,
        name: &str,
        expected: &'static str,
        notes: &mut Vec<FieldError>,
        mut decode: impl FnMut(
            usize,
            &'reader Fact<'scope, 'read, 'source>,
            &mut Vec<FieldError>,
        ) -> Result<SourcedValue<T>, CatalogError>,
    ) -> Result<SourceValue<Vec<SourcedValue<T>>>, CatalogError> {
        Ok(match fact.value() {
            Value::Null => SourceValue::Null,
            Value::Array(values) => {
                let mut rows = Vec::with_capacity(values.len());
                for (index, original) in values.iter().enumerate() {
                    let element = self.originals.fact(original)?;
                    rows.push(decode(index, element, notes)?);
                }
                SourceValue::Present(rows)
            }
            value => self.failure(name, value, expected, notes)?,
        })
    }
}
