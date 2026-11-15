// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceUse, SourceUseRole, SourceUses, SourceValue, SourcedValue, Value};

use crate::catalog_read::original::{
    OriginalDatum, OriginalFlowFields, OriginalValueFact, OriginalValuePart,
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

    pub(super) fn from_fact(
        fact: &OriginalValueFact<'_, '_, 'source>,
    ) -> Result<Self, CatalogError> {
        use SourceUseRole::{Attempted, Conversion, Payload, Selector};

        let part = OriginalValuePart::Value;
        let registration;
        let mut key;
        if let Some(text) = crate::mission_layers::key_value(fact.value()) {
            let payload = fact.observe(part, Payload)?;
            let selector = fact.observe(part, Selector)?;
            registration = Some((payload, selector));
            key = fact.bind(part, SourceValue::Present(text), Payload, true)?;
        } else if matches!(fact.value(), Value::Error(_)) {
            return Ok(Self(Evidence::Authored {
                key: fact.bind(part, SourceValue::Unreadable, Attempted, false)?,
                registration: None,
            }));
        } else {
            registration = None;
            key = fact.bind(part, SourceValue::Absent, Selector, true)?;
        }
        key.contribute(fact.observe(part, Conversion)?);
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
