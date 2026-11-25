// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::sdk_full_proof as full_proof;
use p4kx_export::catalog_queries::quality::QuantizationResourceSnapshot;
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogRecordSelection, DatasetAvailability, ParsedCatalogQueries,
};
use p4kx_extraction::{InputScope, SourceLocation};
use p4kx_testkit::DataForgeBuilder;
use serde_json::{Value, json};

use crate::health_origin_assertions as proof;
use crate::health_origin_fixture::{Fixture, Input, TestResult};
use crate::quality_v3_fixture_schema as schema;
use crate::quality_v3_fixture_values as values;
use crate::quality_v3_support as support;

#[test]
fn inverse_sdk_matches_same_guid_aliases_and_original_member_evidence() -> TestResult {
    for input in [Input::Plain, Input::Encrypted] {
        let database = values::duplicate_quantizations();
        with_catalog(&database, input, |parsed, catalog| {
            let all = compare(parsed, catalog, CatalogRecordSelection::All)?;
            let guid = support::guid(values::QUANTIZATION).parse()?;
            let by_guid = compare(parsed, catalog, CatalogRecordSelection::Guid(guid))?;
            assert_eq!(
                all, by_guid,
                "nominal selection keeps all three physical definitions"
            );
            assert_eq!(
                all,
                [(0, 0), (1, 2), (2, 0)],
                "only the first eligible alias owns the deduplicated resources"
            );
            let winner = compare(parsed, catalog, CatalogRecordSelection::RecordIndex(1))?;
            assert_eq!(
                winner,
                [(1, 2)],
                "a physical query keeps the winning relation"
            );
            let later = compare(parsed, catalog, CatalogRecordSelection::RecordIndex(2))?;
            assert_eq!(
                later,
                [(2, 0)],
                "the unselected winner still consumes the members"
            );
            assert_empty(parsed)
        })?;
    }
    Ok(())
}

#[test]
fn an_eligible_leftover_still_consumes_the_original_members() -> TestResult {
    let valid = || values::bands(vec![values::band(0, 99, 20)]);
    let database = schema::schema()
        .record(values::quantization(
            values::QUANTIZATION,
            "debug",
            "debug",
            valid(),
        ))
        .record(values::quantization(
            values::QUANTIZATION,
            "regular",
            "regular",
            valid(),
        ))
        .record(values::resource(
            values::RESOURCE_A,
            "fixture_resource",
            vec![values::property(Some(values::QUANTIZATION))],
        ));
    with_catalog(&database, Input::Encrypted, |parsed, catalog| {
        let selected = compare(parsed, catalog, CatalogRecordSelection::RecordIndex(1))?;
        assert_eq!(
            selected,
            [(1, 0)],
            "the earlier leftover keeps its original selection effect"
        );
        Ok(())
    })
}

fn with_catalog(
    database: &DataForgeBuilder,
    input: Input,
    inspect: impl FnOnce(&ParsedCatalogQueries<'_>, &Value) -> TestResult,
) -> TestResult {
    let fixture = Fixture::open(&database.build(), input, Vec::new())?;
    let (_, catalog) = fixture.diagnostic()?;
    let session = full_proof::from_entry(
        &fixture.archive,
        &fixture.entry,
        CatalogLanguageSelection::AllAvailable,
        None,
    )?;
    inspect(&session.parsed(None)?, &catalog)
}

fn compare(
    parsed: &ParsedCatalogQueries<'_>,
    catalog: &Value,
    selection: CatalogRecordSelection,
) -> TestResult<Vec<(u32, usize)>> {
    parsed.with_quality_quantization_resources(selection, None, |snapshot| {
        compare_rows(catalog, &snapshot)
    })?
}

fn compare_rows(
    catalog: &Value,
    snapshot: &QuantizationResourceSnapshot,
) -> TestResult<Vec<(u32, usize)>> {
    let projection = &snapshot.projection;
    let audit = json!({"provenance": projection.source_inventory()});
    let mut membership = Vec::new();
    for row in projection.rows() {
        let index = *row.record_index().value().ok_or("sourced physical index")?;
        let expected = support::row(catalog, support::INVERSE, u64::from(index))?;
        assert_eq!(
            proof::with_origins(serde_json::to_value(row)?, &audit)?,
            proof::with_origins(expected.clone(), catalog)?,
            "the scoped SDK row has the same original evidence and member sequence"
        );
        let members = row.resource_type_guids().value().ok_or("sourced members")?;
        membership.push((index, members.len()));
    }
    assert!(
        projection.coverage().unreadable_bodies.is_empty(),
        "fixture owners are readable"
    );
    assert!(
        snapshot.dependency_coverage.unreadable_bodies.is_empty(),
        "fixture dependencies are readable"
    );
    Ok(membership)
}

fn assert_empty(parsed: &ParsedCatalogQueries<'_>) -> TestResult {
    parsed.with_quality_quantization_resources(
        CatalogRecordSelection::RecordIndex(u32::MAX),
        None,
        |snapshot| {
            let projection = &snapshot.projection;
            assert!(
                matches!(projection.availability(), DatasetAvailability::Available),
                "source availability is independent of the physical selector"
            );
            assert!(
                projection.rows().is_empty(),
                "the unallocated index has no row"
            );
            assert!(
                projection.source_inventory().uses().iter().all(|source| {
                    matches!(source.scope(), InputScope::ArchiveDirectoryMetadata)
                        && matches!(source.location(), SourceLocation::Metadata { .. })
                }),
                "only the already opened input's metadata is needed"
            );
            let dependencies = &snapshot.dependency_coverage;
            assert!(
                dependencies.unreadable_headers.is_empty()
                    && dependencies.unreadable_bodies.is_empty()
                    && dependencies.incomplete_fields.is_empty()
                    && dependencies.skipped_leftovers == 0,
                "no membership dependency is examined without an admitted quantizer"
            );
        },
    )?;
    Ok(())
}
