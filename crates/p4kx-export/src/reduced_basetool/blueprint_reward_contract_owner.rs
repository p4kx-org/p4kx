// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::{ReducedError, incomplete};
use crate::guid_text::CatalogGuid;
use crate::source_value_catalog::{
    SourceArenaOutput, SourceField, SourcePayload, SourceRecordReference, SourceValueRef,
    SourceWeakTarget,
};

/// Reads the nearest GUID-owning original ancestor of the exact reward occurrence.
pub(super) fn nearest(
    arena: &SourceArenaOutput<'_>,
    ordinal: u64,
    id: &CatalogGuid,
    pool: &SourceRecordReference<'_>,
) -> Result<Option<u64>, ReducedError> {
    let mut identity = authored_id(arena, arena.roots());
    let mut owner = None;
    let mut link = covering(arena.roots(), ordinal, SourceField::value).ok_or_else(incomplete)?;
    loop {
        let value = arena
            .value(link)
            .and_then(|node| node.value().value())
            .ok_or_else(incomplete)?;
        let position = link.value_index();
        if position == ordinal {
            if identity.as_ref() != Some(id) || !same_target(value, pool) {
                return Err(incomplete());
            }
            return Ok(owner);
        }
        link = match value {
            SourcePayload::Struct { fields, .. } => {
                if let Some(id) = authored_id(arena, fields) {
                    identity = Some(id);
                    owner = Some(position);
                }
                covering(fields, ordinal, SourceField::value)
            }
            SourcePayload::Array { elements } => covering(elements, ordinal, |link| *link),
            _ => None,
        }
        .filter(|next| next.value_index() > position)
        .ok_or_else(incomplete)?;
    }
}

fn authored_id(arena: &SourceArenaOutput<'_>, fields: &[SourceField<'_>]) -> Option<CatalogGuid> {
    let field = fields
        .iter()
        .find(|field| field.name().value() == Some(&"id"))?;
    match arena.value(field.value())?.value().value()? {
        SourcePayload::Guid { value } => Some(*value),
        _ => None,
    }
}

fn covering<T>(
    children: &[T],
    ordinal: u64,
    link: impl Fn(&T) -> SourceValueRef,
) -> Option<SourceValueRef> {
    // Original child subtrees occupy disjoint, increasing ranges of owner-local indices.
    let after = children.partition_point(|child| link(child).value_index() <= ordinal);
    children.get(after.checked_sub(1)?).map(link)
}

fn same_target(value: &SourcePayload<'_>, pool: &SourceRecordReference<'_>) -> bool {
    let observed = match value {
        SourcePayload::RecordReference { value } => {
            (value.guid.value(), value.target_record_index().value())
        }
        SourcePayload::WeakReference { value }
            if matches!(value.target(), SourceWeakTarget::Record { .. }) =>
        {
            (
                value.record_guid().value(),
                value.target_record_index().value(),
            )
        }
        _ => return false,
    };
    let expected = (pool.guid.value(), pool.target_record_index().value());
    observed == expected
}
