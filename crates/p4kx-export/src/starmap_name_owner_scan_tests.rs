// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_extraction::{Archive, DataForge, Filter, SourceUseRole, SourcedValue};
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestRecord, TestStruct, TestType, TestValue,
};

use super::{CatalogRecordSelection, DatasetAvailability, visit, visit_records};
use crate::catalog_read::original::with_entry_capture;
use crate::guid_text::CatalogGuid;
use crate::{CatalogError, ExportAccountingError};

const STARMAP: &str = "StarMapObject";

struct LocatedName {
    index: u32,
    guid: CatalogGuid,
    class_name: SourcedValue<String>,
}

fn record(guid: u128, class_name: &str) -> TestRecord {
    TestRecord::new(
        guid,
        STARMAP,
        format!("{STARMAP}.{class_name}"),
        format!("libs/foundry/records/starmap/pu/system/sample/{class_name}.xml"),
    )
    .value("name", TestValue::Bool(true))
}

fn fixture_bytes() -> Vec<u8> {
    DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new(STARMAP).field("name", TestType::Bool))
        .record(record(1, "Alpha"))
        .record(record(1, "Alias"))
        .record(record(2, "Broken").with_instance_index(u16::MAX))
        .record(record(3, "Dev_Orphan"))
        .record(TestRecord::new(
            4,
            STARMAP,
            "StarMapObject.Unrelated",
            "libs/foundry/records/unrelated/name.xml",
        ))
        .build()
}

fn fixture() -> Result<(tempfile::TempDir, Archive), Box<dyn Error>> {
    let dcb = fixture_bytes();
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("Data.p4k");
    ArchiveBuilder::v2()
        .entry(TestEntry::stored("Data/Game2.dcb", dcb).encrypted())
        .write_to(&path)?;
    let archive = Archive::open_with_key(path, Some(p4kx_testkit::test_key()))?;
    Ok((folder, archive))
}

#[test]
fn name_scan_keeps_physical_aliases_and_separate_structural_failures() -> Result<(), Box<dyn Error>>
{
    let (_folder, archive) = fixture()?;
    with_entry_capture(&archive, |capture| {
        let mut rows = Vec::new();
        let survey = visit(
            &archive,
            capture,
            CatalogRecordSelection::All,
            None,
            |index, name| {
                let guid = name
                    .guid
                    .value()
                    .copied()
                    .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
                rows.push(LocatedName {
                    index,
                    guid,
                    class_name: name.class_name.map(str::to_owned),
                });
                Ok(())
            },
        )?;
        assert!(
            matches!(survey.availability, DatasetAvailability::Available),
            "the physical family exists"
        );
        assert_eq!(
            survey.coverage.unreadable_bodies,
            [2],
            "the broken selected tree cannot become a name candidate"
        );
        assert_eq!(
            survey.coverage.skipped_leftovers, 1,
            "the usual leftover rule applies"
        );
        assert!(
            survey.coverage.incomplete_fields.is_empty(),
            "unrelated authored name fields do not affect structural class labels"
        );
        assert_names(&rows)?;
        let inventory = survey
            .origins
            .uses
            .finish_with(|| Ok::<(), CatalogError>(()))?;
        assert!(
            inventory
                .iter_uses()
                .any(|used| used.role() == SourceUseRole::Conversion),
            "the retained origin registry resolves after the decoded DataForge is released"
        );
        Ok(())
    })?;
    Ok(())
}

#[test]
fn physical_ancestor_scope_distinguishes_selected_excluded_and_unreadable_owners()
-> Result<(), Box<dyn Error>> {
    let bytes = fixture_bytes();
    let forge = DataForge::open(&bytes)?;
    let guid = forge.record(1).ok_or("physical alias")?.guid();
    let only_alias = Filter::new("/alias");
    for (index, filter, admitted, unreadable) in [
        (1, Some(&only_alias), Some(1), None),
        (0, Some(&only_alias), None, None),
        (2, None, None, Some(2)),
        (4, None, None, None),
        (u32::MAX, None, None, None),
    ] {
        let mut admitted_records = Vec::new();
        let (_, coverage) = visit_records(
            &forge,
            CatalogRecordSelection::RecordIndex(index),
            filter,
            None,
            |record, tree| {
                assert!(
                    tree.is_from(record),
                    "the exact original physical tree is admitted"
                );
                admitted_records.push((record.index(), record.guid()));
                Ok(())
            },
        )?;
        assert!(
            admitted_records
                .into_iter()
                .eq(admitted.map(|index| (index, guid))),
            "only the exact admitted physical Record contributes a location fact"
        );
        assert!(
            coverage.unreadable_bodies.into_iter().eq(unreadable),
            "selected read failure remains distinct from a complete empty physical selection"
        );
    }
    Ok(())
}

fn assert_names(rows: &[LocatedName]) -> Result<(), CatalogError> {
    let [first, second] = rows else {
        return Err(ExportAccountingError::ConflictingSourceDatum.into());
    };
    assert_eq!(
        (first.index, second.index),
        (0, 1),
        "both physical Records are retained"
    );
    assert_eq!(
        first.guid, second.guid,
        "a nominal GUID collision does not collapse owners"
    );
    for (row, expected) in [(first, "Alpha"), (second, "Alias")] {
        assert_eq!(
            row.class_name.value().map(String::as_str),
            Some(expected),
            "the class label stays tied to the original Record header"
        );
        assert_eq!(
            row.class_name.origin().contains_encrypted_source(),
            Some(true),
            "the class name remains protected with the decoded source released"
        );
    }
    assert_ne!(
        first.class_name.origin(),
        second.class_name.origin(),
        "the names come from different physical coordinates"
    );
    Ok(())
}
