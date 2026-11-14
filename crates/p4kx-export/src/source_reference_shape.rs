// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Guid, Reference, Value, ValueError};

use crate::definition_reference::{DefinitionReference, DefinitionStatus};
use crate::guid_text::CatalogGuid;

/// Shape classification before the legacy presentation boundary owns target text.
pub(crate) enum ReferenceShape<'value, 'source> {
    Resolved(&'value Reference<'source>),
    Unresolved {
        guid: Guid,
        status: DefinitionStatus,
    },
}

/// The original value together with its checked Reference interpretation.
pub(crate) struct ReferenceSlot<'scope, 'source> {
    pub(crate) value: &'scope Value<'source>,
    pub(crate) reference: ReferenceShape<'scope, 'source>,
}

pub(super) fn read<'value, 'source>(
    value: &'value Value<'source>,
) -> Result<Option<ReferenceShape<'value, 'source>>, &'static str> {
    Ok(match value {
        Value::Null => None,
        Value::Reference(target) if target.location().is_none() => {
            Some(ReferenceShape::Resolved(target))
        }
        Value::Error(ValueError::DanglingReference { guid }) => Some(ReferenceShape::Unresolved {
            guid: *guid,
            status: DefinitionStatus::Dangling,
        }),
        Value::Error(ValueError::UnexpectedReferenceType { guid, .. }) => {
            Some(ReferenceShape::Unresolved {
                guid: *guid,
                status: DefinitionStatus::UnexpectedTargetType,
            })
        }
        _ => return Err("a nullable Record Reference"),
    })
}

impl ReferenceShape<'_, '_> {
    pub(crate) fn into_owned(self) -> DefinitionReference {
        match self {
            Self::Resolved(target) => DefinitionReference {
                target_record_index: Some(target.record_index()),
                guid: CatalogGuid(target.guid()),
                record_path: Some(target.record_path().to_owned()),
                target_type: Some(target.type_name().to_owned()),
                status: DefinitionStatus::Present,
            },
            Self::Unresolved { guid, status } => DefinitionReference {
                target_record_index: None,
                guid: CatalogGuid(guid),
                record_path: None,
                target_type: None,
                status,
            },
        }
    }
}
