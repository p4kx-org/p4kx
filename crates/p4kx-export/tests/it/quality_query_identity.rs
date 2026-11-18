// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::quality::{
    CatalogRecordSelection, with_distributions, with_quantizations,
};
use p4kx_extraction::{Archive, Guid};

use super::fixture;
use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};

#[test]
fn duplicate_distribution_guids_keep_each_individually_selectable_physical_owner() -> TestResult {
    let (_folder, archive) = common::open(&fixture::database(), DcbOrigin::Plain)?;
    let guid = common::guid(fixture::DISTRIBUTION).parse()?;
    let indices = with_distributions(
        &archive,
        CatalogRecordSelection::Guid(guid),
        None,
        |snapshot| {
            snapshot
                .rows()
                .iter()
                .map(|row| {
                    physical(
                        row.source().record_index().value(),
                        &row.source().guid(),
                        guid,
                    )
                })
                .collect::<TestResult<Vec<_>>>()
        },
    )??;
    assert_eq!(
        indices,
        [0, 1],
        "same-GUID definitions retain their physical input order"
    );
    for index in indices {
        with_distributions(
            &archive,
            CatalogRecordSelection::RecordIndex(index),
            None,
            |snapshot| -> TestResult {
                assert_eq!(
                    snapshot.rows().len(),
                    1,
                    "physical selection disambiguates the distribution"
                );
                for row in snapshot.rows() {
                    assert_eq!(
                        physical(
                            row.source().record_index().value(),
                            &row.source().guid(),
                            guid
                        )?,
                        index,
                        "the exact physical source is returned"
                    );
                }
                Ok(())
            },
        )??;
    }
    Ok(())
}

#[test]
fn duplicate_quantization_guids_keep_raw_ineligible_and_dangling_aliases() -> TestResult {
    let (_folder, archive) = common::open(&fixture::database(), DcbOrigin::Plain)?;
    let guid = common::guid(fixture::QUANTIZATION).parse()?;
    let indices = with_quantizations(
        &archive,
        CatalogRecordSelection::Guid(guid),
        None,
        |snapshot| {
            snapshot
                .rows()
                .iter()
                .map(|row| {
                    physical(
                        row.source().record_index().value(),
                        &row.source().guid(),
                        guid,
                    )
                })
                .collect::<TestResult<Vec<_>>>()
        },
    )??;
    assert_eq!(
        indices,
        [3, 4],
        "inverse eligibility never suppresses an authored definition"
    );
    for index in indices {
        verify_quantization_index(&archive, guid, index)?;
    }
    Ok(())
}

fn verify_quantization_index(archive: &Archive, guid: Guid, index: u32) -> TestResult {
    with_quantizations(
        archive,
        CatalogRecordSelection::RecordIndex(index),
        None,
        |snapshot| -> TestResult {
            assert_eq!(
                snapshot.rows().len(),
                1,
                "physical selection disambiguates the quantization"
            );
            for row in snapshot.rows() {
                assert_eq!(
                    physical(
                        row.source().record_index().value(),
                        &row.source().guid(),
                        guid
                    )?,
                    index,
                    "no nominal GUID merge replaces the physical owner"
                );
            }
            Ok(())
        },
    )??;
    Ok(())
}

fn physical(
    index: Option<&u32>,
    identity: &p4kx_extraction::SourcedValue<Guid>,
    expected: Guid,
) -> TestResult<u32> {
    assert_eq!(
        identity.value(),
        Some(&expected),
        "selection retains the original external GUID"
    );
    index
        .copied()
        .ok_or_else(|| "original physical Record index".into())
}
