// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::Value;

use crate::owned_reference::OwnedReferenceStatus;
use crate::owned_reference_read;
use crate::scoped_location_link::{PropertyReference, PropertyReferenceStatus};
use crate::spawn_read::Reader;

/// Interprets a source weak link as a mission-property variable in its original owner.
pub(crate) fn read(
    reader: &Reader<'_, '_>,
    value: &Value<'_>,
) -> Result<Option<PropertyReference>, ()> {
    let Some(target) = owned_reference_read::read(reader, value)? else {
        return Ok(None);
    };
    let reference = target.reference;
    let mut link = PropertyReference {
        target_record_index: reference.target_record_index,
        record_guid: reference.record_guid,
        record_path: reference.record_path,
        target_type: reference.target_type,
        location: reference.location,
        variable: None,
        status: match reference.status {
            OwnedReferenceStatus::Resolved => PropertyReferenceStatus::Resolved,
            OwnedReferenceStatus::MissingTarget => PropertyReferenceStatus::MissingTarget,
            OwnedReferenceStatus::OutsideOwner => PropertyReferenceStatus::OutsideOwner,
        },
        source_pointer: reference.source_pointer,
        source_error: reference.source_error,
    };
    let Some(property) = target.target else {
        return Ok(Some(link));
    };
    if property.type_name() != "MissionProperty" {
        link.status = PropertyReferenceStatus::UnsupportedTargetType;
        return Ok(Some(link));
    }
    match property.field("missionVariableName") {
        Some(Value::String(variable)) => {
            link.variable = Some((*variable).to_owned());
        }
        Some(Value::Error(error)) => {
            link.status = PropertyReferenceStatus::UnreadableTarget;
            link.source_error = Some(error.to_string());
        }
        Some(value) if !matches!(value, Value::Null) => {
            link.status = PropertyReferenceStatus::UnreadableTarget;
            link.source_error = Some("missionVariableName is not a string".to_owned());
        }
        _ => link.status = PropertyReferenceStatus::MissingTarget,
    }
    Ok(Some(link))
}
