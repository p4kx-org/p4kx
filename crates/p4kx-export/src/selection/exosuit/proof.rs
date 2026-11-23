// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    DataType, Record, SourceUse, SourceUseRole, SourceValue, SourcedValue, Value,
};

use super::Exosuit;
use super::controls::{Control, ControlRead, entry_port, entry_reference};
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_read::original::{OriginalFlowFields, OriginalValuePart};
use crate::playability_projection;
use crate::record_header_field::{RecordHeaderField, RecordHeaderInput};
use crate::{CatalogError, ExportAccountingError};

pub(crate) fn observe(
    input: &OriginalCatalogInput<'_, '_>,
    record: &Record<'_>,
    fields: &OriginalFlowFields<'_, '_, '_>,
) -> Result<Option<SourcedValue<()>>, CatalogError> {
    let path = record.record_path().map_err(CatalogError::DataForge)?;
    if Exosuit::by_path(path).is_none() {
        return Ok(None);
    }
    let name = record.name().map_err(CatalogError::DataForge)?;
    let source_type = record.type_name().map_err(CatalogError::DataForge)?;
    let Some(exosuit) = Exosuit::from_header(path, name, source_type) else {
        return Ok(None);
    };
    let context = input.record_context(record)?;
    let components = fields.component_selection(None)?;
    let mut uses = components.uses;
    for field in [
        RecordHeaderInput::NormalizedPath,
        RecordHeaderInput::Field(RecordHeaderField::RecordName),
        RecordHeaderInput::Field(RecordHeaderField::SourceType),
    ] {
        context.append_header_input(field, SourceUseRole::Selector, &mut uses)?;
    }
    let verified = match Control::read(exosuit, fields.root()) {
        ControlRead::Present(control) => observe_control(input, fields, &control, &mut uses)?,
        ControlRead::Unreadable(Some(index)) => {
            input.origins().record(index).append_header_input(
                RecordHeaderInput::NormalizedPath,
                SourceUseRole::Selector,
                &mut uses,
            )?;
            uses.push(input.origins().dataforge.at(
                playability_projection::attempted_body(index),
                SourceUseRole::Attempted,
            )?);
            false
        }
        ControlRead::Absent | ControlRead::Unreadable(None) => false,
    };
    let complete = components.complete && verified;
    let state = if complete {
        SourceValue::Present(())
    } else {
        uses.push(input.origins().dataforge.at(
            playability_projection::attempted_body(record.index()),
            SourceUseRole::Attempted,
        )?);
        SourceValue::Unreadable
    };
    Ok(Some(fields.sources().value(state, &uses, complete)))
}

fn observe_control<'source>(
    input: &OriginalCatalogInput<'_, '_>,
    fields: &OriginalFlowFields<'_, '_, 'source>,
    control: &Control<'_, 'source>,
    uses: &mut Vec<SourceUse>,
) -> Result<bool, CatalogError> {
    for component in control.actor.controllers {
        let owner = fields.fact(component)?;
        if let Some((_, enabled)) = fields.field(Some(owner), "enabled")? {
            uses.push(enabled.observe(OriginalValuePart::Value, SourceUseRole::Selector)?);
        } else {
            uses.push(fields.survey(Some(owner))?);
        }
    }
    let entries_complete = observe_entries(fields, control.actor.entries, uses)?;
    let selector = fields
        .fact(control.actor.seat_link)?
        .reference_observation()?;
    if selector.record_index() != control.seat.record.index() {
        return Err(ExportAccountingError::ConflictingSourceDatum.into());
    }
    let declared = selector.matches_record_declaration(uses, || fields.check_cancel())?;
    let (_, seat) = control
        .seat
        .component()
        .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
    let scope = input.record_scope(&control.seat.record, &control.seat.tree)?;
    let seat_fields = OriginalFlowFields::read(&scope, None)?;
    let selected = seat_fields.component_selection(None)?;
    uses.extend(selected.uses);
    let Some(Value::Struct(attachment)) = seat.field("actorAttachment") else {
        return Err(ExportAccountingError::ConflictingSourceDatum.into());
    };
    for value in [seat.field("seatType"), attachment.field("type")] {
        observe_value(
            &seat_fields,
            value.ok_or(ExportAccountingError::ConflictingSourceDatum)?,
            uses,
        )?;
    }
    let modes = seat
        .field("availableOperatorModes")
        .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
    let selector = seat_fields.fact(modes)?.reference_observation()?;
    let guid = selector.guid_selector()?;
    let (contributors, mode_complete) = guid.contributions();
    uses.extend_from_slice(contributors);
    let mode_declared = selector.matches_record_declaration(uses, || fields.check_cancel())?;
    Ok(entries_complete && declared && selected.complete && mode_complete && mode_declared)
}

fn observe_entries(
    fields: &OriginalFlowFields<'_, '_, '_>,
    entries: &[Value<'_>],
    uses: &mut Vec<SourceUse>,
) -> Result<bool, CatalogError> {
    let mut complete = true;
    for entry in entries {
        if let Some(port) = entry_port(entry) {
            observe_value(fields, port, uses)?;
        }
        if let Some(attachment) = entry_reference(entry) {
            let fact = fields.fact(attachment.value())?;
            uses.push(fact.observe(OriginalValuePart::DataType, SourceUseRole::Selector)?);
            if attachment.storage().data_type() == DataType::Reference
                && matches!(attachment.value(), Value::Reference(_))
            {
                let selector = fact.reference_observation()?;
                let path = selector.record_path()?;
                let (contributors, available) = path.contributions();
                uses.extend_from_slice(contributors);
                complete &= available;
            } else {
                observe_value(fields, attachment.value(), uses)?;
            }
        }
    }
    Ok(complete)
}

fn observe_value(
    fields: &OriginalFlowFields<'_, '_, '_>,
    value: &Value<'_>,
    uses: &mut Vec<SourceUse>,
) -> Result<(), CatalogError> {
    uses.push(
        fields
            .fact(value)?
            .observe(OriginalValuePart::Value, SourceUseRole::Selector)?,
    );
    Ok(())
}
