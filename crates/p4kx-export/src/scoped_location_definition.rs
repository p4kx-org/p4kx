// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Guid, Reference, StructValue, Value, ValueError};

use crate::guid_text::CatalogGuid;
use crate::record_read::ReadError;
use crate::scoped_location_link::{DefinitionReference, DefinitionStatus};
use crate::source_fields::FieldCapture;

pub(super) enum DefinitionShape<'value, 'source> {
    Resolved(&'value Reference<'source>),
    Dangling(Guid),
}

pub(super) fn read_context<'value, 'source>(
    parent: &StructValue<'source>,
    field: &'static str,
    value: &'value Value<'source>,
    capture: Option<&dyn FieldCapture<'source>>,
) -> Result<Option<DefinitionShape<'value, 'source>>, ReadError> {
    if let Some(capture) = capture {
        capture.check_parent(parent)?;
    }
    Ok(match value {
        Value::Null => None,
        Value::Reference(target) if target.location().is_none() => {
            Some(DefinitionShape::Resolved(target))
        }
        Value::Error(ValueError::DanglingReference { guid }) => {
            Some(DefinitionShape::Dangling(*guid))
        }
        _ => {
            return Err(super::unexpected_context(
                parent,
                field,
                value,
                "a nullable Record Reference",
                capture,
            ));
        }
    })
}

impl DefinitionShape<'_, '_> {
    pub(super) fn into_owned(self) -> DefinitionReference {
        match self {
            Self::Resolved(target) => DefinitionReference {
                target_record_index: Some(target.record_index()),
                guid: CatalogGuid(target.guid()),
                record_path: Some(target.record_path().to_owned()),
                target_type: Some(target.type_name().to_owned()),
                status: DefinitionStatus::Present,
            },
            Self::Dangling(guid) => DefinitionReference {
                target_record_index: None,
                guid: CatalogGuid(guid),
                record_path: None,
                target_type: None,
                status: DefinitionStatus::Dangling,
            },
        }
    }
}
