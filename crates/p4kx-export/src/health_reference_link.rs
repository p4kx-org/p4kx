// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Guid, ValueError};

use super::health_reference_data::ReferencePointer;
use crate::ExportReferenceStatus;

/// Descriptive unresolved-link facts classified from one retained decoder error.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ReferenceLink {
    pub(crate) target: Option<Guid>,
    pub(crate) status: ExportReferenceStatus,
    pub(crate) pointer: Option<ReferencePointer>,
}

impl ReferenceLink {
    pub(crate) const fn from_error(error: ValueError) -> Option<Self> {
        let (target, status, pointer) = match error {
            ValueError::DanglingReference { guid } => {
                (Some(guid), ExportReferenceStatus::Dangling, None)
            }
            ValueError::OutsideRecord {
                struct_index,
                instance_index,
            } => (
                None,
                ExportReferenceStatus::OutsideOwner,
                Some(words(struct_index, instance_index)),
            ),
            ValueError::InvalidPointer {
                struct_index,
                instance_index,
            } => (
                None,
                ExportReferenceStatus::InvalidPointer,
                Some(words(struct_index, instance_index)),
            ),
            ValueError::OwnedTwice {
                struct_index,
                instance_index,
            } => (
                None,
                ExportReferenceStatus::OwnedTwice,
                Some(words(struct_index, instance_index)),
            ),
            _ => return Self::typed(error),
        };
        Some(Self {
            target,
            status,
            pointer,
        })
    }

    const fn typed(error: ValueError) -> Option<Self> {
        let (target, struct_index, declared) = match error {
            ValueError::UnexpectedPointerType {
                struct_index,
                declared,
            } => (None, struct_index, declared),
            ValueError::UnexpectedReferenceType {
                guid,
                struct_index,
                declared,
            } => (Some(guid), struct_index, declared),
            _ => return None,
        };
        Some(Self {
            target,
            status: ExportReferenceStatus::UnexpectedTargetType,
            pointer: Some(ReferencePointer {
                structure: Some(struct_index),
                instance: None,
                declared_type: Some(declared),
            }),
        })
    }
}

const fn words(struct_index: u32, instance_index: u32) -> ReferencePointer {
    ReferencePointer {
        structure: Some(struct_index),
        instance: Some(instance_index),
        declared_type: None,
    }
}
