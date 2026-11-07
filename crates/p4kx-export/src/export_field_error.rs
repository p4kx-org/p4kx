// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Guid, ValueError};

use crate::export_diagnostic::{ExportDiagnosticCode, ExportDiagnosticSeverity};
use crate::export_field_collect::FieldCollector;
use crate::export_reference_issue::{ExportReferenceStatus, ExportSourcePointer};

struct Link {
    target: Option<Guid>,
    status: ExportReferenceStatus,
    pointer: Option<ExportSourcePointer>,
}

/// Records decoder failures and preserves unresolved source identities without graph following.
pub(crate) fn record(
    collector: &mut FieldCollector<'_, '_>,
    field: &str,
    source_type: &str,
    error: ValueError,
) {
    if matches!(
        error,
        ValueError::TooDeep { .. } | ValueError::TooManyValues { .. }
    ) {
        collector.limited(field, source_type);
        return;
    }
    let (code, severity, target) = category(error);
    collector.note(field, source_type, code, severity, target);
    if let Some(link) = link(error) {
        collector.unresolved(field, source_type, link.target, link.status, link.pointer);
    }
}

const fn category(
    error: ValueError,
) -> (ExportDiagnosticCode, ExportDiagnosticSeverity, Option<Guid>) {
    match error {
        ValueError::DanglingReference { guid } => (
            ExportDiagnosticCode::DanglingReference,
            ExportDiagnosticSeverity::Information,
            Some(guid),
        ),
        ValueError::OutsideRecord { .. } => (
            ExportDiagnosticCode::OutsideOwner,
            ExportDiagnosticSeverity::Information,
            None,
        ),
        _ => (
            ExportDiagnosticCode::UnreadableSource,
            ExportDiagnosticSeverity::Error,
            None,
        ),
    }
}

const fn link(error: ValueError) -> Option<Link> {
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
            Some(pointer(struct_index, instance_index)),
        ),
        ValueError::InvalidPointer {
            struct_index,
            instance_index,
        } => (
            None,
            ExportReferenceStatus::InvalidPointer,
            Some(pointer(struct_index, instance_index)),
        ),
        ValueError::OwnedTwice {
            struct_index,
            instance_index,
        } => (
            None,
            ExportReferenceStatus::OwnedTwice,
            Some(pointer(struct_index, instance_index)),
        ),
        _ => return typed_link(error),
    };
    Some(Link {
        target,
        status,
        pointer,
    })
}

const fn typed_link(error: ValueError) -> Option<Link> {
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
    Some(Link {
        target,
        status: ExportReferenceStatus::UnexpectedTargetType,
        pointer: Some(ExportSourcePointer {
            struct_index: Some(struct_index),
            instance_index: None,
            declared_type_index: Some(declared),
        }),
    })
}

const fn pointer(struct_index: u32, instance_index: u32) -> ExportSourcePointer {
    ExportSourcePointer {
        struct_index: Some(struct_index),
        instance_index: Some(instance_index),
        declared_type_index: None,
    }
}
