// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::scalar;
use crate::{DataForge, Guid, Value, ValueError, ValueKind};

const NULL: u32 = u32::MAX;

pub(crate) fn element_bytes<'source>(
    forge: &DataForge<'source>,
    kind: ValueKind,
    element: u32,
) -> &'source [u8] {
    let len = kind.element_len();
    let start = (element as usize).checked_mul(len);
    let end = start.and_then(|start| start.checked_add(len));
    forge
        .values
        .get(kind.index())
        .zip(start.zip(end))
        .and_then(|(pool, (start, end))| pool.get(start..end))
        .unwrap_or_default()
}

/// Returns the two stored words of a pointer or array header.
pub(crate) fn words(bytes: &[u8]) -> [u32; 2] {
    [0, 4].map(|at| u32::from_le_bytes(scalar::first_bytes(bytes.get(at..).unwrap_or_default())))
}

/// Returns the original pointer target, or no target for a null pointer.
pub(crate) fn pointer(bytes: &[u8]) -> Option<(u32, u32)> {
    let [structure, instance] = words(bytes);
    (structure != NULL || instance != NULL).then_some((structure, instance))
}

/// Returns the stored Reference GUID following its unused instance index.
pub(crate) fn reference_guid(bytes: &[u8]) -> Option<Guid> {
    let guid = Guid::from_stored(scalar::first_bytes(bytes.get(4..).unwrap_or_default()));
    (guid != Guid::default()).then_some(guid)
}

pub(super) fn reference<'source>(
    forge: &'source DataForge<'source>,
    bytes: &[u8],
) -> Value<'source> {
    let Some(guid) = reference_guid(bytes) else {
        return Value::Null;
    };
    forge.record_by_guid(guid).map_or(
        Value::Error(ValueError::DanglingReference { guid }),
        |record| record.link(),
    )
}
