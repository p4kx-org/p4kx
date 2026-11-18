// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::mining::{with_mining_global_params, with_refining_processes};
use p4kx_export::catalog_queries::{CatalogLanguageSelection, CatalogRecordSelection};

use super::fixture;
use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};

#[test]
fn nominal_guids_keep_all_physical_aliases_and_record_index_selects_each_owner() -> TestResult {
    let (_folder, archive) = common::open(&fixture::database(false), DcbOrigin::Plain)?;
    let indices = with_mining_global_params(
        &archive,
        CatalogRecordSelection::Guid(common::guid(fixture::GLOBAL).parse()?),
        None,
        |snapshot| {
            snapshot
                .rows()
                .iter()
                .map(|row| {
                    row.source()
                        .record_index()
                        .value()
                        .copied()
                        .ok_or("original global index")
                })
                .collect::<Result<Vec<_>, _>>()
        },
    )??;
    assert_eq!(
        indices,
        [0, 1],
        "global aliases keep stable physical tie order"
    );
    for index in indices {
        with_mining_global_params(
            &archive,
            CatalogRecordSelection::RecordIndex(index),
            None,
            |snapshot| {
                assert_eq!(snapshot.rows().len(), 1, "one physical global owner");
                assert_eq!(
                    snapshot.rows()[0].source().record_index().value(),
                    Some(&index),
                    "exact physical global identity"
                );
            },
        )?;
    }
    refining_aliases(&archive)?;
    Ok(())
}

fn refining_aliases(archive: &p4kx_extraction::Archive) -> TestResult {
    let indices = with_refining_processes(
        archive,
        CatalogRecordSelection::Guid(common::guid(fixture::PROCESS).parse()?),
        CatalogLanguageSelection::Selected(&[]),
        None,
        |snapshot| {
            snapshot
                .rows()
                .iter()
                .map(|row| {
                    row.source()
                        .record_index()
                        .value()
                        .copied()
                        .ok_or("original process index")
                })
                .collect::<Result<Vec<_>, _>>()
        },
    )??;
    assert_eq!(
        indices,
        [2, 3],
        "refining aliases keep stable physical tie order"
    );
    for index in indices {
        with_refining_processes(
            archive,
            CatalogRecordSelection::RecordIndex(index),
            CatalogLanguageSelection::Selected(&[]),
            None,
            |snapshot| -> TestResult {
                let [row] = snapshot.rows() else {
                    return Err("one physical refining owner".into());
                };
                assert_eq!(
                    row.source().record_index().value(),
                    Some(&index),
                    "exact physical refining identity"
                );
                Ok(())
            },
        )??;
    }
    Ok(())
}

#[test]
fn open_enum_labels_and_present_empty_values_are_not_legacy_null_shortcuts() -> TestResult {
    let (_folder, archive) = common::open(&fixture::database(false), DcbOrigin::Plain)?;
    with_refining_processes(
        &archive,
        CatalogRecordSelection::RecordIndex(3),
        CatalogLanguageSelection::Selected(&[]),
        None,
        |snapshot| {
            let row = &snapshot.rows()[0];
            assert_eq!(
                row.refining_speed().value(),
                Some(&""),
                "present empty Enum is retained"
            );
            assert_eq!(
                row.refining_quality().value(),
                Some(&"FutureValue"),
                "new open game vocabulary is retained without defaulting"
            );
            assert!(
                snapshot.coverage().incomplete_fields.is_empty(),
                "new readable enum labels are not malformed controls"
            );
        },
    )?;
    Ok(())
}
