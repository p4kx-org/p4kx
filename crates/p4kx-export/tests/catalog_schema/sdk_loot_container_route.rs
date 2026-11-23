// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::loot::{
    LootAncestorSelection, LootContainerQuery, LootContainerStatus, read_loot_container,
};
use p4kx_extraction::{Archive, find_entry};
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestContainer, TestEntry, TestRecord, TestStruct,
};

use super::super::LOOT_CONTAINERS;
use super::{Route, TestResult};

const ROOT: &str = "Data/ObjectContainers/PU/system/lootwitness/lootwitnesssystem.socpak";
const TARGET: &str = "Data/ObjectContainers/PU/witness/crate.socpak";
const CONFIG: &str = "00000000-0000-4000-8000-000000000055";

pub(super) fn read() -> TestResult<Route> {
    let (_folder, archive) = fixture()?;
    let entry = find_entry(&archive, ROOT).ok_or("the physical root Entry")?;
    let target = find_entry(&archive, TARGET).ok_or("the original container Entry")?;
    let query = LootContainerQuery::new(
        u32::try_from(entry.index())?,
        CONFIG.parse()?,
        target.path(),
        LootAncestorSelection::WithoutResolvedObject,
    );
    let selected = read_loot_container(&archive, query, None)?;
    assert_eq!(
        selected.status,
        LootContainerStatus::Found,
        "complete original scope"
    );
    let row = selected.row.ok_or("one typed physical Loot row")?;
    Route::queried_rows(
        LOOT_CONTAINERS,
        std::slice::from_ref(&row),
        "installed Normal parity, cold memory, source closure and scope enumeration remain open",
    )
}

fn fixture() -> TestResult<(tempfile::TempDir, Archive)> {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("Data.p4k");
    let database = DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("StarMapObject"))
        .record(TestRecord::new(
            1,
            "StarMapObject",
            "StarMapObject.Witness",
            "libs/foundry/records/starmap/pu/system/lootwitness/witness.xml",
        ))
        .build();
    let source = format!(r#"<Entity objectContainer="{TARGET}" multiConfigRef="{CONFIG}"/>"#);
    let root = TestContainer::new()
        .entry(TestEntry::stored(
            "lootwitnesssystem.xml",
            b"<ObjectContainer/>",
        ))
        .entry(TestEntry::stored("entdata/witness.entxml", source))
        .build();
    let target = TestContainer::new()
        .entry(TestEntry::stored("crate.xml", b"<ObjectContainer/>"))
        .build();
    ArchiveBuilder::v2()
        .entry(TestEntry::stored("Data/Game2.dcb", database))
        .entry(TestEntry::stored(ROOT, root))
        .entry(TestEntry::stored(TARGET, target))
        .write_to(&path)?;
    Ok((folder, Archive::open(path)?))
}
