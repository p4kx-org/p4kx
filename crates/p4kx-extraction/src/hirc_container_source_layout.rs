// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_audio::{BankContainerRead, BankError};
use schemars::JsonSchema;
use serde::Serialize;

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[schemars(rename = "RandomSequenceLayout")]
pub(in crate::hirc_view) enum ContainerSourceLayout {
    Decoded,
    Malformed,
    Unavailable,
    Unsupported,
}

pub(super) enum CheckedExtent {
    Absent,
    Prefix,
    Complete,
}

impl CheckedExtent {
    pub(super) fn observe<T>(
        fields: Option<&T>,
        complete: impl FnOnce(&T) -> Option<Range<u64>>,
    ) -> Self {
        match fields {
            Some(fields) if complete(fields).is_some() => Self::Complete,
            Some(_) => Self::Prefix,
            None => Self::Absent,
        }
    }
}

pub(super) const fn coverage(
    status: &BankContainerRead,
    extent: CheckedExtent,
) -> (ContainerSourceLayout, Option<&BankError>) {
    match (extent, status) {
        (CheckedExtent::Complete, _) => (ContainerSourceLayout::Decoded, None),
        (_, BankContainerRead::Unsupported(_)) => (ContainerSourceLayout::Unsupported, None),
        (CheckedExtent::Prefix, BankContainerRead::Malformed(error)) => {
            (ContainerSourceLayout::Malformed, Some(error))
        }
        (_, BankContainerRead::Malformed(error)) => {
            (ContainerSourceLayout::Unavailable, Some(error))
        }
        _ => (ContainerSourceLayout::Unavailable, None),
    }
}
