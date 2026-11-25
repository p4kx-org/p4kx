// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::time::UNIX_EPOCH;

use p4kx_export::catalog_queries::LooseDefinitionSnapshot;
use p4kx_export::catalog_queries::creatures::{
    PlanetCreature, read_creatures, read_creatures_with_provenance_mode,
};
use p4kx_export::catalog_queries::terrain::{
    CatalogJsonNumber, LooseArchivePathState, LooseDefinitionSelection as Selection,
    LooseRecordLink, PlanetGeologyAsset, SunShadowMode, read_geology_assets,
    read_geology_assets_with_provenance_mode,
};
use p4kx_export::{CatalogError, CatalogOptions, build_catalog};
use p4kx_extraction::{Archive, DataForgeEntry};
use p4kx_testkit::{ArchiveBuilder, DataForgeBuilder, TestEntity, TestEntry, entity_schema};
use schemars::JsonSchema;
use serde::Serialize;
use serde_json::{Value, json};

use crate::sdk_full_proof as full_proof;

use super::{Route, TestResult};

const GEOLOGY: &str = "/definitions/geologyAssets";
const CREATURES: &str = "/definitions/creatures";
pub(super) const DATASETS: [&str; 2] = [GEOLOGY, CREATURES];

const GEO: &str = "Data/Libs/Planets/geology_assets/fixture.geoasset";
const FAUNA: &str = "Data/Libs/Planets/creatures/fixture.creature";
const DAMAGED_GEO: &str = "Data/Libs/Planets/geology_assets/damaged.geoasset";
const DAMAGED_FAUNA: &str = "Data/Libs/Planets/creatures/damaged.creature";
const RECORD_GEO: &str = "Data/Libs/Planets/geology_assets/record.geoasset";
const RECORD_FAUNA: &str = "Data/Libs/Planets/creatures/record.creature";
const MESH: &str = "Data/Objects/Fixtures/stone.cgf";
const MATERIAL: &str = "Data/Objects/Fixtures/stone.mtl";
const GEO_GUID: &str = "bbbbbbbb-0000-0000-0000-000000000001";
const FAUNA_GUID: &str = "bbbbbbbb-0000-0000-0000-000000000002";
const OTHER_GUID: &str = "bbbbbbbb-0000-0000-0000-000000000003";
const ENTITY_GUID: &str = "00000000-0000-0000-0000-000000000010";
const RAW: &str = r#"{"coordinates":[1.00,null]}"#;

pub(super) fn read() -> TestResult<[Route; 2]> {
    let (_folder, archive) = open(normal_entries())?;
    let full = build_catalog(
        &archive,
        CatalogOptions::new(UNIX_EPOCH).with_full_provenance(),
    )?;
    let full: Value = serde_json::from_slice(full.json())?;
    let geology = read_geology_assets_with_provenance_mode(
        &archive,
        Selection::ArchivePath(GEO),
        full_proof::MODE,
        None,
    )?;
    let creature = read_creatures_with_provenance_mode(
        &archive,
        Selection::Guid(FAUNA_GUID.parse()?),
        full_proof::MODE,
        None,
    )?;
    let [asset] = geology.rows.as_slice() else {
        return Err("one original geology asset".into());
    };
    let [fauna] = creature.rows.as_slice() else {
        return Err("one original creature".into());
    };
    verify_geology(asset)?;
    verify_creature(fauna)?;

    let entry = DataForgeEntry::read_until(&archive, &|| false)?;
    let session = full_proof::from_entry(&archive, &entry, full_proof::NO_LANGUAGES, None)?;
    let parsed = session.parsed(None)?;
    let all_geology = parsed.read_geology_assets(Selection::All, None)?;
    let all_creatures = parsed.read_creatures(Selection::All, None)?;
    assert_eq!(
        table(&geology),
        table(&all_geology),
        "selected geology uses the same Record evidence"
    );
    assert_eq!(
        table(&creature),
        table(&all_creatures),
        "selected fauna uses the same Record evidence"
    );
    Ok([
        parity(&full, &geology, GEOLOGY)?,
        parity(&full, &creature, CREATURES)?,
    ])
}

fn table<Row: Serialize>(snapshot: &LooseDefinitionSnapshot<Row>) -> Value {
    json!({"availability": snapshot.availability, "rows": snapshot.rows})
}

fn parity<Row: Serialize + JsonSchema>(
    catalog: &Value,
    snapshot: &LooseDefinitionSnapshot<Row>,
    dataset: &'static str,
) -> TestResult<Route> {
    assert!(
        snapshot.is_selection_complete(),
        "selected owner scope is complete"
    );
    assert_eq!(
        catalog.pointer(dataset),
        Some(&table(snapshot)),
        "scoped and canonical rows, including field origins, are identical"
    );
    Route::queried_rows(dataset, &snapshot.rows, None)
}

fn verify_geology(asset: &PlanetGeologyAsset) -> TestResult {
    assert!(
        matches!(
            asset.geometry.sun_shadow_mode.value(),
            Some(SunShadowMode::Numeric(value)) if value.as_str() == "2"
        ),
        "the original numeric shadow representation survives"
    );
    let [raw] = asset.additional_fields.as_slice() else {
        return Err("one original raw member".into());
    };
    assert_eq!(
        raw.token.value().map(String::as_str),
        Some(RAW),
        "the unprojected field retains its original token"
    );
    assert_eq!(
        asset.references.geometry.state().value(),
        Some(&LooseArchivePathState::Resolved),
        "the authored geometry is resolved without opening its body"
    );
    assert_eq!(
        asset.references.mineable_material.state().value(),
        Some(&LooseArchivePathState::Resolved),
        "the authored mineable material is resolved without opening its body"
    );
    verify_entity(&asset.entity)
}

fn verify_creature(fauna: &PlanetCreature) -> TestResult {
    assert_eq!(
        fauna.need_vertical_offset.value(),
        Some(&false),
        "the authored false flag stays present"
    );
    assert_eq!(
        fauna.slope_tolerance.value().map(CatalogJsonNumber::as_str),
        Some("-1.2300e+2"),
        "the source numeric token preserves its original precision"
    );
    assert_eq!(
        fauna.slope_tolerance.origin().contains_encrypted_source(),
        Some(false),
        "the numeric value is from the plain owner"
    );
    verify_entity(&fauna.entity)
}

fn verify_entity(link: &LooseRecordLink) -> TestResult {
    assert_eq!(
        link.state().value(),
        Some(&LooseArchivePathState::Resolved),
        "the authored Entity class GUID resolves"
    );
    let [record] = link.candidates() else {
        return Err("one linked Entity class header".into());
    };
    assert_eq!(
        record.source_type().value().map(String::as_str),
        Some("EntityClassDefinition"),
        "the candidate has the requested Record type"
    );
    assert_eq!(
        record.guid().origin().contains_encrypted_source(),
        Some(true),
        "the header is from the encrypted DataForge"
    );
    Ok(())
}

#[test]
fn scoped_owner_and_directory_paths_are_available_without_dataforge() -> TestResult {
    let (_folder, archive) = open(isolation_entries())?;
    verify_geology_scope(&archive)?;
    verify_creature_scope(&archive)?;
    assert!(
        matches!(
            read_geology_assets(&archive, Selection::ArchivePath(RECORD_GEO), None),
            Err(CatalogError::Load(_))
        ),
        "the valid nonzero Entity class needs the real DataForge"
    );
    assert!(
        matches!(
            read_creatures(&archive, Selection::ArchivePath(RECORD_FAUNA), None),
            Err(CatalogError::Load(_))
        ),
        "the valid nonzero Entity class needs the real DataForge"
    );
    Ok(())
}

fn verify_geology_scope(archive: &Archive) -> TestResult {
    let stored = GEO.replace('/', "\\");
    let index = archive
        .entries()
        .iter()
        .find(|entry| entry.path().eq_ignore_ascii_case(&stored))
        .ok_or("original geology Entry index")?
        .index();
    let scope = read_geology_assets(archive, Selection::EntryIndex(u32::try_from(index)?), None)?;
    assert!(
        scope.is_selection_complete(),
        "exact selection skips unrelated geology owners"
    );
    let [asset] = scope.rows.as_slice() else {
        return Err("one exact geology owner".into());
    };
    assert_eq!(
        asset.entity.state().value(),
        Some(&LooseArchivePathState::Empty),
        "the empty authored GUID needs no DataForge"
    );
    assert_eq!(
        asset.references.geometry.state().value(),
        Some(&LooseArchivePathState::Resolved),
        "the directory name is available independently of its payload"
    );
    Ok(())
}

fn verify_creature_scope(archive: &Archive) -> TestResult {
    let owner = read_creatures(archive, Selection::ArchivePath(FAUNA), None)?;
    assert!(
        owner.is_selection_complete(),
        "path selection skips unrelated creature owners"
    );
    let [definition] = owner.rows.as_slice() else {
        return Err("one exact creature owner".into());
    };
    assert_eq!(
        definition.entity.state().value(),
        Some(&LooseArchivePathState::Null),
        "the explicit null needs no DataForge"
    );
    let by_guid = read_creatures(archive, Selection::Guid(FAUNA_GUID.parse()?), None)?;
    assert_eq!(
        by_guid.entries.failed_selection_candidates, 1,
        "the unreadable owner stays in the GUID selection accounting"
    );
    assert!(
        !by_guid.is_selection_complete(),
        "the bad owner makes GUID selection undecidable"
    );
    assert_eq!(
        table(&by_guid),
        table(&owner),
        "the known original row survives the damaged sibling"
    );
    Ok(())
}

fn isolation_entries() -> [TestEntry; 7] {
    let geology = format!(r#"{{"GUID":"{GEO_GUID}","entityClass":"","geomPath":"{MESH}"}}"#);
    let fauna =
        format!(r#"{{"guid":"{FAUNA_GUID}","entityClass":null,"needVerticalOffset":true}}"#);
    let record_geo = format!(r#"{{"GUID":"{OTHER_GUID}","entityClass":"{ENTITY_GUID}"}}"#);
    let record_fauna = format!(r#"{{"guid":"{OTHER_GUID}","entityClass":"{ENTITY_GUID}"}}"#);
    [
        TestEntry::stored(GEO, geology),
        TestEntry::stored(FAUNA, fauna),
        TestEntry::stored(RECORD_GEO, record_geo),
        TestEntry::stored(RECORD_FAUNA, record_fauna),
        TestEntry::stored(DAMAGED_GEO, "{}").with_crc32c(0),
        TestEntry::stored(DAMAGED_FAUNA, "{}").with_crc32c(0),
        TestEntry::stored(MESH, "unreadable original geometry").with_crc32c(0),
    ]
}

fn normal_entries() -> [TestEntry; 6] {
    let forge = entity_schema(DataForgeBuilder::new().version(8))
        .record(TestEntity::new(0x10, "Fixture", "entities/scitem/fixture.xml").record())
        .build();
    let geology = format!(
        r#"{{"GUID":"{GEO_GUID}","entityClass":"{ENTITY_GUID}","geomPath":"{MESH}","Mineable_Material_path":"{MATERIAL}","SunShadowMode":2,"fixtureOpaque":{RAW}}}"#
    );
    let fauna = format!(
        r#"{{"guid":"{FAUNA_GUID}","entityClass":"{ENTITY_GUID}","needVerticalOffset":false,"slopeTolerance":-1.2300e+2}}"#
    );
    [
        TestEntry::stored("Data/Game2.dcb", forge).encrypted(),
        TestEntry::stored(
            "Data/Libs/CosmoTools/LayerRecovery/LayerBackups.xml",
            b"<LayerBackups/>",
        ),
        TestEntry::stored(GEO, geology),
        TestEntry::stored(FAUNA, fauna),
        TestEntry::stored(MESH, "unreadable original geometry").with_crc32c(0),
        TestEntry::stored(MATERIAL, "unreadable original material").with_crc32c(0),
    ]
}

fn open(entries: impl IntoIterator<Item = TestEntry>) -> TestResult<(tempfile::TempDir, Archive)> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("planet-assets-fixture.p4k");
    entries
        .into_iter()
        .fold(ArchiveBuilder::v2(), ArchiveBuilder::entry)
        .write_to(&path)?;
    Ok((
        directory,
        Archive::open_with_key(path, Some(p4kx_testkit::test_key()))?,
    ))
}
