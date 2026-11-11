// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataType, SourceUseRole, SourceValue, SourcedValue, Value};

use super::{Fact, Observed};
use crate::CatalogError;
use crate::catalog_number_capture::CapturedNumber;
use crate::catalog_read::original::OriginalValuePart;
use crate::source_fields::{FieldError, FieldViolation};
use crate::source_value_catalog::{self, SourceArenaOutput};

impl<'scope, 'read, 'source> Observed<'_, 'scope, 'read, 'source> {
    /// Retains the actual contents of a scalar `StrongPointer` to the declared base.
    pub(in crate::catalog_v3) fn raw_strong_pointer_to(
        &self,
        name: &str,
        declared_base: &str,
        notes: &mut Vec<FieldError>,
    ) -> Result<SourcedValue<SourceArenaOutput<'source>>, CatalogError> {
        let Some((_, fact)) = self.originals.field(self.owner, name)? else {
            return self.absent();
        };
        let storage = fact.field().storage();
        if storage.data_type() != DataType::StrongPointer || storage.is_array() {
            let state =
                self.failure(name, fact.value(), "a nullable scalar StrongPointer", notes)?;
            return self.bind_storage(fact, state);
        }
        self.read_declared_owned(fact, &[declared_base], notes, |notes| {
            Ok(match fact.value() {
                Value::Struct(_) => (
                    OriginalValuePart::ConcreteType,
                    SourceValue::Present(source_value_catalog::read_subtree(
                        self.originals,
                        fact,
                        |original| {
                            if let Some(number) =
                                CapturedNumber::item_component_raw(self.originals, original)?
                            {
                                self.numbers.borrow_mut().push(number);
                            }
                            Ok(())
                        },
                    )?),
                ),
                Value::Null => (OriginalValuePart::Value, SourceValue::Null),
                other => (
                    OriginalValuePart::Value,
                    self.failure(name, other, "an owned Struct", notes)?,
                ),
            })
        })
    }

    pub(super) fn read_declared_owned<T>(
        &self,
        fact: &Fact<'scope, 'read, 'source>,
        expected: &[&str],
        notes: &mut Vec<FieldError>,
        read: impl FnOnce(
            &mut Vec<FieldError>,
        ) -> Result<(OriginalValuePart, SourceValue<T>), CatalogError>,
    ) -> Result<SourcedValue<T>, CatalogError> {
        fact.check_input(self.input)?;
        let (declared, declaration) = self
            .input
            .declared_struct_name(fact.field().storage().target(), SourceUseRole::Selector)?;
        let (part, state) = match declared {
            Some(name) if expected.contains(&name) => read(notes)?,
            other => {
                self.unsupported_declaration(fact, notes)?;
                let state = if other.is_some() {
                    SourceValue::Unsupported
                } else {
                    SourceValue::Unreadable
                };
                (OriginalValuePart::Value, state)
            }
        };
        let inputs = [
            self.kind()?,
            fact.observe(part, SourceUseRole::Selector)?,
            fact.observe(OriginalValuePart::FieldName, SourceUseRole::Selector)?,
            fact.observe(OriginalValuePart::DataType, SourceUseRole::Selector)?,
            fact.observe(OriginalValuePart::IsArray, SourceUseRole::Selector)?,
            fact.observe(OriginalValuePart::Target, SourceUseRole::Selector)?,
            declaration,
        ];
        Ok(self
            .originals
            .sources()
            .value(state, &inputs, declared.is_some()))
    }

    fn unsupported_declaration(
        &self,
        fact: &Fact<'scope, 'read, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<(), CatalogError> {
        let path = self.owner.map_or_else(
            || Ok(String::new()),
            |owner| self.originals.logical_path(owner),
        )?;
        let violation = FieldViolation::unsupported(self.structure()?, &path, fact.field());
        let body = self.capture.field_failure(&violation)?;
        notes.push(violation.into_error().with_body(body));
        Ok(())
    }
}
