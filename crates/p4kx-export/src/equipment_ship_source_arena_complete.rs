// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::source_value_catalog::{
    SourceArenaOutput, SourceField, SourcePayload, SourceWeakReference, SourceWeakTarget,
};

use super::{known, reference};

pub(super) fn complete(arena: &SourceArenaOutput<'_>) -> bool {
    fields(arena.roots())
        && arena
            .values()
            .iter()
            .all(|node| known(node.value()) && node.value().value().is_none_or(payload))
}

fn fields(fields: &[SourceField<'_>]) -> bool {
    fields.iter().all(|field| {
        let declaration = field.declaration();
        known(field.name())
            && known(declaration.data_type())
            && known(declaration.is_array())
            && known(declaration.target())
    })
}

fn payload(value: &SourcePayload<'_>) -> bool {
    match value {
        SourcePayload::Struct {
            source_type,
            fields: members,
        } => known(source_type) && fields(members),
        SourcePayload::RecordReference { value } => reference(value),
        SourcePayload::WeakReference { value } => weak(value),
        SourcePayload::Array { .. }
        | SourcePayload::Boolean { .. }
        | SourcePayload::Number { .. }
        | SourcePayload::String { .. }
        | SourcePayload::Locale { .. }
        | SourcePayload::Enum { .. }
        | SourcePayload::Guid { .. } => true,
    }
}

fn weak(value: &SourceWeakReference<'_>) -> bool {
    known(value.target_record_index())
        && known(value.record_guid())
        && known(value.record_path())
        && match value.target() {
            SourceWeakTarget::Record { source_type } => known(source_type),
            SourceWeakTarget::Instance {
                source_type,
                location,
            } => known(source_type) && known(location),
        }
}
