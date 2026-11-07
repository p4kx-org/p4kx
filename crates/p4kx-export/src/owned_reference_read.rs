// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Reference, StructValue, Value, ValueError};

use crate::guid_text::CatalogGuid;
use crate::owned_reference::{OwnedReference, OwnedReferenceStatus};
use crate::source_pointer::SourcePointer;
use crate::spawn_read::Reader;

/// Raw target identity and its immutable node, without interpreting node-specific values.
pub(crate) struct OwnedTarget<'r, 'f> {
    pub(crate) reference: OwnedReference,
    pub(crate) target: Option<&'r StructValue<'f>>,
}

/// Reads a weak link without following another owner or decoding a second tree.
pub(crate) fn read<'r, 'f>(
    reader: &Reader<'r, 'f>,
    value: &Value<'f>,
) -> Result<Option<OwnedTarget<'r, 'f>>, ()> {
    let mut result = match value {
        Value::Null => return Ok(None),
        Value::Reference(target) => return Ok(Some(reference(reader, target))),
        Value::Error(ValueError::OutsideRecord {
            struct_index,
            instance_index,
        }) => unresolved(
            OwnedReferenceStatus::OutsideOwner,
            *struct_index,
            *instance_index,
        ),
        Value::Error(ValueError::InvalidPointer {
            struct_index,
            instance_index,
        }) => unresolved(
            OwnedReferenceStatus::MissingTarget,
            *struct_index,
            *instance_index,
        ),
        Value::Error(ValueError::DanglingReference { guid }) => {
            let mut reference = empty(OwnedReferenceStatus::MissingTarget);
            reference.record_guid = Some(CatalogGuid(*guid));
            OwnedTarget {
                reference,
                target: None,
            }
        }
        _ => return Err(()),
    };
    if let Value::Error(error) = value {
        result.reference.source_error = Some(error.to_string());
    }
    Ok(Some(result))
}

fn reference<'r, 'f>(reader: &Reader<'r, 'f>, source: &Reference<'f>) -> OwnedTarget<'r, 'f> {
    let mut reference = empty(OwnedReferenceStatus::OutsideOwner);
    reference.target_record_index = Some(source.record_index());
    reference.record_guid = Some(CatalogGuid(source.guid()));
    reference.record_path = Some(source.record_path().to_owned());
    reference.target_type = Some(source.type_name().to_owned());
    reference.location = source
        .location()
        .map(|location| location.as_str().to_owned());
    let owned = reader.owns(source);
    let target = owned.then(|| reader.owned_target(source)).flatten();
    if owned {
        reference.status = if target.is_some() {
            OwnedReferenceStatus::Resolved
        } else {
            OwnedReferenceStatus::MissingTarget
        };
    }
    OwnedTarget { reference, target }
}

const fn unresolved<'r, 'f>(
    status: OwnedReferenceStatus,
    struct_index: u32,
    instance_index: u32,
) -> OwnedTarget<'r, 'f> {
    let mut reference = empty(status);
    reference.source_pointer = Some(SourcePointer {
        struct_index,
        instance_index,
    });
    OwnedTarget {
        reference,
        target: None,
    }
}

const fn empty(status: OwnedReferenceStatus) -> OwnedReference {
    OwnedReference {
        target_record_index: None,
        record_guid: None,
        record_path: None,
        target_type: None,
        location: None,
        status,
        source_pointer: None,
        source_error: None,
    }
}
