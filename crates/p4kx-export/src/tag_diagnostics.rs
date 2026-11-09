// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Guid, Record, TagProjectionFailure, TagRelationIssue};

use crate::Kind;
use crate::export_diagnostic::{
    ExportDiagnostic, ExportDiagnosticCode, ExportDiagnosticExample, ExportDiagnosticSeverity,
    ExportDiagnosticStage, ExportDiagnosticUnit, ExportSourceRole,
};
use crate::export_ledger::{DerivedFailure, ExportLedger, SourceDiagnosis};

pub(super) fn cut(ledger: &mut ExportLedger, record: &Record<'_>, reason: TagProjectionFailure) {
    ledger.derived_failure(DerivedFailure {
        affected_kind: Kind::Tags,
        output_kind: Kind::Tags,
        record_index: record.index(),
        role: ExportSourceRole::Selected,
        guid: record.guid(),
        record_path: record.record_path().ok().map(str::to_owned),
        diagnosis: SourceDiagnosis {
            code: Some(ExportDiagnosticCode::DerivedProjectionFailure),
            field: Some("tagPath".to_owned()),
            source_type: Some("Tag".to_owned()),
            target_guid: None,
        },
    });
    let label = match reason {
        TagProjectionFailure::Depth => "tag_path_depth_limit",
        TagProjectionFailure::NameBytes => "tag_path_name_bytes_limit",
        TagProjectionFailure::PathBytes => "tag_path_bytes_limit",
        TagProjectionFailure::TextBytes => "tag_path_text_bytes_limit",
        TagProjectionFailure::NodeCount => "tag_path_node_count_limit",
        TagProjectionFailure::AncestorDiscarded => "tag_path_ancestor_discarded",
    };
    issue(
        ledger,
        record,
        "tagPath",
        None,
        label,
        ExportDiagnosticSeverity::Warning,
    );
}

pub(super) fn relation(
    ledger: &mut ExportLedger,
    record: &Record<'_>,
    field: &str,
    target: Option<Guid>,
    reason: TagRelationIssue,
) {
    let label = match reason {
        TagRelationIssue::Dangling => "tag_relation_dangling",
        TagRelationIssue::Repeated => "tag_relation_repeated",
        TagRelationIssue::MalformedValue => "tag_relation_malformed",
    };
    issue(
        ledger,
        record,
        field,
        target,
        label,
        ExportDiagnosticSeverity::Information,
    );
}

pub(super) fn duplicate(ledger: &mut ExportLedger, record: &Record<'_>) {
    issue(
        ledger,
        record,
        "$guid",
        Some(record.guid()),
        "tag_duplicate_guid",
        ExportDiagnosticSeverity::Information,
    );
}

fn issue(
    ledger: &mut ExportLedger,
    record: &Record<'_>,
    field: &str,
    target: Option<Guid>,
    label: &str,
    severity: ExportDiagnosticSeverity,
) {
    ledger.field_diagnostic(ExportDiagnostic {
        kind: Some(Kind::Tags),
        stage: ExportDiagnosticStage::DerivedProjection,
        code: ExportDiagnosticCode::SourceWarning,
        severity,
        source_role: Some(
            if record.type_name().is_ok_and(|name| name == "TagDatabase") {
                ExportSourceRole::Dependency
            } else {
                ExportSourceRole::Selected
            },
        ),
        warning_code: Some(label.to_owned()),
        count: 1,
        count_unit: ExportDiagnosticUnit::Occurrences,
        examples: vec![ExportDiagnosticExample {
            record_index: Some(record.index()),
            record_guid: Some(record.guid().to_string()),
            record_path: record.record_path().ok().map(str::to_owned),
            field: Some(field.to_owned()),
            source_type: record.type_name().ok().map(str::to_owned),
            target_guid: target.map(|guid| guid.to_string()),
        }],
    });
}
