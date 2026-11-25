// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_extraction::{Archive, DataForgeEntry, ProvenanceMode};
use p4kx_testkit::{ArchiveBuilder, TestEntry, TestRecord, TestStruct};

use super::super::read::Query;
use super::{Capture, TestResult, fixture};
use crate::catalog_queries::CatalogRecordSelection;
use crate::catalog_queries::crafting;
use crate::catalog_query_session::CatalogQuerySource;

#[test]
fn unreadable_global_header_makes_dependent_choices_incomplete() -> TestResult {
    observe_header(1, false, false, &[1], &[], &[(0, true), (2, true)])
}

#[test]
fn unreadable_dismantle_header_makes_process_choice_incomplete() -> TestResult {
    observe_header(
        5,
        true,
        false,
        &[],
        &[5],
        &[(0, true), (1, true), (2, true)],
    )
}

#[test]
fn unreadable_type_on_proven_foreign_path_does_not_poison_rules() -> TestResult {
    observe_header(4, true, true, &[], &[], &[(0, true), (1, true), (2, true)])
}

fn observe_header(
    physical: u32,
    default_complete: bool,
    dismantle_complete: bool,
    global_failures: &[u32],
    process_failures: &[u32],
    expected_reads: &[(u32, bool)],
) -> TestResult {
    let bytes = malformed_type(physical)?;
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("source.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", bytes))
        .write_to(&path)?;
    let archive = Archive::open(path)?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let record = forge.record(physical).ok_or("corrupted physical header")?;
    assert!(
        record.record_path().is_ok(),
        "The known path remains readable"
    );
    assert!(
        record.type_name().is_err(),
        "The actual type header is unreadable"
    );
    verify_direct_globals(&archive, &entry, global_failures)?;
    let capture = Arc::new(Capture::default());
    let query = Query {
        archive: &archive,
        entry: CatalogQuerySource::new(&entry, ProvenanceMode::Full),
        forge: &forge,
        selection: CatalogRecordSelection::RecordIndex(0),
        languages: &[],
        cancel: None,
        observer: Some(capture.clone()),
    };
    query.run(|snapshot| {
        verify_choices(&snapshot, default_complete, dismantle_complete)?;
        verify_coverage(&snapshot, global_failures, process_failures);
        Ok::<_, Box<dyn std::error::Error>>(())
    })??;
    let reads = capture
        .0
        .lock()
        .map_err(|_| "source read observer poisoned")?;
    assert_eq!(
        reads.as_slice(),
        expected_reads,
        "Unclassified and unrelated bodies remain unread"
    );
    Ok(())
}

fn verify_direct_globals(
    archive: &Archive,
    entry: &DataForgeEntry,
    global_failures: &[u32],
) -> TestResult {
    crafting::with_global_params_from(
        archive,
        entry,
        CatalogRecordSelection::All,
        None,
        |snapshot| {
            assert_eq!(
                snapshot.coverage().unreadable_headers.as_slice(),
                global_failures,
                "Direct globals report only headers that can belong to the selected family"
            );
        },
    )?;
    Ok(())
}

fn verify_coverage(
    snapshot: &crate::catalog_queries::BlueprintSnapshot<'_>,
    global_failures: &[u32],
    process_failures: &[u32],
) {
    assert_eq!(
        snapshot
            .dependency_coverage()
            .globals()
            .unreadable_headers
            .as_slice(),
        global_failures,
        "Unclassified global headers retain their physical indices"
    );
    assert_eq!(
        snapshot
            .dependency_coverage()
            .dismantle()
            .unreadable_headers
            .as_slice(),
        process_failures,
        "Unclassified process headers retain their physical indices"
    );
}

fn verify_choices(
    snapshot: &crate::catalog_queries::BlueprintSnapshot<'_>,
    default_complete: bool,
    dismantle_complete: bool,
) -> TestResult {
    let [presentation] = snapshot.presentations() else {
        return Err("one selected physical Blueprint presentation".into());
    };
    let available = presentation.default_available();
    let dismantle = presentation.dismantle();
    assert!(
        dismantle.value().is_some(),
        "The existing known generic process survives"
    );
    assert_eq!(
        available.origin().source_set_complete(),
        default_complete,
        "Unreadable possible globals cannot prove a complete default survey"
    );
    assert_eq!(
        dismantle.origin().source_set_complete(),
        dismantle_complete,
        "Unreadable possible rule inputs cannot prove a complete process choice"
    );
    assert_eq!(
        available.origin().contains_encrypted_source(),
        default_complete.then_some(false),
        "Incomplete default knowledge does not claim a wholly plain source set"
    );
    assert_eq!(
        dismantle.origin().contains_encrypted_source(),
        dismantle_complete.then_some(false),
        "Incomplete process knowledge does not claim a wholly plain source set"
    );
    Ok(())
}

fn malformed_type(physical: u32) -> TestResult<Vec<u8>> {
    let database = fixture::database(fixture::Product::Present, false, false);
    let database = if physical == 5 {
        database
            .structure(TestStruct::new("UnclassifiedRule"))
            .record(TestRecord::new(
                6,
                "UnclassifiedRule",
                "UnclassifiedRule.Candidate",
                "libs/foundry/records/crafting/blueprints/dismantle/unknown.xml",
            ))
    } else {
        database
    };
    let mut bytes = database.build();
    let class_index = {
        let forge = p4kx_extraction::DataForge::open(&bytes)?;
        forge
            .record(physical)
            .ok_or("fixture type owner")?
            .struct_type()?
            .index()
    };
    let header_words = 9 + p4kx_extraction::ValueKind::ALL.len() + 2;
    let struct_row_bytes = 16;
    let offset = header_words * size_of::<u32>() + usize::try_from(class_index)? * struct_row_bytes;
    bytes
        .get_mut(offset..offset + size_of::<u32>())
        .ok_or("struct name field")?
        .copy_from_slice(&u32::MAX.to_le_bytes());
    Ok(bytes)
}
