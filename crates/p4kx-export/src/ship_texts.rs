// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;

use p4kx_extraction::{CancelToken, RecordTree, SourceUseRole, SourceValue, SourcedValue, Value};

use super::{Fact, Observed};
use crate::CatalogError;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_read::original::{ComponentSelection, OriginalFlowFields, OriginalValuePart};
use crate::catalog_text_contract::{GameTextRef, PublicNameRef};
use crate::record_observation::RecordObservation;
use crate::source_fields::FieldError;
use crate::template_field_capture::TemplateFieldCapture;

pub(crate) struct ShipTexts {
    pub(crate) name: SourcedValue<PublicNameRef>,
    pub(crate) description: SourcedValue<GameTextRef>,
    pub(crate) role: SourcedValue<PublicNameRef>,
    pub(crate) career: SourcedValue<PublicNameRef>,
}

pub(crate) fn read<'source>(
    tree: &RecordTree<'source>,
    projection: &CatalogProjection<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    cancel: Option<&CancelToken>,
    selected: Option<&Value<'source>>,
) -> Result<(ShipTexts, Vec<FieldError>), CatalogError> {
    let input = projection.original();
    let scope = input.record_scope(admission.record(), tree)?;
    let originals = OriginalFlowFields::read(&scope, cancel)?;
    let selection = originals.component_selection(selected)?;
    let capture = TemplateFieldCapture::new(&originals, input, admission)?;
    let numbers = RefCell::new(Vec::new());
    let fields = Observed::definition(&originals, input, &capture, &numbers);
    let mut notes = Vec::new();
    let texts = match selected {
        Some(selected) => read_component_texts(
            fields.at_owner(originals.fact(selected)?),
            &selection,
            projection,
            &mut notes,
        )?,
        None => ShipTexts {
            name: missing_component(&originals, &selection),
            description: missing_component(&originals, &selection),
            role: missing_component(&originals, &selection),
            career: missing_component(&originals, &selection),
        },
    };
    Ok((texts, notes))
}

fn missing_component<T>(
    originals: &OriginalFlowFields<'_, '_, '_>,
    selection: &ComponentSelection,
) -> SourcedValue<T> {
    let state = if selection.complete {
        SourceValue::Absent
    } else {
        SourceValue::Unreadable
    };
    originals
        .sources()
        .value(state, &selection.uses, selection.complete)
}

fn read_component_texts<'source>(
    fields: Observed<'_, '_, '_, 'source>,
    selection: &ComponentSelection,
    projection: &CatalogProjection<'_, 'source>,
    notes: &mut Vec<FieldError>,
) -> Result<ShipTexts, CatalogError> {
    Ok(ShipTexts {
        name: fields.ship_locale("vehicleName", selection, notes, |fact| {
            projection.public_name_fact(fact)
        })?,
        description: fields.ship_locale("vehicleDescription", selection, notes, |fact| {
            projection.protected_locale_fact(fact)
        })?,
        role: fields.ship_locale("vehicleRole", selection, notes, |fact| {
            projection.public_name_fact(fact)
        })?,
        career: fields.ship_locale("vehicleCareer", selection, notes, |fact| {
            projection.public_name_fact(fact)
        })?,
    })
}

impl<'scope, 'read, 'source> Observed<'_, 'scope, 'read, 'source> {
    fn ship_locale<T>(
        &self,
        name: &str,
        selection: &ComponentSelection,
        notes: &mut Vec<FieldError>,
        retain: impl FnOnce(&Fact<'scope, 'read, 'source>) -> Result<T, CatalogError>,
    ) -> Result<SourcedValue<T>, CatalogError> {
        let sources = self.originals.sources();
        let Some((_, fact)) = self.originals.field(self.owner, name)? else {
            let mut uses = selection.uses.clone();
            uses.push(self.originals.survey(self.owner)?);
            return Ok(sources.value(SourceValue::Absent, &uses, selection.complete));
        };
        let (state, role) = match fact.value() {
            Value::Locale("") => (SourceValue::Absent, SourceUseRole::Selector),
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
        let mut cell = fact.bind(OriginalValuePart::Value, state, role, selection.complete)?;
        cell.contribute(fact.observe(OriginalValuePart::FieldName, SourceUseRole::Selector)?);
        for source in &selection.uses {
            cell.contribute(source.clone());
        }
        Ok(cell.into_cell(sources))
    }
}
