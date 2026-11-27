// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::sdk_full_proof as full_proof;
use p4kx_export::{CatalogError, CatalogPublicationMode, build_catalog};
use serde_json::Value;

use super::fixture;
use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};

#[test]
fn exact_raw_numbers_satisfy_normal_publication_without_hiding_projection_failures() -> TestResult {
    let (_folder, archive) = common::open(&fixture::unique_database(false), DcbOrigin::Plain)?;
    let built = build_catalog(&archive, common::options().with_full_provenance())?;
    assert!(
        built.summary().export_health().is_complete(),
        "a readable exact source arena covers legal nonfinite values"
    );
    let catalog: Value = serde_json::from_slice(built.json())?;
    assert_eq!(
        common::at(&catalog, "/configurations/miningGlobal/rows")?
            .as_array()
            .ok_or("canonical global rows")?
            .len(),
        1,
        "the actual original owner is published once"
    );
    let (_folder, malformed) = common::open(&fixture::unique_database(true), DcbOrigin::Plain)?;
    assert!(
        matches!(
            build_catalog(&malformed, common::options().with_full_provenance()),
            Err(CatalogError::IncompleteExport(_))
        ),
        "normal output refuses the actual Bool-to-number projection failure"
    );
    let mut options = common::options();
    options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    let built = build_catalog(&malformed, options.with_full_provenance())?;
    assert!(
        !built.summary().export_health().is_complete(),
        "retained originals never manufacture complete semantic health"
    );
    let catalog: Value = serde_json::from_slice(built.json())?;
    crate::quality_v3_support::assert_field_error(
        &catalog,
        "absorbableVolumeThreshold",
        "MiningGlobalParams",
    )?;
    Ok(())
}

#[test]
fn additional_refining_original_nan_values_have_independent_coverage_without_typed_fields()
-> TestResult {
    let database = refining_numbers();
    let (_folder, archive) = common::open(&database, DcbOrigin::Plain)?;
    let built = build_catalog(&archive, common::options().with_full_provenance())?;
    assert!(
        built.summary().export_health().is_complete(),
        "all original nonfinite occurrences have genuine emitted raw coverage"
    );
    let catalog: Value = serde_json::from_slice(built.json())?;
    let expected = common::at(&catalog, "/definitions/refiningProcesses/rows/0")?;
    full_proof::open(
        &archive,
        p4kx_export::catalog_queries::CatalogLanguageSelection::Selected(&[]),
        None,
    )?
    .with_refining_processes(
        p4kx_export::catalog_queries::CatalogRecordSelection::RecordIndex(0),
        None,
        |snapshot| -> TestResult {
            let [row] = snapshot.rows() else {
                return Err("one physical original refining owner".into());
            };
            super::walk::visit_refining(row, snapshot.source_inventory(), expected)?;
            let mut found = 0;
            for node in row.fields().values() {
                if let Some(p4kx_export::catalog_queries::SourcePayload::Number {
                    value: p4kx_export::catalog_queries::ResourceNumber::Double(value),
                }) = node.value().value()
                {
                    assert_eq!(
                        value.raw_bits(),
                        0xfff8_0000_0000_1166,
                        "exact Double NaN sign/payload is retained"
                    );
                    found += 1;
                }
            }
            assert_eq!(
                found, 3,
                "one scalar and two equal array slots remain three original occurrences"
            );
            Ok(())
        },
    )??;
    Ok(())
}

fn refining_numbers() -> p4kx_testkit::DataForgeBuilder {
    use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};
    let original = f64::from_bits(0xfff8_0000_0000_1166);
    DataForgeBuilder::new()
        .version(8)
        .structure(
            TestStruct::new("RefiningProcess")
                .field("unprojectedNumber", TestType::Double)
                .array("unprojectedSamples", TestType::Double),
        )
        .record(
            TestRecord::new(
                0xD111,
                "RefiningProcess",
                "RefiningProcess.RawOnly",
                "libs/foundry/records/refiningprocess/raw_only.xml",
            )
            .value("unprojectedNumber", TestValue::Double(original))
            .value(
                "unprojectedSamples",
                TestValue::Array(vec![
                    TestValue::Double(original),
                    TestValue::Double(original),
                ]),
            ),
        )
}
