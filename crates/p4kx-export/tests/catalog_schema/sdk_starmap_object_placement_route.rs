// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::time::UNIX_EPOCH;

use p4kx_export::catalog_queries::starmap::{
    StarmapObjectPlacementSnapshot, StarmapObjectPlacementStatus, read_starmap_object_placement,
};
use p4kx_export::{CatalogOptions, CatalogPublicationMode, StarmapTraversalBudget, build_catalog};
use p4kx_extraction::Archive;
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestRecord, TestStruct, TestType, TestValue,
    test_key,
};
use serde_json::Value;

use super::{Route, TestResult};

const DATASET: &str = "/placements/starmapPlacements";
const RETAINED: u128 = 0x1166_5101;
const CONFLICT: u128 = 0x1166_5102;
const UNPLACED: u128 = 0x1166_5103;
const FIRST_INDEX: u32 = 0;
const DUPLICATE_INDEX: u32 = 1;
const CONFLICT_INDEX: u32 = 2;
const UNPLACED_INDEX: u32 = 3;
const OUTSIDE_PU_INDEX: u32 = 4;

pub(super) fn read() -> TestResult<Route> {
    let (_directory, archive) = open_fixture(true)?;
    let mut options = CatalogOptions::new(UNIX_EPOCH);
    options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    let catalog: Value = serde_json::from_slice(build_catalog(&archive, &options)?.json())?;

    let first = verify_repeated_placement(&archive, &catalog)?;
    verify_ambiguous_and_absent_placements(&archive, &catalog)?;
    verify_outside_pu(&archive)?;
    verify_refused(&archive)?;

    Route::queried_rows(
        DATASET,
        &first.rows,
        "multi-root synthetic parity and refusal available; installed producer parity and SDK-only memory open",
    )
}

fn verify_repeated_placement(
    archive: &Archive,
    catalog: &Value,
) -> TestResult<StarmapObjectPlacementSnapshot> {
    let first = complete_query(
        archive,
        catalog,
        FIRST_INDEX,
        StarmapObjectPlacementStatus::Placed,
    )?;
    let duplicate = complete_query(
        archive,
        catalog,
        DUPLICATE_INDEX,
        StarmapObjectPlacementStatus::Placed,
    )?;
    let ([first_row], [duplicate_row]) = (first.rows.as_slice(), duplicate.rows.as_slice()) else {
        return Err("both physical owners have their own placement row".into());
    };
    assert_eq!(
        first_row.system().value().map(String::as_str),
        Some("alpha"),
        "the first system in canonical root order supplies a repeated placement"
    );
    assert_eq!(
        first_row.entity_name().value().map(String::as_str),
        Some("alpha-repeat"),
        "the selected authored child keeps its own entity name"
    );
    assert_eq!(
        first_row.position().value(),
        Some(&[1.0, 2.0, 3.0]),
        "matching repetitions keep the first selected pose"
    );
    assert_eq!(
        first_row.position().origin().contains_encrypted_source(),
        Some(false),
        "the encrypted matching repetition does not taint the selected clear placement"
    );
    assert_eq!(
        duplicate_row.object_guid().value(),
        first_row.object_guid().value(),
        "the physical alias retains the nominal GUID shared by both Records"
    );
    assert_eq!(
        duplicate_row.object_record_index().value(),
        Some(&DUPLICATE_INDEX),
        "the selected physical alias keeps its distinct Record index"
    );
    Ok(first)
}

fn verify_ambiguous_and_absent_placements(archive: &Archive, catalog: &Value) -> TestResult<()> {
    let ambiguous = complete_query(
        archive,
        catalog,
        CONFLICT_INDEX,
        StarmapObjectPlacementStatus::Ambiguous,
    )?;
    let unplaced = complete_query(
        archive,
        catalog,
        UNPLACED_INDEX,
        StarmapObjectPlacementStatus::Unplaced,
    )?;
    let ([ambiguous_row], [unplaced_row]) = (ambiguous.rows.as_slice(), unplaced.rows.as_slice())
    else {
        return Err("both admitted objects retain their status row".into());
    };
    let conflict = ambiguous_row.position();
    assert!(
        conflict.value().is_none(),
        "conflicting positions across roots have no uniquely selected pose"
    );
    assert!(
        conflict.origin().source_set_complete(),
        "both decisive positions are captured"
    );
    assert_eq!(
        conflict.origin().contains_encrypted_source(),
        Some(true),
        "the encrypted conflicting original contributes to the ambiguity"
    );
    assert!(
        !unplaced_row.position().origin().source_set_complete(),
        "the unplaced row follows the canonical unknown placement origin"
    );
    Ok(())
}

fn verify_outside_pu(archive: &Archive) -> TestResult<()> {
    let absent = query(archive, OUTSIDE_PU_INDEX)?;
    assert_eq!(
        absent.status,
        StarmapObjectPlacementStatus::NotAdmitted,
        "an original Record outside the persistent universe is not a placement owner"
    );
    assert!(
        absent.rows.is_empty(),
        "rejected owners have no placement rows"
    );
    assert!(
        absent.traversal.is_none(),
        "rejected owners need no root traversal"
    );
    Ok(())
}

fn complete_query(
    archive: &Archive,
    catalog: &Value,
    index: u32,
    expected: StarmapObjectPlacementStatus,
) -> TestResult<StarmapObjectPlacementSnapshot> {
    let snapshot = query(archive, index)?;
    assert_eq!(
        snapshot.status, expected,
        "the physical selection has its canonical status"
    );
    let coverage = snapshot.traversal.as_ref().ok_or("all-root traversal")?;
    assert_eq!(coverage.systems, 2, "both canonical systems are surveyed");
    assert!(
        coverage.is_complete(),
        "complete results prove all root traversal"
    );
    let [row] = snapshot.rows.as_slice() else {
        return Err("expected one physically selected starmap placement row".into());
    };
    let expected = catalog
        .pointer(&format!("{DATASET}/rows"))
        .and_then(Value::as_array)
        .ok_or("canonical starmap placement table")?
        .iter()
        .find(|row| {
            row.pointer("/objectRecordIndex/value")
                .and_then(Value::as_u64)
                == Some(u64::from(index))
        })
        .ok_or("canonical row for selected physical index")?;
    assert_eq!(
        &serde_json::to_value(row)?,
        expected,
        "the scoped placement and original field origins match the Catalog"
    );
    Ok(snapshot)
}

fn verify_refused(complete: &Archive) -> TestResult<()> {
    let (_directory, unreadable) = open_fixture(false)?;
    let incomplete = query(&unreadable, FIRST_INDEX)?;
    assert_eq!(
        incomplete.status,
        StarmapObjectPlacementStatus::Unavailable,
        "a failed root can change even a discovered result"
    );
    assert!(
        incomplete.rows.is_empty(),
        "a partial root survey cannot certify a row"
    );
    assert!(
        !incomplete
            .traversal
            .ok_or("unreadable root coverage")?
            .is_complete(),
        "the failed root remains accounted for in traversal coverage"
    );

    let budget = StarmapTraversalBudget::new().with_placed_children_per_system(1);
    let truncated = read_starmap_object_placement(complete, FIRST_INDEX, budget, None)?;
    assert_eq!(
        truncated.status,
        StarmapObjectPlacementStatus::Unavailable,
        "the caller budget cannot silently truncate a canonical result"
    );
    assert!(
        truncated.rows.is_empty(),
        "a truncated traversal cannot certify a row"
    );
    assert!(
        !truncated
            .traversal
            .ok_or("truncated traversal coverage")?
            .routes_complete,
        "the affected routes remain explicitly incomplete"
    );
    Ok(())
}
fn query(archive: &Archive, index: u32) -> TestResult<StarmapObjectPlacementSnapshot> {
    Ok(read_starmap_object_placement(
        archive,
        index,
        StarmapTraversalBudget::new(),
        None,
    )?)
}

fn open_fixture(beta_readable: bool) -> TestResult<(tempfile::TempDir, Archive)> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("Data.p4k");
    let database = DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("StarMapObject").field("hideInStarmap", TestType::Bool))
        .record(object(RETAINED, "Repeat", "pu"))
        .record(object(RETAINED, "Duplicate", "pu"))
        .record(object(CONFLICT, "Conflict", "pu"))
        .record(object(UNPLACED, "Unplaced", "pu"))
        .record(object(RETAINED, "Outside", "arena"))
        .build();
    let beta = if beta_readable {
        root("beta", "40,50,60")
    } else {
        b"unreadable system archive".to_vec()
    };
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", database))
        .entry(
            TestEntry::stored(
                "Data/ObjectContainers/PU/system/beta/betasystem.socpak",
                beta,
            )
            .encrypted(),
        )
        .entry(TestEntry::stored(
            "Data/ObjectContainers/PU/system/alpha/alphasystem.socpak",
            root("alpha", "4,5,6"),
        ))
        .write_to(&path)?;
    let archive = Archive::open_with_key(&path, Some(test_key()))?;
    Ok((directory, archive))
}

fn object(guid: u128, name: &str, scope: &str) -> TestRecord {
    TestRecord::new(
        guid,
        "StarMapObject",
        format!("StarMapObject.{name}"),
        format!("libs/foundry/records/starmap/{scope}/fixture/{name}.xml"),
    )
    .value("hideInStarmap", TestValue::Bool(false))
}

fn root(system: &str, conflict_pos: &str) -> Vec<u8> {
    let xml = format!(
        r#"<ObjectContainer version="14"><ChildObjectContainers>
        <Child entityName="{system}-repeat" starMapRecord="00000000-0000-0000-0000-{RETAINED:012x}" pos="1,2,3"/>
        <Child entityName="{system}-conflict" starMapRecord="00000000-0000-0000-0000-{CONFLICT:012x}" pos="{conflict_pos}"/>
        </ChildObjectContainers></ObjectContainer>"#,
    );
    ArchiveBuilder::v2()
        .entry(
            TestEntry::stored(format!("{system}system.xml"), xml.into_bytes())
                .with_encryption_field(0),
        )
        .entry(TestEntry::stored("empty.soc", Vec::new()))
        .build()
}
