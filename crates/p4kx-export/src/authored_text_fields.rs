// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;

use p4kx_extraction::{
    CancelToken, DataType, RecordTree, SourceUseRole, SourceValue, SourcedValue, Value,
};

use super::{Fact, Observed};
use crate::CatalogError;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_read::original::{OriginalFlowFields, OriginalValuePart};
use crate::catalog_text_contract::{GameTextRef, PublicNameRef};
use crate::record_observation::RecordObservation;
use crate::source_fields::FieldError;
use crate::template_field_capture::TemplateFieldCapture;

/// Reads one protected Locale from an admitted Record root.
pub(crate) fn read_root_locale<'source>(
    tree: &RecordTree<'source>,
    projection: &CatalogProjection<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    cancel: Option<&CancelToken>,
    field: &str,
) -> Result<(SourcedValue<GameTextRef>, Vec<FieldError>), CatalogError> {
    let input = projection.original();
    let scope = input.record_scope(admission.record(), tree)?;
    let originals = OriginalFlowFields::read(&scope, cancel)?;
    let capture = TemplateFieldCapture::new(&originals, input, admission)?;
    let numbers = RefCell::new(Vec::new());
    let fields = Observed::definition(&originals, input, &capture, &numbers);
    let mut notes = Vec::new();
    let locale = fields.protected_locale(field, projection, &mut notes)?;
    Ok((locale, notes))
}

impl<'scope, 'read, 'source> Observed<'_, 'scope, 'read, 'source> {
    pub(in crate::catalog_v3) fn public_name(
        &self,
        name: &str,
        projection: &CatalogProjection<'_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<SourcedValue<PublicNameRef>, CatalogError> {
        self.locale(name, notes, |fact| projection.public_name_fact(fact))
    }

    pub(in crate::catalog_v3) fn protected_locale(
        &self,
        name: &str,
        projection: &CatalogProjection<'_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<SourcedValue<GameTextRef>, CatalogError> {
        self.locale(name, notes, |fact| projection.protected_locale_fact(fact))
    }

    /// Reads ordered authored Locale occurrences in the protected text domain.
    pub(in crate::catalog_v3) fn protected_locale_array(
        &self,
        name: &str,
        projection: &CatalogProjection<'_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<SourcedValue<Vec<SourcedValue<GameTextRef>>>, CatalogError> {
        let Some((_, fact)) = self.originals.field(self.owner, name)? else {
            return self.absent();
        };
        let storage = fact.field().storage();
        let expected = "a nullable Locale array";
        let state = if storage.data_type() == DataType::Locale && storage.is_array() {
            self.array_elements(fact, name, expected, notes, |index, element, notes| {
                self.locale_cell(&format!("{name}[{index}]"), element, notes, |fact| {
                    projection.protected_locale_fact(fact)
                })
            })?
        } else {
            self.failure(name, fact.value(), expected, notes)?
        };
        self.bind_storage(fact, state)
    }

    /// Reads an authored Locale or a legacy String localization key in the protected domain.
    pub(in crate::catalog_v3) fn protected_locale_or_legacy_key(
        &self,
        name: &str,
        projection: &CatalogProjection<'_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<SourcedValue<GameTextRef>, CatalogError> {
        if let Some((_, fact)) = self.originals.field(self.owner, name)?
            && matches!(fact.value(), Value::String(_))
        {
            return projection.description_fact(fact);
        }
        self.protected_locale(name, projection, notes)
    }

    /// Reads a localized or directly stored authored text in its caller's domain.
    pub(in crate::catalog_v3) fn locale_or_string<T>(
        &self,
        name: &str,
        notes: &mut Vec<FieldError>,
        localize: impl FnOnce(&Fact<'scope, 'read, 'source>) -> Result<T, CatalogError>,
        literal: impl FnOnce(&'source str) -> T,
    ) -> Result<SourcedValue<T>, CatalogError> {
        let Some((_, fact)) = self.originals.field(self.owner, name)? else {
            return self.absent();
        };
        let (state, role) = match fact.value() {
            Value::Locale(_) => (
                SourceValue::Present(localize(fact)?),
                SourceUseRole::Conversion,
            ),
            Value::String(text) => (SourceValue::Present(literal(text)), SourceUseRole::Payload),
            Value::Null => (SourceValue::Null, SourceUseRole::Selector),
            value => (
                self.failure(name, value, "a nullable Locale or String", notes)?,
                SourceUseRole::Selector,
            ),
        };
        self.bind_field(fact, OriginalValuePart::Value, state, role)
    }

    fn locale<T>(
        &self,
        name: &str,
        notes: &mut Vec<FieldError>,
        retain: impl FnOnce(&Fact<'scope, 'read, 'source>) -> Result<T, CatalogError>,
    ) -> Result<SourcedValue<T>, CatalogError> {
        let Some((_, fact)) = self.originals.field(self.owner, name)? else {
            return self.absent();
        };
        self.locale_cell(name, fact, notes, retain)
    }

    fn locale_cell<T>(
        &self,
        name: &str,
        fact: &Fact<'scope, 'read, 'source>,
        notes: &mut Vec<FieldError>,
        retain: impl FnOnce(&Fact<'scope, 'read, 'source>) -> Result<T, CatalogError>,
    ) -> Result<SourcedValue<T>, CatalogError> {
        let (state, role) = match fact.value() {
            Value::Locale(_) => (
                SourceValue::Present(retain(fact)?),
                SourceUseRole::Conversion,
            ),
            Value::Null => (SourceValue::Null, SourceUseRole::Selector),
            value => (
                self.failure(name, value, "a nullable Locale", notes)?,
                SourceUseRole::Selector,
            ),
        };
        self.bind_field(fact, OriginalValuePart::Value, state, role)
    }
}
