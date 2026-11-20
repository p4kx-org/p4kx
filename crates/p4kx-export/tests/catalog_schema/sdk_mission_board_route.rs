// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::time::UNIX_EPOCH;

use p4kx_export::catalog_queries::equipment::mission_boards::{
    BoardDetails, MissionBoardSnapshot, with_mission_boards_from,
};
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, DatasetAvailability,
};
use p4kx_export::{CatalogOptions, build_catalog_from};
use p4kx_extraction::{Archive, DataForgeEntry, InputScope, SourceLocation, SourceValueState};
use p4kx_testkit::{ArchiveBuilder, DataForgeBuilder, TestEntry};
use serde_json::Value;

use super::component_parity::parity;
use super::{Route, TestResult};

#[path = "../it/mission_board_fixture.rs"]
mod fixture;

pub(super) const DATASET: &str = "/configurations/missionBoards";

const BOARD_OWNER: u32 = 2;
const GUILD_RECORD: u32 = 1;

pub(super) fn read() -> TestResult<Route> {
    let (_directory, archive) = open_fixture(&fixture::database())?;
    let entry = DataForgeEntry::read(&archive)?;
    let catalog: Value = serde_json::from_slice(
        build_catalog_from(&archive, &entry, &CatalogOptions::new(UNIX_EPOCH))?.json(),
    )?;
    with_mission_boards_from(
        &archive,
        &entry,
        CatalogRecordSelection::RecordIndex(BOARD_OWNER),
        None,
        |snapshot| {
            verify_rows(&snapshot)?;
            parity(DATASET, &snapshot, &catalog)?;
            Route::queried_rows(
                DATASET,
                snapshot.rows(),
                "encrypted synthetic producer and source parity witnessed; installed producer parity open",
            )
        },
    )?
}

fn verify_rows(snapshot: &MissionBoardSnapshot<'_>) -> TestResult<()> {
    let [career, notice, authored, empty, base] = snapshot.rows() else {
        return Err("five original provider slots".into());
    };
    for (row, ordinal) in snapshot.rows().iter().zip([1, 2, 4, 5, 6]) {
        assert_eq!(
            row.owner().record_index().value(),
            Some(&BOARD_OWNER),
            "keep the physical provider owner"
        );
        let component = row.owner().component_ordinal();
        assert_eq!(
            component.value(),
            Some(&ordinal),
            "keep the original component slot"
        );
        assert_eq!(
            component.origin().contains_encrypted_source(),
            Some(true),
            "the provider slot is observed in the encrypted DataForge Entry"
        );
    }
    assert!(
        notice
            .board_type()
            .value()
            .is_some_and(|row| matches!(row.details(), BoardDetails::Notice)),
        "preserve the Notice board"
    );
    assert!(
        authored.board_type().value().is_some_and(|row| {
            matches!(row.details(), BoardDetails::Authored)
                && !row.authored_fields().roots().is_empty()
        }),
        "preserve the unknown authored board and its fields"
    );
    assert_eq!(
        empty.board_type().state(),
        SourceValueState::Null,
        "retain the original null slot"
    );
    assert!(
        base.board_type()
            .value()
            .is_some_and(|row| matches!(row.details(), BoardDetails::Base)),
        "preserve the Base board"
    );
    verify_career(
        snapshot,
        career
            .board_type()
            .value()
            .ok_or("original Career board type")?
            .details(),
    )
}

fn verify_career(
    snapshot: &MissionBoardSnapshot<'_>,
    details: &BoardDetails<'_>,
) -> TestResult<()> {
    let BoardDetails::Career {
        guild_faction_reputation,
        factions,
    } = details
    else {
        return Err("typed Career details".into());
    };
    let [first, empty, dangling, repeated] =
        factions.value().ok_or("original faction slots")?.as_slice()
    else {
        return Err("preserved Career faction slots".into());
    };
    assert_eq!(
        empty.state(),
        SourceValueState::Null,
        "the null faction remains in its original position"
    );
    let dangling = dangling.value().ok_or("authored dangling reference")?;
    assert_eq!(
        serde_json::to_value(dangling.status())?,
        "dangling",
        "the missing target does not erase its authored slot"
    );
    let first = first.value().ok_or("original first reference")?;
    let repeated = repeated.value().ok_or("original repeated reference")?;
    assert_eq!(
        first.guid().value(),
        repeated.guid().value(),
        "an authored occurrence is preserved even when it repeats a target"
    );
    let guild = guild_faction_reputation
        .value()
        .ok_or("original guild link")?;
    let target = guild.target_record_index();
    assert_eq!(
        target.value(),
        Some(&GUILD_RECORD),
        "retain the exact guild target header"
    );
    assert!(
        snapshot.source_inventory().iter_uses().any(|source| {
            source.scope() == InputScope::Payload
                && target.origin().source_uses_refs().contains(source.id())
                && matches!(source.location(), SourceLocation::Subresource {
                    namespace, indices, field
                } if namespace == "dataforgeRecordHeader"
                    && indices.as_slice() == [u64::from(GUILD_RECORD)]
                    && field == "recordIndex")
        }),
        "the original target header has an independent physical source use"
    );
    Ok(())
}

#[test]
fn board_provider_query_follows_physical_selection_for_owners_shared_with_items() -> TestResult<()>
{
    let (_directory, archive) = open_fixture(&fixture::shared_item_database())?;
    let session =
        CatalogQuerySession::open(&archive, CatalogLanguageSelection::Selected(&[]), None)?;
    let parsed = session.parsed(None)?;
    parsed.with_mission_boards(
        CatalogRecordSelection::RecordIndex(BOARD_OWNER),
        None,
        |snapshot| verify_rows(&snapshot),
    )??;
    parsed.with_mission_boards(
        CatalogRecordSelection::RecordIndex(GUILD_RECORD),
        None,
        |snapshot| {
            assert!(
                snapshot.rows().is_empty(),
                "the guild Record is not a board owner"
            );
            assert!(
                matches!(snapshot.availability(), DatasetAvailability::Available),
                "the exact provider declaration remains available"
            );
            assert!(
                snapshot.source_inventory().iter_uses().all(|used| {
                    used.scope() == InputScope::ArchiveDirectoryMetadata
                        && matches!(used.location(), SourceLocation::Metadata { .. })
                }),
                "a non-board selection retains only its DataForge Entry directory metadata"
            );
            assert!(
                snapshot.coverage().unreadable_bodies.is_empty(),
                "the foreign Record body is outside scope"
            );
        },
    )?;
    Ok(())
}

fn open_fixture(database: &DataForgeBuilder) -> TestResult<(tempfile::TempDir, Archive)> {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("mission-boards.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", database.build()).encrypted())
        .entry(TestEntry::stored(
            "Data/Libs/CosmoTools/LayerRecovery/LayerBackups.xml",
            b"<LayerBackups/>",
        ))
        .write_to(&path)?;
    let archive = Archive::open_with_key(path, Some(p4kx_testkit::test_key()))?;
    Ok((folder, archive))
}
