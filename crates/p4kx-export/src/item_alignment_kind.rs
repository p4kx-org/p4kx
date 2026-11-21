// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataType, StructValue, Value};

use super::{ATTACHABLE, CONTAINER, DEFINITION};
use crate::build::original_input::OriginalCatalogInput;

#[derive(Clone, Copy)]
pub(crate) enum ItemAlignmentKind {
    Port,
    Attachment,
}

#[derive(Clone, Copy)]
pub(crate) enum AttachmentTarget {
    Definition,
    Other,
    Unknown,
}

impl ItemAlignmentKind {
    pub(crate) fn of(
        value: &StructValue<'_>,
        input: &OriginalCatalogInput<'_, '_>,
    ) -> Option<Self> {
        match value.type_name() {
            CONTAINER => Some(Self::Port),
            ATTACHABLE
                if matches!(
                    attachment_target(value, input),
                    AttachmentTarget::Definition
                ) =>
            {
                Some(Self::Attachment)
            }
            _ => None,
        }
    }
}

pub(crate) fn attachment_target(
    source: &StructValue<'_>,
    input: &OriginalCatalogInput<'_, '_>,
) -> AttachmentTarget {
    let Some(field) = source
        .fields()
        .iter()
        .find(|field| field.name() == "AttachDef")
    else {
        return AttachmentTarget::Other;
    };
    let storage = field.storage();
    if storage.is_array()
        || !matches!(
            storage.data_type(),
            DataType::Class | DataType::StrongPointer
        )
    {
        return AttachmentTarget::Other;
    }
    match field.value() {
        Value::Struct(definition) if definition.type_name() == DEFINITION => {
            AttachmentTarget::Definition
        }
        Value::Struct(_) => AttachmentTarget::Other,
        _ => match input.struct_name(storage.target()) {
            Some(DEFINITION) => AttachmentTarget::Definition,
            Some(_) => AttachmentTarget::Other,
            None => AttachmentTarget::Unknown,
        },
    }
}
