// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    DataType, SourceUse, SourceUseRole, SourceUses, SourceValue, SourcedValue, Value,
};

use super::{Fact, Observed};
use crate::catalog_read::original::OriginalValuePart;
use crate::catalog_v3::authored_link::{self as link, HealthLink};
use crate::definition_reference::DefinitionStatus;
use crate::source_fields::FieldError;
use crate::source_value_catalog::{SourcePayload, SourceRecordReference, read_reference_payload};
use crate::{CatalogError, ExportAccountingError};

impl<'scope, 'read, 'source> Observed<'_, 'scope, 'read, 'source> {
    pub(in crate::catalog_v3) fn link(
        &self,
        name: &str,
        notes: &mut Vec<FieldError>,
    ) -> Result<SourcedValue<HealthLink<'source>>, CatalogError> {
        self.read_link(
            name,
            "a nullable Reference or weak pointer",
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
            notes,
            |fact, sources, selectors| self.decode_record_link(fact, sources, selectors),
        )
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
        let (part, state) =
            if let Some((part, target)) = decode(fact, self.originals.sources(), &mut selectors)? {
                (part, SourceValue::Present(target))
            } else {
                match fact.value() {
                    Value::Null => (OriginalValuePart::Value, SourceValue::Null),
                    Value::Struct(source) => (
                        OriginalValuePart::ConcreteType,
                        self.unsupported(fact, source, notes)?,
                    ),
                    other => (
                        OriginalValuePart::Value,
                        self.failure(name, other, expected, notes)?,
                    ),
                }
            };
        let mut cell = self.field_datum(fact, part, state, SourceUseRole::Selector)?;
        for selector in selectors {
            cell.contribute(selector);
        }
        Ok(cell.into_cell(self.originals.sources()))
    }
}
