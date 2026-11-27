// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::{
    CatalogRecordSelection, DefinitionStatus, with_mineable_elements,
};
use p4kx_extraction::{Archive, Guid};

use super::mining_resource_query_fixture as fixture;
use crate::commodity_v3_fixture::{self as archive_fixture, DcbOrigin, TestResult};

#[test]
fn mineable_element_guid_duplicates_keep_individually_selectable_physical_records() -> TestResult {
    let database = fixture::database().record(fixture::duplicate_element());
    let (_folder, archive) = archive_fixture::open(&database, DcbOrigin::Plain)?;
    let expected = guid(fixture::ELEMENT_ALPHA)?;
    let indices = with_mineable_elements(
        &archive,
        CatalogRecordSelection::Guid(expected),
        None,
        |snapshot| -> TestResult<Vec<u32>> {
            assert_eq!(
                snapshot.rows().len(),
                2,
                "Both physical declarations survive GUID selection"
            );
            snapshot
                .rows()
                .iter()
                .map(|row| {
                    assert_eq!(
                        row.source.guid.value(),
                        Some(&expected),
                        "The authored owner GUID remains unchanged"
                    );
                    row.source
                        .record_index
                        .value()
                        .copied()
                        .ok_or_else(|| "selected duplicate has no physical index".into())
                })
                .collect()
        },
    )??;
    assert!(
        indices.windows(2).all(|pair| pair[0] < pair[1]),
        "Equal GUIDs use physical Record order"
    );
    verify_physical_owners(&archive, expected, indices)?;
    Ok(())
}

fn verify_physical_owners(archive: &Archive, expected: Guid, indices: Vec<u32>) -> TestResult {
    for index in indices {
        with_mineable_elements(
            archive,
            CatalogRecordSelection::RecordIndex(index),
            None,
            |snapshot| {
                assert_eq!(
                    snapshot.rows().len(),
                    1,
                    "Physical selection returns one owner"
                );
                for row in snapshot.rows() {
                    assert_eq!(
                        row.source.guid.value(),
                        Some(&expected),
                        "Physical selection retains the owner GUID"
                    );
                    assert_eq!(
                        row.source.record_index.value(),
                        Some(&index),
                        "Physical selection retains its exact index"
                    );
                }
            },
        )?;
    }
    Ok(())
}

#[test]
fn mineable_element_retains_dangling_and_unexpected_resource_target_identities() -> TestResult {
    let database = fixture::database().record(fixture::unexpected_resource_element());
    let (_folder, archive) = archive_fixture::open(&database, DcbOrigin::Plain)?;
    for (owner, target, dangling) in [
        (
            fixture::ELEMENT_DANGLING_RESOURCE,
            fixture::MISSING_RESOURCE,
            true,
        ),
        (
            fixture::ELEMENT_UNEXPECTED_RESOURCE,
            crate::resource_fixture::CONTAINER,
            false,
        ),
    ] {
        let expected_owner = guid(owner)?;
        let expected_target = guid(target)?;
        with_mineable_elements(
            &archive,
            CatalogRecordSelection::Guid(expected_owner),
            None,
            |snapshot| -> TestResult {
                assert_eq!(snapshot.rows().len(), 1, "One exact owner is selected");
                let row = snapshot
                    .rows()
                    .first()
                    .ok_or("selected element is missing")?;
                assert_eq!(
                    row.source.guid.value(),
                    Some(&expected_owner),
                    "The selected owner retains its GUID"
                );
                let reference = row
                    .resource_type
                    .value()
                    .ok_or("authored resource reference was discarded")?;
                assert_eq!(
                    reference.guid().value(),
                    Some(&expected_target),
                    "The authored target GUID is retained"
                );
                match reference.status() {
                    DefinitionStatus::Dangling => {
                        assert!(dangling, "Missing targets retain dangling status");
                        assert!(
                            reference.target_record_index().value().is_none(),
                            "A missing target has no physical index"
                        );
                    }
                    DefinitionStatus::UnexpectedTargetType => {
                        assert!(!dangling, "Wrong target types remain explicit");
                    }
                    other => return Err(format!("unexpected resolution state: {other:?}").into()),
                }
                Ok(())
            },
        )??;
    }
    Ok(())
}

fn guid(value: u128) -> TestResult<Guid> {
    Ok(archive_fixture::guid(value).parse()?)
}
