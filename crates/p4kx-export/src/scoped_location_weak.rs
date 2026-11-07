// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Reference, Value, ValueError};

use crate::guid_text::CatalogGuid;
use crate::scoped_location_link::{PropertyReference, PropertyReferenceStatus, SourcePointer};
use crate::spawn_read::Reader;

/// Preserves a weak link without resolving outside its original owning Record.
pub(crate) fn read(
    reader: &Reader<'_, '_>,
    value: &Value<'_>,
) -> Result<Option<PropertyReference>, ()> {
    let mut link = match value {
        Value::Null => None,
        Value::Reference(target) => Some(reference(reader, target)),
        Value::Error(ValueError::OutsideRecord {
            struct_index,
            instance_index,
        }) => Some(unresolved(
            PropertyReferenceStatus::OutsideOwner,
            *struct_index,
            *instance_index,
        )),
        Value::Error(ValueError::InvalidPointer {
            struct_index,
            instance_index,
        }) => Some(unresolved(
            PropertyReferenceStatus::MissingTarget,
            *struct_index,
            *instance_index,
        )),
        Value::Error(ValueError::DanglingReference { guid }) => {
            let mut link = empty(PropertyReferenceStatus::MissingTarget);
            link.record_guid = Some(CatalogGuid(*guid));
            Some(link)
        }
        _ => return Err(()),
    };
    if let (Some(link), Value::Error(error)) = (&mut link, value) {
        link.source_error = Some(error.to_string());
    }
    Ok(link)
}

fn reference(reader: &Reader<'_, '_>, target: &Reference<'_>) -> PropertyReference {
    let mut link = empty(PropertyReferenceStatus::OutsideOwner);
    link.target_record_index = Some(target.record_index());
    link.record_guid = Some(CatalogGuid(target.guid()));
    link.record_path = Some(target.record_path().to_owned());
    link.target_type = Some(target.type_name().to_owned());
    link.location = target
        .location()
        .map(|location| location.as_str().to_owned());
    if !reader.owns(target) {
        return link;
    }
    let Some(property) = reader.owned_target(target) else {
        link.status = PropertyReferenceStatus::MissingTarget;
        return link;
    };
    if property.type_name() != "MissionProperty" {
        link.status = PropertyReferenceStatus::UnsupportedTargetType;
        return link;
    }
    match property.field("missionVariableName") {
        Some(Value::String(variable)) => {
            link.status = PropertyReferenceStatus::Resolved;
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
    link
}

const fn unresolved(
    status: PropertyReferenceStatus,
    struct_index: u32,
    instance_index: u32,
) -> PropertyReference {
    let mut link = empty(status);
    link.source_pointer = Some(SourcePointer {
        struct_index,
        instance_index,
    });
    link
}

const fn empty(status: PropertyReferenceStatus) -> PropertyReference {
    PropertyReference {
        target_record_index: None,
        record_guid: None,
        record_path: None,
        target_type: None,
        location: None,
        variable: None,
        status,
        source_pointer: None,
        source_error: None,
    }
}
