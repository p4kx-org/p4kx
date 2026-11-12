// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    DataType, SourceUse, SourceUseRole, SourceUses, SourceValue, SourcedValue, Value,
};

use super::{Fact, Observed};
use crate::catalog_read::original::OriginalValuePart;
use crate::catalog_v3::authored_link::{self as link, HealthLink};
use crate::definition_reference::DefinitionStatus;
use crate::guid_text::CatalogGuid;
use crate::source_fields::{FieldError, FieldViolation};
use crate::source_value_catalog::{
    SourcePayload, SourceRecordReference, SourceWeakReference, read_reference_payload,
};
use crate::{CatalogError, ExportAccountingError};

pub(in crate::catalog_v3) type ReferenceCell<'source> =
    SourcedValue<Box<SourceRecordReference<'source>>>;

#[derive(Clone, Copy, PartialEq, Eq)]
enum ReferenceCardinality {
    Scalar,
    Array,
}

impl<'scope, 'read, 'source> Observed<'_, 'scope, 'read, 'source> {
    pub(in crate::catalog_v3) fn guid(
        &self,
        name: &str,
        notes: &mut Vec<FieldError>,
    ) -> Result<SourcedValue<CatalogGuid>, CatalogError> {
        self.scalar(name, "a nullable Guid", notes, |fact| {
            Ok(match fact.value() {
                Value::Guid(value) => Some(CatalogGuid(*value)),
                _ => None,
            })
        })
    }

    pub(in crate::catalog_v3) fn selector_use(
        &self,
        name: &str,
    ) -> Result<Option<SourceUse>, CatalogError> {
        self.originals
            .field(self.owner, name)?
            .map(|(_, fact)| fact.observe(OriginalValuePart::Value, SourceUseRole::Selector))
            .transpose()
    }

    pub(in crate::catalog_v3) fn link(
        &self,
        name: &str,
        notes: &mut Vec<FieldError>,
    ) -> Result<SourcedValue<HealthLink<'source>>, CatalogError> {
        self.read_link(
            name,
            "a nullable Reference or weak pointer",
            None,
            notes,
            |fact, sources, selectors| {
                if fact.field().storage().data_type() == DataType::Reference {
                    return Ok(self
                        .decode_record_link(fact, sources, selectors)?
                        .map(|(part, value)| (part, HealthLink::RecordReference { value })));
                }
                link::read(fact, sources)
            },
        )
    }

    pub(in crate::catalog_v3) fn record_link(
        &self,
        name: &str,
        notes: &mut Vec<FieldError>,
    ) -> Result<SourcedValue<Box<SourceRecordReference<'source>>>, CatalogError> {
        self.read_link(
            name,
            "a nullable Reference",
            None,
            notes,
            |fact, sources, selectors| self.decode_record_link(fact, sources, selectors),
        )
    }

    /// Reads a scalar Reference whose original declaration names the expected family.
    pub(in crate::catalog_v3) fn record_link_to(
        &self,
        name: &str,
        target_type: &'static str,
        notes: &mut Vec<FieldError>,
    ) -> Result<SourcedValue<Box<SourceRecordReference<'source>>>, CatalogError> {
        self.read_link(
            name,
            "a nullable Reference",
            Some((DataType::Reference, target_type)),
            notes,
            |fact, sources, selectors| self.decode_record_link(fact, sources, selectors),
        )
    }

    /// Reads a scalar weak pointer to its exact declared family.
    pub(in crate::catalog_v3) fn weak_link_to(
        &self,
        name: &str,
        target_type: &'static str,
        notes: &mut Vec<FieldError>,
    ) -> Result<SourcedValue<Box<SourceWeakReference<'source>>>, CatalogError> {
        self.read_link(
            name,
            "a nullable WeakPointer",
            Some((DataType::WeakPointer, target_type)),
            notes,
            |fact, sources, _| {
                Ok(
                    link::read(fact, sources)?.and_then(|(part, link)| match link {
                        HealthLink::WeakReference { value } => Some((part, value)),
                        HealthLink::RecordReference { .. } => None,
                    }),
                )
            },
        )
    }

    /// Reads an authored Reference or a weak pointer whose admitted target is a Record.
    pub(in crate::catalog_v3) fn record_or_weak_record_link(
        &self,
        name: &str,
        notes: &mut Vec<FieldError>,
    ) -> Result<ReferenceCell<'source>, CatalogError> {
        self.read_link(
            name,
            "a nullable Reference or weak Record pointer",
            None,
            notes,
            |fact, sources, selectors| {
                if fact.field().storage().data_type() != DataType::WeakPointer {
                    return self.decode_record_link(fact, sources, selectors);
                }
                let Some((part, HealthLink::WeakReference { value })) = link::read(fact, sources)?
                else {
                    return Ok(None);
                };
                let Some(record) = value.into_record() else {
                    return Ok(None);
                };
                selectors.push(fact.observe(OriginalValuePart::DataType, SourceUseRole::Selector)?);
                Ok(Some((part, Box::new(record))))
            },
        )
    }

    pub(in crate::catalog_v3) fn record_link_array_to(
        &self,
        name: &str,
        target_type: &str,
        notes: &mut Vec<FieldError>,
    ) -> Result<SourcedValue<Vec<ReferenceCell<'source>>>, CatalogError> {
        let Some((_, fact)) = self.originals.field(self.owner, name)? else {
            return self.absent();
        };
        let mut selectors = Vec::new();
        if let Some(rejected) = self.reject_link_declaration(
            fact,
            (DataType::Reference, target_type),
            ReferenceCardinality::Array,
            notes,
            &mut selectors,
        )? {
            return Ok(rejected);
        }
        let state = self.array_elements(
            fact,
            name,
            "a nullable Reference array",
            notes,
            |index, element, notes| self.record_link_element(name, index, element, notes),
        )?;
        self.bind_link(fact, OriginalValuePart::Value, state, selectors)
    }

    fn record_link_element(
        &self,
        name: &str,
        index: usize,
        fact: &Fact<'scope, 'read, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<ReferenceCell<'source>, CatalogError> {
        let mut selectors = Vec::new();
        let decoded = self.decode_record_link(fact, self.originals.sources(), &mut selectors)?;
        let (part, state) = self.link_state(
            &format!("{name}[{index}]"),
            fact,
            decoded,
            "a nullable Reference",
            notes,
        )?;
        self.bind_link(fact, part, state, selectors)
    }

    fn reject_link_declaration<T>(
        &self,
        fact: &Fact<'scope, 'read, 'source>,
        expected: (DataType, &str),
        cardinality: ReferenceCardinality,
        notes: &mut Vec<FieldError>,
        selectors: &mut Vec<SourceUse>,
    ) -> Result<Option<SourcedValue<T>>, CatalogError> {
        let storage = fact.field().storage();
        for part in [OriginalValuePart::DataType, OriginalValuePart::IsArray] {
            selectors.push(fact.observe(part, SourceUseRole::Selector)?);
        }
        let mut declaration_error = None;
        if storage.data_type() == expected.0
            && storage.is_array() == (cardinality == ReferenceCardinality::Array)
        {
            selectors.push(fact.observe(OriginalValuePart::Target, SourceUseRole::Selector)?);
            if storage.target() != u16::MAX {
                fact.check_input(self.input)?;
                let (declared_type, declaration) = self
                    .input
                    .declared_struct_name(storage.target(), SourceUseRole::Selector)?;
                selectors.push(declaration);
                if declared_type == Ok(expected.1) {
                    return Ok(None);
                }
                declaration_error = declared_type.err();
            }
        }

        let path = self.owner.map_or_else(
            || Ok(String::new()),
            |owner| self.originals.logical_path(owner),
        )?;
        let violation = FieldViolation::rejected_declaration(
            self.structure()?,
            &path,
            fact.field(),
            declaration_error,
        );
        let body = self.capture.field_failure(&violation)?;
        notes.push(violation.into_error().with_body(body));

        selectors.push(self.kind()?);
        selectors.push(fact.observe(OriginalValuePart::FieldName, SourceUseRole::Selector)?);
        let complete = declaration_error.is_none();
        let state = if complete {
            SourceValue::Unsupported
        } else {
            SourceValue::Unreadable
        };
        Ok(Some(
            self.originals.sources().value(state, selectors, complete),
        ))
    }

    fn decode_record_link(
        &self,
        fact: &Fact<'scope, 'read, 'source>,
        sources: &SourceUses,
        selectors: &mut Vec<SourceUse>,
    ) -> Result<Option<(OriginalValuePart, Box<SourceRecordReference<'source>>)>, CatalogError>
    {
        if fact.field().storage().data_type() != DataType::Reference {
            return Ok(None);
        }
        if matches!(fact.value(), Value::Reference(_)) {
            let observed = fact.reference_observation()?;
            let compatible =
                observed.matches_record_declaration(selectors, || self.originals.check_cancel())?;
            let SourcePayload::RecordReference { mut value } =
                read_reference_payload(observed, sources)?
            else {
                return Err(ExportAccountingError::ConflictingSourceDatum.into());
            };
            if !compatible {
                value.status = DefinitionStatus::UnexpectedTargetType;
            }
            return Ok(Some((OriginalValuePart::Value, Box::new(value))));
        }
        Ok(
            link::read(fact, sources)?.and_then(|(part, link)| match link {
                HealthLink::RecordReference { value } => Some((part, value)),
                HealthLink::WeakReference { .. } => None,
            }),
        )
    }

    fn read_link<T>(
        &self,
        name: &str,
        expected: &'static str,
        declaration: Option<(DataType, &str)>,
        notes: &mut Vec<FieldError>,
        decode: impl FnOnce(
            &Fact<'scope, 'read, 'source>,
            &SourceUses,
            &mut Vec<SourceUse>,
        ) -> Result<Option<(OriginalValuePart, T)>, CatalogError>,
    ) -> Result<SourcedValue<T>, CatalogError> {
        let Some((_, fact)) = self.originals.field(self.owner, name)? else {
            return self.absent();
        };
        let mut selectors = Vec::new();
        if let Some(declaration) = declaration
            && let Some(rejected) = self.reject_link_declaration(
                fact,
                declaration,
                ReferenceCardinality::Scalar,
                notes,
                &mut selectors,
            )?
        {
            return Ok(rejected);
        }
        let decoded = decode(fact, self.originals.sources(), &mut selectors)?;
        let (part, state) = self.link_state(name, fact, decoded, expected, notes)?;
        self.bind_link(fact, part, state, selectors)
    }

    fn link_state<T>(
        &self,
        name: &str,
        fact: &Fact<'scope, 'read, 'source>,
        decoded: Option<(OriginalValuePart, T)>,
        expected: &'static str,
        notes: &mut Vec<FieldError>,
    ) -> Result<(OriginalValuePart, SourceValue<T>), CatalogError> {
        Ok(match decoded {
            Some((part, target)) => (part, SourceValue::Present(target)),
            None => match fact.value() {
                Value::Null => (OriginalValuePart::Value, SourceValue::Null),
                Value::Struct(source) => (
                    OriginalValuePart::ConcreteType,
                    self.unsupported(fact, source, notes)?,
                ),
                other => (
                    OriginalValuePart::Value,
                    self.failure(name, other, expected, notes)?,
                ),
            },
        })
    }

    fn bind_link<T>(
        &self,
        fact: &Fact<'scope, 'read, 'source>,
        part: OriginalValuePart,
        state: SourceValue<T>,
        selectors: Vec<SourceUse>,
    ) -> Result<SourcedValue<T>, CatalogError> {
        let mut cell = self.field_datum(fact, part, state, SourceUseRole::Selector)?;
        for selector in selectors {
            cell.contribute(selector);
        }
        Ok(cell.into_cell(self.originals.sources()))
    }
}
