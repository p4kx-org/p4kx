// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogRecordSelection, SourcePayload, with_contracts,
};
use p4kx_extraction::DataForge;

use super::TestResult;
use crate::commodity_v3_fixture::{self, DcbOrigin};

#[path = "catalog_contract_query_demand_fixture.rs"]
mod fixture;

#[test]
fn contract_sdk_uninterpreted_references_do_not_activate_mission_or_place_bodies() -> TestResult {
    let bytes = fixture::database(fixture::Used::Neither);
    let forge = DataForge::open(&bytes)?;
    for index in [1, 2] {
        assert!(
            forge
                .record(index)
                .ok_or("broken target header")?
                .tree()
                .is_err(),
            "the referenced target is a real unreadable source body"
        );
    }
    let (_folder, archive) = commodity_v3_fixture::open_bytes(bytes, DcbOrigin::Plain)?;
    with_contracts(
        &archive,
        CatalogRecordSelection::RecordIndex(0),
        CatalogLanguageSelection::Selected(&[]),
        None,
        |snapshot| {
            assert_eq!(
                snapshot.rows.len(),
                1,
                "the selected independent contract remains available"
            );
            assert!(
                snapshot.coverage.dependencies.forward_records.is_empty(),
                "unused authored references cannot become semantic dependency candidates"
            );
            assert!(
                snapshot.coverage.dependencies.forward_failures.is_empty()
                    && snapshot.coverage.projection_failures.is_empty(),
                "uninterpreted target bodies cannot create source or projection failures"
            );
            let [source] = snapshot.generators.as_slice() else {
                return Err("one selected original definition".into());
            };
            for name in ["unusedTemplate", "unusedPlace"] {
                let field = source
                    .source()
                    .roots()
                    .iter()
                    .find(|field| field.name().value().copied() == Some(name))
                    .ok_or("the uninterpreted original reference declaration")?;
                let node = source
                    .source()
                    .value(field.value())
                    .ok_or("the original reference value")?;
                assert!(
                    matches!(
                        node.value().value(),
                        Some(SourcePayload::RecordReference { .. })
                    ),
                    "retaining an original reference does not require interpreting its target body"
                );
            }
            Ok::<_, Box<dyn std::error::Error>>(())
        },
    )??;
    Ok(())
}

#[test]
fn contract_sdk_real_typed_uses_report_only_the_actual_failed_dependency() -> TestResult {
    for used in [fixture::Used::Template, fixture::Used::Place] {
        let expected = used.target().ok_or("the actual used target")?;
        let bytes = fixture::database(used);
        let forge = DataForge::open(&bytes)?;
        let original = forge
            .record(expected)
            .ok_or("original target header")?
            .tree()
            .expect_err("the required dependency body is actually unreadable");
        let (_folder, archive) = commodity_v3_fixture::open_bytes(bytes, DcbOrigin::Plain)?;
        with_contracts(
            &archive,
            CatalogRecordSelection::RecordIndex(0),
            CatalogLanguageSelection::Selected(&[]),
            None,
            |snapshot| {
                assert_eq!(
                    snapshot.coverage.dependencies.forward_records,
                    [expected],
                    "only the real canonical use activates a forward candidate"
                );
                let [failure] = snapshot.coverage.dependencies.forward_failures.as_slice() else {
                    return Err("one actual failed body".into());
                };
                assert_eq!(
                    failure.record().index(),
                    expected,
                    "the failure retains its real physical owner"
                );
                assert_eq!(
                    failure.canonical_error(),
                    Some(&original),
                    "the dependency reports its original source error rather than a provisional absence"
                );
                Ok::<_, Box<dyn std::error::Error>>(())
            },
        )??;
    }
    Ok(())
}
