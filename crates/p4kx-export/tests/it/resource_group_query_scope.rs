// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::CatalogError;
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection,
    with_resource_group_parents, with_resource_groups,
};
use p4kx_extraction::{Archive, CancelToken, DataForgeEntry, Language};
use p4kx_testkit::{ArchiveBuilder, TestEntry};

use super::fixture;
use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};

#[test]
fn parsed_parent_queries_do_not_consult_requested_encrypted_locales_or_world_payloads() -> TestResult
{
    let (_folder, archive) = locked_inputs()?;
    let entry = DataForgeEntry::read(&archive)?;
    let session = CatalogQuerySession::from_entry(
        &archive,
        &entry,
        CatalogLanguageSelection::AllAvailable,
        None,
    )?;
    let before = archive.key_demand().snapshot().unread();
    let parsed = session.parsed(None)?;
    parsed.with_resource_group_parents(CatalogRecordSelection::All, None, |snapshot| {
        assert_eq!(
            snapshot.rows().len(),
            2,
            "The parent projection works without locale or world access"
        );
        assert_eq!(
            snapshot.coverage().unreadable_bodies.len(),
            0,
            "Both synthetic physical owners were readable"
        );
        assert_eq!(
            snapshot.dependency_coverage().unreadable_bodies.len(),
            0,
            "The canonical group survey was readable"
        );
    })?;
    assert_eq!(
        archive.key_demand().snapshot().unread(),
        before,
        "Parent preparation does not request an Archive key for existing encrypted nondependencies"
    );
    Ok(())
}

fn locked_inputs() -> TestResult<(tempfile::TempDir, Archive)> {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("locked-inputs.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored(
            "Data/Game2.dcb",
            fixture::clean().build(),
        ))
        .entry(
            TestEntry::stored(Language::English.global_ini_path(), b"shared=Locked text")
                .encrypted(),
        )
        .entry(
            TestEntry::stored(
                "Data/ObjectContainers/PU/system/synthetic/syntheticsystem.socpak",
                b"locked world",
            )
            .encrypted(),
        )
        .write_to(&path)?;
    Ok((folder, Archive::open(path)?))
}

#[test]
fn pre_cancelled_resource_group_queries_never_publish_a_snapshot() -> TestResult {
    let (_folder, archive) = common::open(&fixture::clean(), DcbOrigin::Plain)?;
    let cancel = CancelToken::new();
    cancel.cancel();
    let mut direct = false;
    let result = with_resource_groups(
        &archive,
        CatalogRecordSelection::All,
        CatalogLanguageSelection::AllAvailable,
        Some(&cancel),
        |_| {
            direct = true;
        },
    );
    assert!(
        matches!(result, Err(CatalogError::Cancelled)),
        "The direct query preserves cancellation"
    );
    assert!(
        !direct,
        "A cancelled direct query does not call its consumer"
    );
    let mut parent = false;
    let result =
        with_resource_group_parents(&archive, CatalogRecordSelection::All, Some(&cancel), |_| {
            parent = true;
        });
    assert!(
        matches!(result, Err(CatalogError::Cancelled)),
        "The parent query preserves cancellation"
    );
    assert!(
        !parent,
        "A cancelled parent query does not call its consumer"
    );
    Ok(())
}

#[test]
fn callback_success_does_not_hide_cancellation_before_outer_query_success() -> TestResult {
    let (_folder, archive) = common::open(&fixture::clean(), DcbOrigin::Plain)?;
    let direct_cancel = CancelToken::new();
    let mut direct = false;
    let result = with_resource_groups(
        &archive,
        CatalogRecordSelection::All,
        CatalogLanguageSelection::Selected(&[]),
        Some(&direct_cancel),
        |snapshot| {
            assert_eq!(
                snapshot.definitions().len(),
                2,
                "Both direct owners reach the callback"
            );
            direct = true;
            direct_cancel.cancel();
        },
    );
    assert!(
        direct && matches!(result, Err(CatalogError::Cancelled)),
        "A complete callback result is rejected after cancellation"
    );
    let parent_cancel = CancelToken::new();
    let mut parent = false;
    let result = with_resource_group_parents(
        &archive,
        CatalogRecordSelection::All,
        Some(&parent_cancel),
        |snapshot| {
            assert_eq!(
                snapshot.rows().len(),
                2,
                "Both physical parent rows reach the callback"
            );
            parent = true;
            parent_cancel.cancel();
        },
    );
    assert!(
        parent && matches!(result, Err(CatalogError::Cancelled)),
        "Parent results also require outer success before consumer publication"
    );
    Ok(())
}
