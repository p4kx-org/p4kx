// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Fixed control schemas accept their serializer and reject unknown states.

use serde::Serialize;
use serde_json::{Value, json};

use crate::definition_reference::DefinitionStatus;
use crate::export_diagnostic::ExportDiagnosticSeverity;
use crate::export_diagnostic::ExportDiagnosticStage;
use crate::export_diagnostic::ExportDiagnosticUnit;
use crate::export_diagnostic::ExportSourceRole;
use crate::export_health::CatalogPublicationMode;
use crate::export_health::ExportHealthStatus;
use crate::export_health::ExportSourceMode;
use crate::export_health::ExportStageStatus;
use crate::owned_reference::OwnedReferenceStatus;
use crate::scoped_location_link::PropertyReferenceStatus;

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn check_control<T: Serialize>(name: &str, values: &[T]) -> TestResult {
    let schema = crate::catalog_schema();
    let definition = &schema["$defs"][name];
    assert_eq!(definition["x-p4kx-vocabularyClass"], "control", "{name}");
    let validator = jsonschema::validator_for(definition)?;
    for value in values {
        let serialized = serde_json::to_value(value)?;
        assert!(validator.is_valid(&serialized), "{name}: {serialized}");
        let wrong_case = serialized
            .as_str()
            .expect("a control serializes to a string")
            .to_uppercase();
        assert_ne!(serialized, json!(wrong_case), "{name}: case mutation");
        assert!(
            !validator.is_valid(&json!(wrong_case)),
            "{name} rejects a case-changed control"
        );
    }
    for invalid in [
        json!("futureControlState"),
        json!(42),
        json!(true),
        json!({}),
        json!([]),
        Value::Null,
    ] {
        assert!(!validator.is_valid(&invalid), "{name} rejects {invalid}");
    }
    Ok(())
}

macro_rules! control_test {
    ($name:ident, $control:ident, [$($value:path),+ $(,)?]) => {
        #[test]
        fn $name() -> TestResult {
            check_control(stringify!($control), &[$($value),+])
        }
    };
}

control_test!(
    publication_mode,
    CatalogPublicationMode,
    [
        CatalogPublicationMode::Normal,
        CatalogPublicationMode::DiagnosticPartial,
    ]
);
control_test!(
    export_health,
    ExportHealthStatus,
    [ExportHealthStatus::Complete, ExportHealthStatus::Incomplete,]
);
control_test!(
    export_stage,
    ExportStageStatus,
    [ExportStageStatus::Complete, ExportStageStatus::Incomplete,]
);
control_test!(
    source_mode,
    ExportSourceMode,
    [
        ExportSourceMode::Records,
        ExportSourceMode::RecordInputs,
        ExportSourceMode::ArchiveEntries,
    ]
);
control_test!(
    diagnostic_severity,
    ExportDiagnosticSeverity,
    [
        ExportDiagnosticSeverity::Information,
        ExportDiagnosticSeverity::Warning,
        ExportDiagnosticSeverity::Error,
    ]
);
control_test!(
    diagnostic_stage,
    ExportDiagnosticStage,
    [
        ExportDiagnosticStage::Selection,
        ExportDiagnosticStage::SourceRead,
        ExportDiagnosticStage::Projection,
        ExportDiagnosticStage::DerivedProjection,
        ExportDiagnosticStage::SourceField,
    ]
);
control_test!(
    diagnostic_unit,
    ExportDiagnosticUnit,
    [
        ExportDiagnosticUnit::Records,
        ExportDiagnosticUnit::ArchiveEntries,
        ExportDiagnosticUnit::EntryOperations,
        ExportDiagnosticUnit::Values,
        ExportDiagnosticUnit::Projections,
        ExportDiagnosticUnit::Rows,
        ExportDiagnosticUnit::Occurrences,
    ]
);
control_test!(
    source_role,
    ExportSourceRole,
    [
        ExportSourceRole::Selected,
        ExportSourceRole::DerivedInput,
        ExportSourceRole::Dependency,
        ExportSourceRole::SelectionCandidate,
    ]
);
control_test!(
    definition_status,
    DefinitionStatus,
    [
        DefinitionStatus::Present,
        DefinitionStatus::Dangling,
        DefinitionStatus::UnexpectedTargetType,
    ]
);
control_test!(
    owned_reference_status,
    OwnedReferenceStatus,
    [
        OwnedReferenceStatus::Resolved,
        OwnedReferenceStatus::MissingTarget,
        OwnedReferenceStatus::OutsideOwner,
    ]
);
control_test!(
    property_reference_status,
    PropertyReferenceStatus,
    [
        PropertyReferenceStatus::Resolved,
        PropertyReferenceStatus::MissingTarget,
        PropertyReferenceStatus::OutsideOwner,
        PropertyReferenceStatus::UnsupportedTargetType,
        PropertyReferenceStatus::UnreadableTarget,
    ]
);
