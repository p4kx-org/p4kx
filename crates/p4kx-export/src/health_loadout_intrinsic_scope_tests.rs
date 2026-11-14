// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_extraction::{SourceLocation, SourceUseRole, SourceValue};

use super::{Probe, TestResult, fixture};
use crate::build::original_input::ship_pool_test_input;
use crate::catalog_read::original::health_example_retention::RetainedExampleBody;

#[test]
fn an_intrinsic_missing_file_row_excludes_a_later_root_reuse() -> TestResult {
    ship_pool_test_input::with_input(
        &fixture::database(),
        fixture::entries(None),
        |archive, files, forge, input| {
            fixture::original(archive, forge, None)?;
            let probe = Arc::new(Probe::new(input, None));
            fixture::resolve(forge, files, Arc::clone(&probe), true)?;
            check_operations(&probe)?;

            let ledger = probe.capture.finish()?;
            let diagnostic = ledger
                .field_diagnostics
                .iter()
                .find(|row| row.warning_code.as_deref() == Some("missing_loadout_file"))
                .ok_or("the original missing FileRead diagnostic")?;
            assert_eq!(
                diagnostic.count, 1,
                "the original missing read is recorded once"
            );
            let example = diagnostic
                .examples
                .first()
                .ok_or("the original file example")?;
            let retained = RetainedExampleBody::read(example.body()?.clone(), input)?;
            let texts = retained.texts();
            let field = texts.first().ok_or("the actual file path body")?;
            assert!(
                matches!(field.value(), SourceValue::Present(text) if text.text().as_str() == fixture::CHILD),
                "the original file datum names the exact missing child"
            );
            let contributions = field.contributions().0;
            assert!(contributions.iter().any(|used| used.role() == SourceUseRole::Payload
                && matches!(used.description().location(), SourceLocation::Subresource {namespace, indices, field}
                    if namespace == "xml" && indices == &[u64::from(fixture::INCLUDE_NODE)] && field == "@loadout")),
                "the original same-parse Include supplies the missing path");
            assert!(
                contributions.iter().any(|used| root_selector(
                    used.role(),
                    used.description().location(),
                    0
                )),
                "the genuine first root request selected the parent operation"
            );
            assert!(
                !contributions.iter().any(|used| root_selector(
                    used.role(),
                    used.description().location(),
                    1
                )),
                "a later root cache hit does not contribute to the already executed FileRead"
            );
            Ok(())
        },
    )
}

fn check_operations(probe: &Probe<'_>) -> TestResult {
    let seen = probe.seen.lock().map_err(|_| "actual SDK observations")?;
    assert_eq!(
        seen.reads.len(),
        2,
        "one root read and one missing child read"
    );
    assert_eq!(
        seen.reused, 1,
        "the second root actually reused the first root parse"
    );
    assert_eq!(seen.edges.len(), 1, "one actual direct Include edge");
    let edge = seen.edges.first().ok_or("the actual Include")?;
    assert_eq!(
        edge.parent.projection_outcome(),
        Some(Ok(())),
        "the original parent parsed"
    );
    assert!(
        matches!(
            edge.child.read_outcome(),
            Err(p4kx_assembly::LoadoutFileError::NotFound)
        ),
        "the actual child operation was missing"
    );
    drop(seen);
    Ok(())
}

fn root_selector(role: SourceUseRole, location: &SourceLocation, record: u64) -> bool {
    role == SourceUseRole::Selector
        && matches!(location, SourceLocation::Subresource { namespace, indices, field }
        if namespace == "dataforgeValue" && indices.first() == Some(&record) && field == "$value")
}
