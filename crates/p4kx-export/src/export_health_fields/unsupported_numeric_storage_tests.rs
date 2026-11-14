// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::time::UNIX_EPOCH;

use p4kx_extraction::{Archive, DataForge, DataType, RecordTree, TreeBuilder, Value};
use p4kx_testkit::{ArchiveBuilder, TestEntry};
use serde_json::Value as JsonValue;

use crate::catalog_number_diagnostics;
use crate::catalog_number_health::NumberCaptures;
use crate::catalog_read::original::RecordSubjectKey;
use crate::catalog_row::CatalogRows;
use crate::export_diagnostic::{ExportDiagnosticSeverity, ExportDiagnosticStage, ExportSourceRole};
use crate::export_field_inspection::FieldInspection;
use crate::export_ledger::{SourceDisposition, SourceObservation};
use crate::health_diagnostic_observation::DiagnosticObservation;
use crate::{CatalogError, CatalogOptions, CatalogPublicationMode, ExportDiagnosticCode, Kind};

#[path = "unsupported_numeric_storage_fixture.rs"]
mod fixture;
use fixture::{Case, Expected};

type Checked<T = ()> = Result<T, Box<dyn Error>>;

#[test]
fn unsupported_numeric_storage_keeps_the_finite_projection_failure() -> Checked {
    for case in Case::ALL {
        check_case(case)?;
    }
    Ok(())
}

fn check_case(case: Case) -> Checked {
    let bytes = case.bytes();
    let forge = DataForge::open(&bytes)?;
    let record = forge.records().next().ok_or("resource missing")?;
    let tree = TreeBuilder::new().build(&record)?;
    assert!(
        tree.is_from(&record),
        "the canonical tree retains this actual Record owner"
    );
    check_source(&tree, &case.expected())?;
    let mut ledger = crate::export_field_walk::inspect(
        &tree,
        &record,
        Some(Kind::ResourceTypes),
        ExportSourceRole::Selected,
        None,
    )?;
    let proof = ledger
        .field_inspections
        .pop()
        .ok_or("field proof missing")?;
    check_requirement(&proof, &case.expected())?;
    let source = SourceObservation::new(
        Kind::ResourceTypes,
        &record,
        ExportSourceRole::Selected,
        SourceDisposition::Projected,
    );
    check_final(proof, source, &case.expected())
}

fn check_final(proof: FieldInspection, source: SourceObservation, expected: &Expected) -> Checked {
    let captured = NumberCaptures::new(None, None);
    let mut coverage = captured.for_record(source.record_index, source.kind);
    let diagnostic = catalog_number_diagnostics::uncovered(&proof, &source, &mut coverage)?
        .ok_or("unsupported original token remains uncovered")?;
    check_diagnostic(&diagnostic, &source, expected)?;
    let rows = CatalogRows::default();
    let physical = source.record_index;
    let final_fields =
        crate::export_health_fields::resolve(vec![proof], &[source], Some(&rows), &[])?;
    assert!(
        final_fields
            .failed
            .iter()
            .any(|failed| failed.key().index() == physical
                && failed.code == ExportDiagnosticCode::UnsupportedValue),
        "actual final resolution retains the blocking owner"
    );
    Ok(())
}

fn check_diagnostic(
    diagnostic: &DiagnosticObservation,
    source: &SourceObservation,
    expected: &Expected,
) -> Checked {
    assert_eq!(
        diagnostic.code,
        ExportDiagnosticCode::UnsupportedValue,
        "same final blocking classification"
    );
    assert_eq!(
        diagnostic.stage,
        ExportDiagnosticStage::Projection,
        "the unresolved numeric projection is blocking"
    );
    assert_eq!(
        diagnostic.severity,
        ExportDiagnosticSeverity::Error,
        "unknown contracts never become informational"
    );
    assert_eq!(
        diagnostic.count, 1,
        "exactly one original nonfinite token is uncovered"
    );
    let example = diagnostic
        .examples
        .first()
        .ok_or("actual owner/path example")?;
    assert_eq!(
        example.subject().key().map(RecordSubjectKey::index),
        Some(source.record_index),
        "actual physical source index"
    );
    assert_eq!(
        example.subject().key().map(|key| key.guid().to_string()),
        Some(source.guid.to_string()),
        "actual original GUID"
    );
    assert_eq!(
        example.field.as_deref(),
        Some(expected.path),
        "exact original scalar path"
    );
    Ok(())
}

fn check_source(tree: &RecordTree<'_>, expected: &Expected) -> Checked {
    assert_eq!(tree.record_index(), 0, "original physical fixture Record");
    assert_eq!(
        tree.error_count(),
        0,
        "all three original shapes are canonically readable"
    );
    assert_eq!(tree.fields().len(), 1, "the original density declaration");
    let root = tree.fields().first().ok_or("original root field")?;
    assert_eq!(
        root.storage().data_type(),
        DataType::StrongPointer,
        "actual root pointer declaration"
    );
    let Value::Struct(owner) = root.value() else {
        return Err("readable original owner".into());
    };
    assert_eq!(
        owner.type_name(),
        expected.owner,
        "actual source owner class"
    );
    assert_eq!(owner.fields().len(), 1, "one original numeric declaration");
    let field = owner.fields().first().ok_or("actual numeric declaration")?;
    assert_eq!(
        field.name(),
        expected.field,
        "original source property name"
    );
    assert_eq!(
        field.storage().data_type(),
        expected.data_type,
        "original numeric width"
    );
    assert_eq!(
        field.storage().is_array(),
        expected.is_array,
        "original cardinality declaration"
    );
    let value = if expected.is_array {
        let Value::Array(values) = field.value() else {
            return Err("original numeric array".into());
        };
        assert_eq!(values.len(), 1, "one actual numeric array occurrence");
        values.first().ok_or("original array element")?
    } else {
        field.value()
    };
    assert_eq!(
        fixture::token(value),
        Some(expected.token),
        "exact original storage token precedes all projection assertions"
    );
    Ok(())
}

fn check_requirement(proof: &FieldInspection, expected: &Expected) -> Checked {
    assert!(proof.complete, "the readable source inventory is complete");
    assert_eq!(
        proof.number_requirements.len(),
        1,
        "every original nonfinite is staged once independently of kind"
    );
    let required = proof
        .number_requirements
        .first()
        .ok_or("actual numeric requirement")?;
    assert_eq!(
        required.record_index, proof.record_index,
        "same actual physical source"
    );
    assert_eq!(
        required.original_ordinal, expected.ordinal,
        "actual DFS ordinal including the array container"
    );
    assert_eq!(
        required.source_type, expected.owner,
        "actual source owner type"
    );
    assert_eq!(required.field, expected.path, "exact original scalar path");
    assert_eq!(
        required.token, expected.token,
        "exact original float width/bits"
    );
    assert_eq!(
        required.contract, None,
        "unsupported named shape does not acquire a raw contract from kind"
    );
    Ok(())
}

#[test]
fn unsupported_original_shapes_still_refuse_normal_and_remain_in_diagnostics() -> Checked {
    for case in Case::ALL {
        check_publication(case)?;
    }
    Ok(())
}

fn check_publication(case: Case) -> Checked {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("unsupported-number.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored(r"Data\Game2.dcb", case.bytes()))
        .entry(TestEntry::stored(
            r"Data\Libs\CosmoTools\LayerRecovery\LayerBackups.xml",
            b"<LayerBackups/>",
        ))
        .write_to(&path)?;
    let archive = Archive::open(path)?;
    let outcome = crate::build_catalog(&archive, &CatalogOptions::new(UNIX_EPOCH));
    assert!(
        matches!(&outcome, Err(CatalogError::IncompleteExport(_))),
        "unsupported Resource shape {} owned by {} still blocks Normal publication: {:?}",
        case.expected().path,
        case.expected().owner,
        outcome.as_ref().map(|_| "catalog")
    );
    let mut options = CatalogOptions::new(UNIX_EPOCH);
    options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    let built = crate::build_catalog(&archive, &options)?;
    let catalog: JsonValue = serde_json::from_slice(built.json())?;
    assert_eq!(
        catalog
            .pointer("/exportHealth/status")
            .and_then(JsonValue::as_str),
        Some("incomplete"),
        "diagnostic mode does not turn unsupported storage complete"
    );
    let expected = case.expected();
    super::assert_diagnostic_field(&catalog, expected.path)?;
    Ok(())
}

#[test]
fn original_double_storage_keeps_its_exact_public_refusal_and_diagnostic() -> Checked {
    check_case(Case::Double)?;
    check_publication(Case::Double)
}

#[test]
fn original_array_storage_keeps_its_exact_public_refusal_and_diagnostic() -> Checked {
    check_case(Case::Array)?;
    check_publication(Case::Array)
}

#[test]
fn original_unrecognized_owner_keeps_its_exact_public_refusal_and_diagnostic() -> Checked {
    check_case(Case::Owner)?;
    check_publication(Case::Owner)
}
