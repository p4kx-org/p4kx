// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceLocation, SourceUse, SourceUseRole};

use crate::build::original_input::OriginalCatalogInput;
use crate::build::original_input::test_support::TestResult;
use crate::catalog_read::original::health_example_retention::RetainedExampleBody;
use crate::export_ledger::ExportLedger;

/// Checks bodies after the real `ShipPool` probe proves one read and one reuse.
pub(super) fn assert_cached_requests(
    first: &ExportLedger,
    second: &ExportLedger,
    input: &OriginalCatalogInput<'_, '_>,
) -> TestResult {
    for (index, ledger) in [(0, first), (1, second)] {
        let diagnostic = ledger
            .field_diagnostics
            .iter()
            .find(|row| row.warning_code.as_deref() == Some("xml_loadout"))
            .ok_or("the genuine cached missing-request diagnostic")?;
        assert_eq!(
            diagnostic.count, 1,
            "each actual sequential caller contributes once"
        );
        assert_eq!(
            diagnostic.examples.len(),
            1,
            "one physical sequential caller example"
        );
        let example = diagnostic
            .examples
            .first()
            .ok_or("the genuine sequential example")?;
        let retained = RetainedExampleBody::read(example.body()?.clone(), input)?;
        let fields = retained.texts();
        let field = fields.first().ok_or("the actual retained request")?;
        let used = field.contributions().0;
        assert_cached_request_uses(used, index)?;
        let kind = fields.get(1).ok_or("the actual root Header type")?;
        assert!(
            kind.contributions()
                .0
                .iter()
                .all(|used| used.role() == SourceUseRole::Selector
                    && at(
                        used.description().location(),
                        "dataforgeRecordHeader",
                        index,
                        "sourceType"
                    )),
            "each actual sequential caller keeps its own Header-type coordinate"
        );
        assert_eq!(
            kind.contributions().0.len(),
            1,
            "one genuine root type Header input"
        );
    }
    Ok(())
}

fn assert_cached_request_uses(used: &[SourceUse], index: u64) -> TestResult {
    let mut payloads = used
        .iter()
        .filter(|used| used.role() == SourceUseRole::Payload);
    let payload = payloads.next().ok_or("the original request Payload")?;
    assert!(
        payloads.next().is_none(),
        "one actual original request Payload"
    );
    assert!(
        at(
            payload.description().location(),
            "dataforgeValue",
            0,
            "$value"
        ),
        "the actual reused error keeps the first original request"
    );
    if index == 0 {
        assert!(
            !used
                .iter()
                .any(|used| used.role() == SourceUseRole::Selector
                    && at(used.description().location(), "dataforgeValue", 1, "$value")),
            "a later independent caller cannot contribute to the already resolved first request field"
        );
    }
    if index == 1 {
        assert!(
            used.iter()
                .any(|used| used.role() == SourceUseRole::Selector
                    && at(used.description().location(), "dataforgeValue", 1, "$value")),
            "the counted genuine second caller is a separate reuse Selector"
        );
    }
    Ok(())
}

fn at(
    location: &SourceLocation,
    expected_namespace: &str,
    record: u64,
    expected_field: &str,
) -> bool {
    matches!(location, SourceLocation::Subresource { namespace, indices, field }
        if namespace == expected_namespace && indices.first() == Some(&record) && field == expected_field)
}

pub(super) fn assert_third_origins(retained: &RetainedExampleBody) -> TestResult {
    let fields = retained.texts();
    let field = fields.first().ok_or("the actual third request field")?;
    let used = field.contributions().0;
    let mut payloads = used
        .iter()
        .filter(|used| used.role() == SourceUseRole::Payload);
    let payload = payloads
        .next()
        .ok_or("the genuine original first request Payload")?;
    assert!(
        payloads.next().is_none(),
        "there is exactly one original Payload"
    );
    assert!(
        at(
            payload.description().location(),
            "dataforgeValue",
            0,
            "$value"
        ),
        "the third use retains the first genuine cached request's payload"
    );
    assert!(
        used.iter()
            .any(|used| used.role() == SourceUseRole::Selector
                && at(used.description().location(), "dataforgeValue", 2, "$value")),
        "the actual current third caller supplies its own selector"
    );
    let kind = fields.get(1).ok_or("the actual third Header type")?;
    assert_eq!(
        kind.contributions().0.len(),
        1,
        "one actual third sourceType Header selector"
    );
    assert!(
        kind.contributions()
            .0
            .iter()
            .all(|used| used.role() == SourceUseRole::Selector
                && at(
                    used.description().location(),
                    "dataforgeRecordHeader",
                    2,
                    "sourceType"
                )),
        "the actual third diagnostic keeps its own Header"
    );
    assert!(
        !used
            .iter()
            .any(|used| used.role() == SourceUseRole::Selector
                && at(used.description().location(), "dataforgeValue", 1, "$value")),
        "the independent middle caller does not contribute to the third request field"
    );
    Ok(())
}
