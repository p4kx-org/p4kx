// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceUse, SourceUseRole, SourceUses, SourceValue, SourcedValue, Value};

use crate::catalog_read::original::{
    OriginalDatum, OriginalFlowFields, OriginalRecordScope, OriginalValueFact, OriginalValuePart,
};
use crate::{CatalogError, ExportAccountingError};

/// An exact original text-key occurrence awaiting canonical publication.
#[derive(Debug, Clone)]
pub(crate) struct ProtectedKey<'source>(Evidence<'source>);

#[derive(Debug, Clone)]
enum Evidence<'source> {
    Authored {
        key: OriginalDatum<&'source str>,
        registration: Option<(SourceUse, SourceUse)>,
    },
    Absent(SourceUse),
}

impl<'source> ProtectedKey<'source> {
    pub(crate) fn read<'scope, 'read>(
        fields: &OriginalFlowFields<'scope, 'read, 'source>,
        owner: Option<&OriginalValueFact<'scope, 'read, 'source>>,
        name: &str,
    ) -> Result<Self, CatalogError> {
        match fields.field(owner, name)? {
            Some((_, fact)) => Self::from_fact(fact),
            None => Ok(Self(Evidence::Absent(fields.survey(owner)?))),
        }
    }

    /// Records the absence of a field in the admitted original root.
    pub(crate) fn absent_from_root(
        scope: &OriginalRecordScope<'_, 'source>,
    ) -> Result<Self, CatalogError> {
        let survey = scope
            .dataforge()
            .at(scope.root_declaration_location(), SourceUseRole::Selector)?;
        Ok(Self(Evidence::Absent(survey)))
    }

    pub(crate) fn from_fact(
        fact: &OriginalValueFact<'_, '_, 'source>,
    ) -> Result<Self, CatalogError> {
        let value = match fact.value() {
            Value::Error(_) => SourceValue::Unreadable,
            value => crate::mission_layers::key_value(value)
                .map_or(SourceValue::Absent, SourceValue::Present),
        };
        Self::capture(fact, value)
    }

    /// Retains an authored Locale token verbatim, including an empty or placeholder token.
    pub(crate) fn read_locale<'scope, 'read>(
        fields: &OriginalFlowFields<'scope, 'read, 'source>,
        owner: Option<&OriginalValueFact<'scope, 'read, 'source>>,
        name: &str,
    ) -> Result<Self, CatalogError> {
        let Some((_, fact)) = fields.field(owner, name)? else {
            return Ok(Self(Evidence::Absent(fields.survey(owner)?)));
        };
        let value = match fact.value() {
            Value::Locale(token) => SourceValue::Present(*token),
            Value::Null => SourceValue::Null,
            Value::Error(_) => SourceValue::Unreadable,
            _ => return Err(ExportAccountingError::ConflictingSourceDatum.into()),
        };
        Self::capture(fact, value)
    }

    fn capture(
        fact: &OriginalValueFact<'_, '_, 'source>,
        value: SourceValue<&'source str>,
    ) -> Result<Self, CatalogError> {
        use SourceUseRole::{Attempted, Conversion, Payload, Selector};

        let part = OriginalValuePart::Value;
        let (role, registration) = match &value {
            SourceValue::Present(_) => (
                Payload,
                Some((fact.observe(part, Payload)?, fact.observe(part, Selector)?)),
            ),
            SourceValue::Unreadable => (Attempted, None),
            _ => (Selector, None),
        };
        let complete = role != Attempted;
        let mut key = fact.bind(part, value, role, complete)?;
        if complete {
            key.contribute(fact.observe(part, Conversion)?);
        }
        Ok(Self(Evidence::Authored { key, registration }))
    }

    pub(super) fn map<T>(
        self,
        sources: &SourceUses,
        publish: impl FnOnce(&'source str, SourceUse, SourceUse) -> Result<T, CatalogError>,
    ) -> Result<SourcedValue<T>, CatalogError> {
        match self.0 {
            Evidence::Authored { key, registration } => Ok(key
                .try_map(|key| {
                    let (payload, selector) =
                        registration.ok_or(ExportAccountingError::ConflictingSourceDatum)?;
                    publish(key, payload, selector)
                })?
                .into_cell(sources)),
            Evidence::Absent(survey) => Ok(sources.value(SourceValue::Absent, &[survey], true)),
        }
    }
}
