// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{CatalogError, CatalogFailureView, CatalogSummary, build_catalog_from};
use p4kx_extraction::{DataForgeError, SourceInventory, SourcedValue};

use crate::health_origin_assertions as check;
use crate::health_origin_fixture::{self as fixture, Fixture, Input, TestResult};

#[test]
fn captured_failure_views_resolve_original_headers_and_clone_shared_storage() -> TestResult {
    for input in [Input::Plain, Input::Encrypted, Input::Standalone] {
        let source = Fixture::open(&fixture::broken_lists(1, false), input, Vec::new())?;
        let reason = original(&source)?;
        let (built, catalog) = source.diagnostic()?;
        let summary = built.summary();
        retained(summary, &catalog, input, &reason)?;
        let cloned = summary.clone();
        assert!(
            std::ptr::eq(
                summary.export_health().metadata().texts().as_ptr(),
                cloned.export_health().metadata().texts().as_ptr(),
            ),
            "the public Clone route shares this report's immutable text allocation",
        );
        assert_eq!(
            summary, &cloned,
            "visible state, origins and metadata participate in equality"
        );
        check::closure(&catalog)?;
    }
    Ok(())
}

#[test]
fn normal_refusal_keeps_actual_failure_cells_closed_in_its_owned_inventory() -> TestResult {
    let source = Fixture::open(
        &fixture::broken_lists(1, false),
        Input::Encrypted,
        Vec::new(),
    )?;
    let reason = original(&source)?;
    let error = build_catalog_from(&source.archive, &source.entry, &fixture::options())
        .expect_err("normal publication refuses this actual source failure");
    let inventory = error
        .source_inventory()
        .ok_or("actual refusal's immutable inventory")?;
    let CatalogError::IncompleteExport(refusal) = &error else {
        return Err("expected the actual normal-publication refusal".into());
    };
    let health = refusal.health();
    let failure = health
        .failures()
        .first()
        .ok_or("actual retained human failure")?;
    assert_eq!(
        failure.guid(),
        fixture::guid(fixture::LIST),
        "original GUID"
    );
    assert_eq!(
        failure.reason(),
        reason,
        "the actual canonical reason and source chain are retained"
    );
    let details = failure.details();
    for origin in [
        details.guid().origin(),
        details.record_index().origin(),
        details.record_path().origin(),
        details.source_type().origin(),
    ] {
        assert_eq!(
            origin.contains_encrypted_source(),
            Some(true),
            "actual encrypted header"
        );
        assert!(
            origin.source_set_complete(),
            "the body failure does not cut readable headers"
        );
        closed(inventory, origin.source_uses_refs())?;
    }
    assert_eq!(
        details.reason().origin().contains_encrypted_source(),
        Some(true),
        "actual known encrypted formatter inputs survive incomplete coordinate capture",
    );
    assert!(
        !details.reason().origin().source_set_complete(),
        "the derived layout size is unlocalized"
    );
    closed(inventory, details.reason().origin().source_uses_refs())?;
    Ok(())
}

fn original(source: &Fixture) -> TestResult<String> {
    let forge = source.entry.open()?;
    let record = forge.record(0).ok_or("actual original failed Record")?;
    assert_eq!(
        record.stored_file_name()?,
        "synthetic/health/List0.xml",
        "stored filename"
    );
    assert_eq!(record.type_name()?, fixture::LIST_TYPE, "original type");
    assert_eq!(
        record.guid().to_string(),
        fixture::guid(fixture::LIST),
        "original GUID"
    );
    let error = record
        .tree()
        .err()
        .ok_or("the actual original read fails")?;
    assert!(
        matches!(
            error,
            DataForgeError::StructLayout {
                struct_index: 0,
                declared: 9,
                fields: 8
            }
        ),
        "the actual canonical decoder fails before any product assertion",
    );
    Ok(error.to_string())
}

fn retained(
    summary: &CatalogSummary,
    catalog: &serde_json::Value,
    input: Input,
    reason: &str,
) -> TestResult {
    assert_eq!(
        summary.failures().len(),
        1,
        "the existing human-failure cardinality is preserved"
    );
    let failure = summary
        .failures()
        .first()
        .ok_or("actual borrowed failure")?;
    human(failure, reason);
    let details = failure.details();
    assert_eq!(
        details.record_index().value(),
        Some(&0),
        "original physical position"
    );
    let source_type = details.source_type_text();
    assert_eq!(
        source_type.value(),
        Some(fixture::LIST_TYPE),
        "actual open type label"
    );
    let kind = if matches!(input, Input::Standalone) {
        "file"
    } else {
        "archiveEntry"
    };
    header(catalog, details.guid(), input, "guid", "payload", kind)?;
    header(
        catalog,
        details.record_index(),
        input,
        "recordIndex",
        "selector",
        kind,
    )?;
    header(
        catalog,
        details.record_path(),
        input,
        "fileName",
        "conversion",
        kind,
    )?;
    header(
        catalog,
        details.source_type(),
        input,
        "sourceType",
        "selector",
        kind,
    )
}

fn header<T: serde::Serialize>(
    catalog: &serde_json::Value,
    cell: &SourcedValue<T>,
    input: Input,
    field: &str,
    role: &str,
    kind: &str,
) -> TestResult {
    assert_eq!(
        cell.origin().contains_encrypted_source(),
        input.marker(),
        "original field domain"
    );
    assert!(
        cell.origin().source_set_complete(),
        "readable original header proof is complete"
    );
    check::header(catalog, &serde_json::to_value(cell)?, 0, field, role, kind)
}

fn closed(inventory: &SourceInventory, refs: &[p4kx_extraction::SourceUseRef]) -> TestResult {
    assert!(
        !refs.is_empty(),
        "this actual source-derived cell has a nonvacuous contribution"
    );
    for reference in refs {
        let used = inventory
            .uses()
            .iter()
            .find(|used| used.id() == reference)
            .ok_or("this report's own contributing use")?;
        assert!(
            inventory
                .sources()
                .iter()
                .any(|source| source.id() == used.source()),
            "the use resolves to the same report's retained physical source",
        );
    }
    Ok(())
}
#[test]
fn equal_storage_indices_cannot_cross_pair_independent_report_occurrences() -> TestResult {
    let plain = distinct_report("synthetic/report-a.xml", Input::Plain)?;
    let encrypted = distinct_report("synthetic/report-b.xml", Input::Encrypted)?;
    let plain_summary = plain.summary();
    let encrypted_summary = encrypted.summary();
    let plain_failure = plain_summary
        .failures()
        .first()
        .ok_or("actual plain report failure")?;
    let encrypted_failure = encrypted_summary
        .failures()
        .first()
        .ok_or("actual encrypted report failure")?;
    assert_eq!(
        plain_failure.details().record_path().value(),
        encrypted_failure.details().record_path().value(),
        "the two real reports deliberately collide at a document-local atom index",
    );
    paired_path(plain_failure, "synthetic/report-a", Some(false));
    paired_path(encrypted_failure, "synthetic/report-b", Some(true));
    let copied = encrypted_summary.clone();
    let copied_failure = copied
        .failures()
        .first()
        .ok_or("the owning clone's failure")?;
    paired_path(copied_failure, "synthetic/report-b", Some(true));
    assert!(
        std::ptr::eq(
            encrypted_failure.record_path().as_ptr(),
            copied_failure.record_path().as_ptr()
        ),
        "a whole owning clone shares the exact immutable text allocation",
    );
    assert_eq!(
        copied, *encrypted_summary,
        "visible origins and text values remain equal"
    );
    Ok(())
}

fn distinct_report(file: &str, input: Input) -> TestResult<p4kx_export::BuiltCatalog> {
    let source = distinct_source(file, input)?;
    let (built, catalog) = source.diagnostic()?;
    check::closure(&catalog)?;
    Ok(built)
}

fn distinct_source(file: &str, input: Input) -> TestResult<Fixture> {
    let source = Fixture::open(&distinct_database(file), input, Vec::new())?;
    {
        let forge = source.entry.open()?;
        let record = forge
            .record(0)
            .ok_or("the actual original physical Record")?;
        assert_eq!(
            record.stored_file_name()?,
            file,
            "distinct original header before export"
        );
        assert_eq!(
            record.index(),
            0,
            "same actual physical index in each independent original"
        );
        assert_eq!(
            record.guid().to_string(),
            fixture::guid(fixture::LIST),
            "same actual GUID"
        );
        assert!(
            matches!(
                record.tree(),
                Err(DataForgeError::StructLayout {
                    struct_index: 0,
                    declared: 9,
                    fields: 8
                }),
            ),
            "the original canonical body independently fails before producer assertions"
        );
    }
    Ok(source)
}

fn distinct_database(file: &str) -> Vec<u8> {
    use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};

    DataForgeBuilder::new()
        .version(8)
        .structure(
            TestStruct::new(fixture::LIST_TYPE)
                .array(
                    "categories",
                    TestType::ReferenceTo("BlueprintCategoryRecord".to_owned()),
                )
                .declared_size(9),
        )
        .structure(TestStruct::new("BlueprintCategoryRecord"))
        .record(
            TestRecord::new(fixture::LIST, fixture::LIST_TYPE, "List.source", file)
                .value("categories", TestValue::Array(Vec::new())),
        )
        .build()
}

fn paired_path(failure: CatalogFailureView<'_>, expected: &str, marker: Option<bool>) {
    let details = failure.details();
    let text = details.record_path_text();
    assert_eq!(
        failure.record_path(),
        expected,
        "the human view retains its own original text"
    );
    assert_eq!(
        text.value(),
        Some(expected),
        "semantic lookup needs no externally supplied pool"
    );
    assert_eq!(
        text.origin(),
        details.record_path().origin(),
        "the cell's own exact contribution set"
    );
    assert_eq!(
        text.origin().contains_encrypted_source(),
        marker,
        "no foreign report backtaint"
    );
    assert!(
        text.origin().source_set_complete(),
        "the original header is readable"
    );
}

fn human(failure: CatalogFailureView<'_>, reason: &str) {
    assert_eq!(
        failure.guid(),
        fixture::guid(fixture::LIST),
        "original GUID text"
    );
    assert_eq!(
        failure.record_path(),
        "synthetic/health/List0",
        "actual normalized path"
    );
    assert_eq!(
        failure.reason(),
        reason,
        "the original human reason is preserved"
    );
}

#[path = "health_refusal_pairing.rs"]
mod refusal_pairing;
