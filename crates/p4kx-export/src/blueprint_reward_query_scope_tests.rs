// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    Archive, DataForgeEntry, FieldOrigin, SourceLocation, SourceValueState, Value,
};
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestRecord, TestStruct, TestType, TestValue,
};

use super::{TestResult, fixture};
use crate::catalog_queries::CatalogRecordSelection;
use crate::catalog_queries::crafting::{self, BlueprintRewardSelection, BlueprintRewardSnapshot};
use crate::catalog_read::original::{OwnedValueEvent, OwnedValueWalk};

#[test]
fn informational_identity_reference_errors_still_mark_reward_metadata_incomplete() -> TestResult {
    for (kind, value, incomplete) in [
        (TestType::Guid, TestValue::Guid(31), false),
        (TestType::Reference, TestValue::Reference(99), true),
    ] {
        for encrypted in [false, true] {
            let database = identity_database(kind.clone(), value.clone());
            let folder = tempfile::tempdir()?;
            let path = folder.path().join("identity.p4k");
            let entry = TestEntry::stored("Data/Game2.dcb", database.build());
            let entry = if encrypted { entry.encrypted() } else { entry };
            ArchiveBuilder::new().entry(entry).write_to(&path)?;
            let archive = Archive::open_with_key(path, Some(p4kx_testkit::test_key()))?;
            let selection =
                BlueprintRewardSelection::SourceOwners(CatalogRecordSelection::RecordIndex(1));
            crafting::with_reward_sources(&archive, selection, None, |snapshot| {
                verify_identity(&snapshot, incomplete, encrypted)
            })??;
        }
    }
    Ok(())
}

fn identity_database(kind: TestType, value: TestValue) -> DataForgeBuilder {
    DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("BlueprintPoolRecord"))
        .structure(
            TestStruct::new("Owner")
                .field("id", kind)
                .field("debugName", TestType::String)
                .field("pool", TestType::Reference)
                .field("chance", TestType::UInt32),
        )
        .record(TestRecord::new(
            1,
            "BlueprintPoolRecord",
            "BlueprintPoolRecord.Primary",
            "libs/foundry/records/crafting/blueprintrewards/primary.xml",
        ))
        .record(
            TestRecord::new(2, "Owner", "Owner.Source", "outside/source.xml")
                .value("id", value)
                .value("debugName", TestValue::text("Fallback name"))
                .value("pool", TestValue::Reference(1))
                .value("chance", TestValue::UInt32(7)),
        )
}

fn verify_identity(
    snapshot: &BlueprintRewardSnapshot<'_>,
    incomplete: bool,
    encrypted: bool,
) -> TestResult {
    let [row] = snapshot.rows() else {
        return Err("one original qualifying pool reference".into());
    };
    assert_eq!(
        row.contract_id().state(),
        if incomplete {
            SourceValueState::Absent
        } else {
            SourceValueState::Present
        },
        "the existing GUID-only identity rule is unchanged"
    );
    for origin in [row.contract_id().origin(), row.contract_name().origin()] {
        verify_origin(origin, incomplete, encrypted);
    }
    assert_eq!(
        row.chance().origin().contains_encrypted_source(),
        Some(encrypted),
        "local chance has its own complete original evidence"
    );
    assert_eq!(
        row.source_ordinal().origin().contains_encrypted_source(),
        Some(encrypted),
        "an identity gap does not stamp the whole row unknown"
    );
    assert_eq!(
        snapshot.source_coverage().incomplete_fields,
        if incomplete { vec![1] } else { vec![] },
        "per-cell gaps must not vanish behind informational decoder diagnostics"
    );
    assert!(
        snapshot.source_coverage().unreadable_bodies.is_empty(),
        "the owner body is actually readable"
    );
    assert!(
        snapshot.pool_coverage().unreadable_headers.is_empty()
            && snapshot.pool_coverage().unreadable_bodies.is_empty(),
        "the actual pool dependency is healthy"
    );
    Ok(())
}

#[test]
fn physically_excluded_source_owner_retains_no_orphan_source_uses() -> TestResult {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("physical-filter.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored(
            "Data/Game2.dcb",
            fixture::database(false).build(),
        ))
        .write_to(&path)?;
    let archive = Archive::open(path)?;
    let excluded = excluded_direct_ordinal(&archive)?;
    let selection =
        BlueprintRewardSelection::TargetPools(CatalogRecordSelection::RecordIndex(fixture::ALIAS));
    crafting::with_reward_sources(&archive, selection, None, |snapshot| {
        verify_filtered_uses(&snapshot, excluded)
    })??;
    Ok(())
}

fn verify_filtered_uses(snapshot: &BlueprintRewardSnapshot<'_>, excluded: u64) -> TestResult {
    assert_eq!(
        snapshot.source_records_read(),
        8,
        "the inverse scan really reaches every source body"
    );
    let [row] = snapshot.rows() else {
        return Err("one actual physical target link".into());
    };
    assert_eq!(
        row.source().record_index().value(),
        Some(&fixture::SOURCE),
        "only the source with the actual weak target is emitted"
    );
    assert!(
        snapshot
            .source_inventory()
            .iter_uses()
            .any(|used| belongs_to(used.location(), fixture::SOURCE)),
        "the returned row retains real original use evidence"
    );
    assert!(
        snapshot
            .source_inventory()
            .iter_uses()
            .all(|used| !belongs_to(used.location(), fixture::SOURCE_ALIAS)),
        "a scanned owner with only physically excluded links registers no original-use claims"
    );
    assert!(snapshot.source_inventory().iter_uses().all(|used| !matches!(used.location(),
        SourceLocation::Subresource { namespace, indices, field }
            if namespace == "dataforgeValue" && indices.as_slice() == [u64::from(fixture::SOURCE), excluded]
                && field == "$value")), "the physically excluded direct occurrence in the emitted owner retains no payload-use claim");
    Ok(())
}

fn excluded_direct_ordinal(archive: &Archive) -> TestResult<u64> {
    let entry = DataForgeEntry::read(archive)?;
    let forge = entry.open()?;
    let record = forge
        .record(fixture::SOURCE)
        .ok_or("actual source Record")?;
    let tree = record.tree()?;
    let Value::Reference(reference) = tree
        .root()
        .field("direct")
        .ok_or("actual direct pool link")?
    else {
        return Err("the control must be an original Reference".into());
    };
    assert_eq!(
        reference.record_index(),
        0,
        "the direct occurrence reaches the unselected physical alias"
    );
    let mut walk = OwnedValueWalk::new(&record, &tree, None)?;
    while let Some(event) = walk.next_event()? {
        if let OwnedValueEvent::Value(visit) = event
            && visit.logical_path() == "direct"
        {
            return Ok(visit.ordinal());
        }
    }
    Err("original direct-reference preorder occurrence".into())
}

fn belongs_to(location: &SourceLocation, record: u32) -> bool {
    match location {
        SourceLocation::DataForge { record_index, .. } => *record_index == record,
        SourceLocation::Subresource {
            namespace, indices, ..
        } if matches!(
            namespace.as_str(),
            "dataforgeRecordHeader" | "dataforgeValue" | "dataforgeTraversal"
        ) =>
        {
            indices.first() == Some(&u64::from(record))
        }
        _ => false,
    }
}

fn verify_origin(origin: &FieldOrigin, incomplete: bool, encrypted: bool) {
    assert_eq!(
        origin.source_set_complete(),
        !incomplete,
        "actual selector read completeness"
    );
    assert_eq!(
        origin.contains_encrypted_source(),
        if encrypted {
            Some(true)
        } else if incomplete {
            None
        } else {
            Some(false)
        },
        "known encrypted contributors remain true even when selector knowledge is incomplete"
    );
}
