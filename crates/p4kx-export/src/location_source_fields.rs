// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;

use p4kx_extraction::{FieldOrigin, SourceUses, SourceValue, SourceValueState, SourcedValue};

use super::super::authored_fields::Observed;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_read::original::OriginalFlowFields;
use crate::location_template::LocationName;
use crate::mission_location_tag::Reader;
use crate::source_fields::FieldError;
use crate::source_value_catalog::SourceRecordReference;
use crate::template_field_capture::TemplateFieldCapture;
use crate::{CatalogError, ExportAccountingError};

#[path = "location_source_names.rs"]
mod names;

pub(crate) struct LocationSourceFields<'source> {
    pub(crate) disabled: SourcedValue<bool>,
    pub(crate) is_security_network_host: SourcedValue<bool>,
    pub(crate) entity_cluster_member: SourcedValue<Box<SourceRecordReference<'source>>>,
    pub(crate) names: SourcedValue<Vec<SourcedValue<LocationName>>>,
}

pub(crate) fn read<'fields, 'scope, 'read, 'source>(
    originals: &'fields OriginalFlowFields<'scope, 'read, 'source>,
    input: &OriginalCatalogInput<'_, 'source>,
    capture: &TemplateFieldCapture<'_, 'scope, 'read, 'source>,
    reader: &Reader<'_, 'fields, 'scope, 'read, 'source>,
    projection: &CatalogProjection<'_, 'source>,
    notes: &mut Vec<FieldError>,
) -> Result<LocationSourceFields<'source>, CatalogError> {
    let selected = reader.original_owner(&["locationData"])?;
    let sources = originals.sources();
    let (disabled, is_security_network_host, entity_cluster_member) = if let Some(owner) =
        selected.value()
    {
        let numbers = RefCell::default();
        let fields = Observed::definition(originals, input, capture, &numbers).at_owner(owner);
        let origin = selected.origin();
        (
            sources.add_value_origins(fields.boolean("disabled", notes)?, &[origin])?,
            sources
                .add_value_origins(fields.boolean("isSecurityNetworkHost", notes)?, &[origin])?,
            cluster(
                fields.record_or_weak_record_link("entityClusterMember", notes)?,
                sources,
                origin,
            )?,
        )
    } else {
        (
            non_present(sources, &selected)?,
            non_present(sources, &selected)?,
            non_present(sources, &selected)?,
        )
    };
    Ok(LocationSourceFields {
        disabled,
        is_security_network_host,
        entity_cluster_member,
        names: names::read(originals, capture, reader, projection, notes)?,
    })
}

fn cluster<'source>(
    value: SourcedValue<Box<SourceRecordReference<'source>>>,
    sources: &SourceUses,
    selection: &FieldOrigin,
) -> Result<SourcedValue<Box<SourceRecordReference<'source>>>, CatalogError> {
    let value = value.try_map(|reference| {
        let SourceRecordReference {
            target_record_index,
            guid,
            record_path,
            target_type,
            status,
        } = *reference;
        Ok::<_, CatalogError>(Box::new(SourceRecordReference {
            target_record_index: sources.add_value_origins(target_record_index, &[selection])?,
            guid: sources.add_value_origins(guid, &[selection])?,
            record_path: sources.add_value_origins(record_path, &[selection])?,
            target_type: sources.add_value_origins(target_type, &[selection])?,
            status,
        }))
    })?;
    Ok(sources.add_value_origins(value, &[selection])?)
}

fn non_present<T, Selected>(
    sources: &SourceUses,
    selected: &SourcedValue<Selected>,
) -> Result<SourcedValue<T>, CatalogError> {
    let state = match selected.state() {
        SourceValueState::Absent => SourceValue::Absent,
        SourceValueState::Null => SourceValue::Null,
        SourceValueState::Unreadable => SourceValue::Unreadable,
        SourceValueState::Unsupported => SourceValue::Unsupported,
        SourceValueState::Invalid => SourceValue::Invalid,
        _ => return Err(ExportAccountingError::ConflictingSourceDatum.into()),
    };
    Ok(sources.value_with_origins(state, &[selected.origin()])?)
}
