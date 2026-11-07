// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataForge, Filter};

use crate::Kind;
use crate::export_diagnostic::{ExportDiagnosticCode, ExportDiagnosticSeverity, ExportSourceRole};
use crate::export_ledger::SourceDisposition;

use super::fixtures;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn a_readable_overdeep_blueprint_is_a_projection_failure() -> TestResult {
    let bytes = fixtures::blueprint(33, false);
    let forge = DataForge::open(&bytes)?;
    let read = super::read(&forge, None, None, None)?;
    let source = read
        .ledger
        .sources
        .first()
        .ok_or("source observation missing")?;
    assert_eq!(
        source.kind,
        Some(Kind::Blueprints),
        "the selected source belongs to blueprints"
    );
    assert_eq!(
        source.disposition,
        SourceDisposition::ProjectionFailed,
        "canonical reading succeeded before typed depth rejection"
    );
    assert_eq!(
        read.data.failures.len(),
        1,
        "the public compatibility failure is retained"
    );
    assert_eq!(
        read.ledger.field_inspections.len(),
        1,
        "the canonical content was inspected once"
    );
    Ok(())
}

#[test]
fn canonical_layout_failure_is_not_typed_projection_failure() -> TestResult {
    let bytes = fixtures::broken(
        "CraftingBlueprintRecord",
        "CraftingBlueprintRecord.Synthetic",
        "libs/foundry/records/crafting/blueprints/crafting/synthetic.xml",
    );
    let forge = DataForge::open(&bytes)?;
    let read = super::read(&forge, None, None, None)?;
    let source = read
        .ledger
        .sources
        .first()
        .ok_or("source observation missing")?;
    assert_eq!(
        source.disposition,
        SourceDisposition::ReadFailed,
        "an invalid declared layout fails the canonical read"
    );
    assert!(
        read.ledger.field_inspections.is_empty(),
        "no canonical tree was fabricated for a failed read"
    );
    Ok(())
}

#[test]
fn excluded_blueprints_are_not_read_or_reported_as_omitted() -> TestResult {
    let bytes = fixtures::blueprint(33, false);
    let forge = DataForge::open(&bytes)?;
    let filter = Filter::new("unrelated-selection");
    let read = super::read(&forge, Some(&filter), None, None)?;
    assert!(
        read.data.failures.is_empty(),
        "an excluded overdeep source is not a selected failure"
    );
    assert!(
        read.ledger.sources.is_empty(),
        "no source outcome is invented for an unrequested blueprint"
    );
    let bytes = fixtures::blueprint(33, true);
    let forge = DataForge::open(&bytes)?;
    let read = super::read(&forge, None, None, None)?;
    assert_eq!(
        read.data.skipped_blueprints, 1,
        "the documented leftover remains intentionally skipped"
    );
    assert_eq!(
        read.ledger
            .sources
            .first()
            .ok_or("skip missing")?
            .disposition,
        SourceDisposition::SkippedBeforeRead,
        "the excluded leftover never reaches the canonical reader"
    );
    Ok(())
}

#[test]
fn a_failed_post_read_leftover_keeps_skip_and_dependency_roles_separate() -> TestResult {
    let bytes = fixtures::broken(
        "CraftingGameplayPropertyDef",
        "CraftingGameplayPropertyDef.synthetic_test",
        "libs/foundry/records/crafting/craftedproperties/synthetic_test.xml",
    );
    let forge = DataForge::open(&bytes)?;
    let read = super::read(&forge, None, None, None)?;
    assert!(
        read.ledger
            .sources
            .iter()
            .any(|source| source.role == ExportSourceRole::Selected
                && source.disposition == SourceDisposition::SkippedBeforeRead),
        "the known catalog exclusion remains a source-Record skip"
    );
    assert!(
        read.ledger
            .sources
            .iter()
            .any(|source| source.role == ExportSourceRole::Dependency
                && source.disposition == SourceDisposition::ReadFailed),
        "the actual API dependency-read failure is retained separately"
    );
    assert!(
        !read
            .ledger
            .sources
            .iter()
            .any(|source| source.role == ExportSourceRole::Selected && source.disposition.failed()),
        "an excluded catalog row never becomes an omitted selected Record"
    );
    Ok(())
}

#[test]
fn outside_filter_definition_reads_are_actual_dependencies() -> TestResult {
    let bytes = fixtures::broken(
        "CraftingGameplayPropertyDef",
        "CraftingGameplayPropertyDef.Synthetic",
        "libs/foundry/records/crafting/craftedproperties/synthetic.xml",
    );
    let forge = DataForge::open(&bytes)?;
    let filter = Filter::new("unrelated-selection");
    let read = super::read(&forge, Some(&filter), None, None)?;
    assert!(
        read.data.failures.is_empty(),
        "the established public Filter failure behavior is retained"
    );
    assert_eq!(
        read.ledger
            .sources
            .first()
            .ok_or("dependency missing")?
            .role,
        ExportSourceRole::Dependency,
        "the source was actually read to supply shared definitions"
    );
    Ok(())
}

#[test]
fn error_leaves_are_not_disguised_as_absent_empty_or_dangling_fields() -> TestResult {
    let bytes = fixtures::fields();
    let forge = DataForge::open(&bytes)?;
    let read = super::read(&forge, None, None, None)?;
    assert!(
        read.data.failures.is_empty(),
        "a canonical tree with field failures remains distinct from a failed tree build"
    );
    let source = read.ledger.sources.first().ok_or("global source missing")?;
    assert_eq!(
        source.kind, None,
        "global crafting configuration is not a fabricated catalog kind"
    );
    assert_eq!(
        source.role,
        ExportSourceRole::Dependency,
        "global configuration is an actual dependency"
    );
    let diagnostics = read
        .ledger
        .field_inspections
        .iter()
        .flat_map(|inspection| &inspection.diagnostics)
        .collect::<Vec<_>>();
    for code in [
        ExportDiagnosticCode::OptionalAbsent,
        ExportDiagnosticCode::EmptyValue,
        ExportDiagnosticCode::DanglingReference,
        ExportDiagnosticCode::UnreadableSource,
    ] {
        assert!(
            diagnostics.iter().any(|diagnostic| diagnostic.code == code),
            "the semantic source field state {code:?} is retained"
        );
    }
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic.code
            == ExportDiagnosticCode::UnreadableSource
            && diagnostic.severity == ExportDiagnosticSeverity::Error),
        "an actual value decoding failure remains an error"
    );
    Ok(())
}

#[test]
fn an_explicit_successful_zero_row_projection_is_not_a_failed_or_skipped_source() -> TestResult {
    let bytes = fixtures::no_dismantle();
    let forge = DataForge::open(&bytes)?;
    let read = super::read(&forge, None, None, None)?;
    assert!(
        read.data.dismantle.is_empty(),
        "the known non-dismantle branch emits zero rows"
    );
    let source = read
        .ledger
        .sources
        .first()
        .ok_or("dismantle source missing")?;
    assert_eq!(
        source.kind,
        Some(Kind::DismantleProcesses),
        "the original source kind is retained"
    );
    assert_eq!(
        source.disposition,
        SourceDisposition::Projected,
        "the actual successful zero-row typed branch completed"
    );
    Ok(())
}

#[test]
fn completed_empty_enumerations_keep_all_six_crafting_inventories() -> TestResult {
    let bytes = p4kx_testkit::DataForgeBuilder::new().build();
    let forge = DataForge::open(&bytes)?;
    let read = super::read(&forge, None, None, None)?;
    assert_eq!(
        read.ledger.inventories.len(),
        6,
        "each actual empty selection enumeration completed"
    );
    assert!(
        read.ledger.sources.is_empty(),
        "no Record outcomes are guessed from empty rows"
    );
    Ok(())
}
