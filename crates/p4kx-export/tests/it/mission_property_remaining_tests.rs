// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{
    BuiltCatalog, CatalogError, Compression, ExportDiagnosticExample, ExportDiagnosticStage,
    ExportHealthStatus, Kind,
};
use serde_json::Value;

use crate::health_origin_fixture::{self, Fixture, Input, TestResult};
use crate::mission_property_remaining_fixture::{self as fixture, Case};
use crate::mission_property_remaining_origins;
use crate::mission_property_remaining_source::{self as source, Original};
use crate::mission_property_sets_fixture as sets;

struct Checked {
    input: Fixture,
    original: Original,
    kind: Input,
    case: Case,
}

#[test]
fn remaining_owned_property_guards_retain_their_actual_original_body() -> TestResult {
    run(&[
        Case::Investigation,
        Case::Name,
        Case::Clue,
        Case::Option,
        Case::Race,
    ])
}

#[test]
fn actual_context_conditions_keep_the_original_cause_and_guard_receipt() -> TestResult {
    run(&[Case::Organization, Case::MissionItem, Case::Spawn])
}

fn run(cases: &[Case]) -> TestResult {
    let mut checked = Vec::new();
    for case in cases {
        for kind in [Input::Plain, Input::Encrypted, Input::Standalone] {
            let input = fixture::open(*case, kind)?;
            let original = source::check(&input, *case)?;
            checked.push(Checked {
                input,
                original,
                kind,
                case: *case,
            });
        }
    }
    for actual in checked {
        let (built, document) = actual.input.diagnostic()?;
        check(&built, &document, &actual)?;
        refusal(&actual.input);
    }
    Ok(())
}

fn check(built: &BuiltCatalog, document: &Value, actual: &Checked) -> TestResult {
    let health = built.summary().export_health();
    assert_eq!(
        health.status,
        ExportHealthStatus::Incomplete,
        "required original failure remains blocking"
    );
    assert_eq!(
        built.summary().failures().len(),
        1,
        "one physical failed Generator"
    );
    let matching: Vec<_> = health
        .diagnostics()
        .iter()
        .filter(|diagnostic| {
            diagnostic.kind == Some(Kind::Contracts)
                && diagnostic.stage == ExportDiagnosticStage::Projection
                && diagnostic.code == actual.case.code()
        })
        .collect();
    assert_eq!(
        matching.len(),
        1,
        "the exact actual typed guard cause is retained"
    );
    let diagnostic = matching
        .first()
        .ok_or("actual remaining guard diagnostic")?;
    assert_eq!(diagnostic.count, 1, "one original selected Record");
    assert_eq!(
        diagnostic.examples().len(),
        1,
        "the same first guard controls the transaction"
    );
    let example = diagnostic
        .examples()
        .first()
        .ok_or("actual remaining guard example")?;
    body(example, actual);
    mission_property_remaining_origins::check(document, example, &actual.original)?;
    assert!(
        sets::rows(document)?.is_empty(),
        "failed Generator emits no partial property sets"
    );
    assert!(
        crate::catalog_contracts::v3::rows(document)?.is_empty(),
        "failed Generator emits no partial contracts"
    );
    Ok(())
}

fn body(example: &ExportDiagnosticExample, actual: &Checked) {
    assert_eq!(
        example.record_index().value(),
        Some(&0),
        "the same actual physical owner"
    );
    assert_eq!(
        example.field_text().value(),
        Some(actual.case.field()),
        "exact existing field/operation text"
    );
    assert_eq!(
        example.source_type_text().value(),
        Some(actual.case.source_type()),
        "actual concrete source type"
    );
    assert_eq!(
        example
            .source_type_text()
            .origin()
            .contains_encrypted_source(),
        actual.kind.marker(),
        "actual type origin"
    );
    assert!(
        example.source_type_text().origin().source_set_complete(),
        "complete original type recipe"
    );
    let marker = if actual.case.stored_field().is_some() {
        actual.kind.marker()
    } else {
        Some(false)
    };
    assert_eq!(
        example.field_text().origin().contains_encrypted_source(),
        marker,
        "own field or independent control origin"
    );
    assert!(
        example.field_text().origin().source_set_complete(),
        "complete original field/control recipe"
    );
    assert!(
        example.target_guid().value().is_none(),
        "no fabricated target from failed values"
    );
}

fn refusal(input: &Fixture) {
    let options = health_origin_fixture::options();
    assert!(
        matches!(
            p4kx_export::build_catalog_from(&input.archive, &input.entry, &options),
            Err(CatalogError::IncompleteExport(_))
        ),
        "the same required original failure refuses Normal publication"
    );
    for compression in [Compression::None, Compression::ZSTD] {
        let mut created = false;
        let result = p4kx_export::write_catalog_from(
            &input.archive,
            &input.entry,
            &options,
            compression,
            || {
                created = true;
                Ok(Vec::new())
            },
        );
        assert!(
            matches!(result, Err(CatalogError::IncompleteExport(_))),
            "both writers retain the same refusal"
        );
        assert!(
            !created,
            "no destination or complete-looking prefix is created"
        );
    }
}
