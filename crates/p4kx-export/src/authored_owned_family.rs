// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataType, SourceUseRole, SourceValue, SourcedValue, Value};

use super::{Fact, Observed};
use crate::CatalogError;
use crate::catalog_read::original::{OriginalDatum, OriginalValuePart};
use crate::source_fields::FieldError;

#[derive(Clone, Copy)]
enum PointerShape {
    Scalar,
    Array,
}

impl PointerShape {
    const fn expectation(self) -> &'static str {
        match self {
            Self::Scalar => "a nullable scalar StrongPointer",
            Self::Array => "a nullable StrongPointer array",
        }
    }
}

impl<'reader, 'scope, 'read, 'source> Observed<'reader, 'scope, 'read, 'source> {
    /// Reads a scalar `StrongPointer` whose concrete value belongs to the declared family.
    pub(in crate::catalog_v3) fn strong_pointer_family_to<T>(
        &self,
        name: &str,
        target: &str,
        notes: &mut Vec<FieldError>,
        decode: impl FnOnce(Self, &mut Vec<FieldError>) -> Result<T, CatalogError>,
    ) -> Result<SourcedValue<T>, CatalogError> {
        self.family_field(name, target, PointerShape::Scalar, notes, |fact, notes| {
            self.family_value(name, fact, notes, decode)
        })
    }

    /// Reads ordered `StrongPointer` slots with physical declaration and ancestry checks.
    pub(in crate::catalog_v3) fn strong_pointer_family_array_to<T>(
        &self,
        name: &str,
        target: &str,
        notes: &mut Vec<FieldError>,
        mut decode: impl FnMut(Self, &mut Vec<FieldError>) -> Result<T, CatalogError>,
    ) -> Result<SourcedValue<Vec<SourcedValue<T>>>, CatalogError> {
        self.family_field(name, target, PointerShape::Array, notes, |fact, notes| {
            let state = self.array_elements(
                fact,
                name,
                PointerShape::Array.expectation(),
                notes,
                |index, element, notes| {
                    let path = format!("{name}[{index}]");
                    let mut cell = self.family_value(&path, element, notes, &mut decode)?;
                    cell.contribute(self.kind()?);
                    Ok(cell.into_cell(self.originals.sources()))
                },
            )?;
            fact.bind(
                OriginalValuePart::Value,
                state,
                SourceUseRole::Selector,
                true,
            )
        })
    }

    fn family_field<T>(
        &self,
        name: &str,
        target: &str,
        shape: PointerShape,
        notes: &mut Vec<FieldError>,
        read: impl FnOnce(
            &'reader Fact<'scope, 'read, 'source>,
            &mut Vec<FieldError>,
        ) -> Result<OriginalDatum<T>, CatalogError>,
    ) -> Result<SourcedValue<T>, CatalogError> {
        let Some((_, fact)) = self.originals.field(self.owner, name)? else {
            return self.absent();
        };
        let storage = fact.field().storage();
        if storage.data_type() != DataType::StrongPointer
            || storage.is_array() != matches!(shape, PointerShape::Array)
        {
            let state = self.failure(name, fact.value(), shape.expectation(), notes)?;
            return self.bind_storage(fact, state);
        }
        let declared = fact.observe_declared_target(self.input)?;
        self.read_declared_value(fact, &[target], declared, notes, |notes| read(fact, notes))
    }

    fn family_value<T>(
        &self,
        name: &str,
        fact: &'reader Fact<'scope, 'read, 'source>,
        notes: &mut Vec<FieldError>,
        decode: impl FnOnce(Self, &mut Vec<FieldError>) -> Result<T, CatalogError>,
    ) -> Result<OriginalDatum<T>, CatalogError> {
        self.originals.check_cancel()?;
        let mut selectors = Vec::new();
        let (part, state) = match fact.value() {
            Value::Struct(source) => {
                let admitted =
                    fact.matches_owned_declaration(self.input, &mut selectors, || {
                        self.originals.check_cancel()
                    })?;
                let state = if admitted {
                    SourceValue::Present(decode(
                        Self {
                            owner: Some(fact),
                            ..*self
                        },
                        notes,
                    )?)
                } else {
                    self.unsupported(fact, source, notes)?
                };
                (OriginalValuePart::ConcreteType, state)
            }
            Value::Null => (OriginalValuePart::Value, SourceValue::Null),
            value => (
                OriginalValuePart::Value,
                self.failure(name, value, "a nullable Struct", notes)?,
            ),
        };
        let mut cell = fact.bind(part, state, SourceUseRole::Selector, true)?;
        for selector in selectors {
            cell.contribute(selector);
        }
        Ok(cell)
    }
}
