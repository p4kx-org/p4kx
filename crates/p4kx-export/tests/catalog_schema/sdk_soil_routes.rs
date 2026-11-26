// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::time::UNIX_EPOCH;

use p4kx_export::catalog_queries::soils::{
    CatalogJsonNumber, LooseArchivePathState, LooseDefinitionSelection as Selection, Soil,
    SoilSnapshot, TerrainGuidLinkState, read_soil_texture_sets_with_provenance_mode, read_soils,
    read_soils_with_provenance_mode,
};
use p4kx_export::{CatalogOptions, ExportDiagnosticCode, Kind, build_catalog};
use p4kx_extraction::Archive;
use p4kx_testkit::{ArchiveBuilder, DataForgeBuilder, TestEntry};
use serde_json::{Value, json};

use crate::sdk_full_proof as full_proof;

use super::{Route, TestResult};

const SOILS: &str = "/definitions/soils";
const TEXTURE_SETS: &str = "/definitions/soilTextureSets";
const OWNER: &str = "Data/Libs/Planets/soil_sets/selected.soil";
const ALIAS: &str = "Data/Libs/Planets/soil_sets/alias.soil";
const DAMAGED_OWNER: &str = "Data/Libs/Planets/soil_sets/damaged.soil";
const TARGET: &str = "Data/Libs/Planets/soil_sets/fixture.soiltextureset";
const WRONG_FAMILY: &str = "Data/Libs/Planets/geology_assets/fixture.geoasset";
const DAMAGED_TARGET: &str = "Data/Libs/Planets/soil_sets/damaged.soiltextureset";
const MEDIA: &str = "Data/Textures/fixture/terrain.dds";
const OWNER_GUID: &str = "dddddddd-0000-0000-0000-000000000001";
const TARGET_GUID: &str = "dddddddd-0000-0000-0000-000000000002";

pub(super) fn read() -> TestResult<[Route; 2]> {
    let (_folder, archive) = open(normal_entries())?;
    let built = build_catalog(
        &archive,
        CatalogOptions::new(UNIX_EPOCH).with_full_provenance(),
    )?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    let soils = read_soils_with_provenance_mode(
        &archive,
        Selection::ArchivePath(OWNER),
        full_proof::MODE,
        None,
    )?;
    let soil = verify_single_owner(&soils)?;
    verify_resolved_link(soil)?;
    assert_eq!(
        json!({"availability": soils.availability, "rows": soils.rows}),
        *catalog.pointer(SOILS).ok_or("normal soils table")?,
        "the selected single owner with no aliases matches the normal Catalog row"
    );
    let sets = read_soil_texture_sets_with_provenance_mode(
        &archive,
        Selection::Guid(TARGET_GUID.parse()?),
        full_proof::MODE,
        None,
    )?;
    let [set] = sets.rows.as_slice() else {
        return Err("one original soil texture set".into());
    };
    let ground = set
        .ground_textures
        .as_ref()
        .ok_or("ground texture directory links")?;
    let [slot] = ground.slots.as_slice() else {
        return Err("one authored texture path".into());
    };
    assert_eq!(
        slot.target.state().value(),
        Some(&LooseArchivePathState::Resolved),
        "directory can resolve the path without texture payload"
    );
    assert_eq!(
        json!({"availability": sets.availability, "rows": sets.rows}),
        *catalog
            .pointer(TEXTURE_SETS)
            .ok_or("normal soil texture-set table")?,
        "the canonical texture-set table matches without reading media"
    );
    Ok([
        Route::queried_rows(SOILS, &soils.rows, None)?,
        Route::queried_rows(TEXTURE_SETS, &sets.rows, None)?,
    ])
}

fn verify_resolved_link(soil: &Soil) -> TestResult {
    let link = soil
        .texture_set
        .as_ref()
        .ok_or("original soil texture-set link")?;
    assert_eq!(
        link.state.value(),
        Some(&TerrainGuidLinkState::Resolved),
        "the one actual soil texture-set GUID resolves"
    );
    assert_eq!(
        link.state.origin().contains_encrypted_source(),
        Some(true),
        "only the target payload is encrypted"
    );
    assert!(
        !link.state.origin().source_set_complete(),
        "an origin contains only the actual matching candidates and owner"
    );
    assert_eq!(
        link.candidates.len(),
        2,
        "both same-GUID candidates keep their original families"
    );
    assert!(
        link.candidates
            .iter()
            .any(|candidate| candidate.kind == Kind::GeologyAssets),
        "foreign-family evidence is retained even with a unique soil match"
    );
    Ok(())
}

fn verify_single_owner(snapshot: &SoilSnapshot) -> TestResult<&Soil> {
    let [row] = snapshot.rows.as_slice() else {
        return Err("one original soil".into());
    };
    assert!(
        snapshot.is_selection_complete(),
        "the selected owner was read"
    );
    assert_eq!(
        row.ocean_secondary_colour_weight
            .value()
            .map(CatalogJsonNumber::as_str),
        Some("1.0000"),
        "the world material weight preserves its original decimal spelling"
    );
    assert_eq!(
        row.ocean_secondary_colour_weight
            .origin()
            .contains_encrypted_source(),
        Some(false),
        "the original numeric field is from an unencrypted owner"
    );
    Ok(row)
}

#[test]
fn exact_soil_owner_is_independent_of_damaged_owners_and_target_uncertainty() -> TestResult {
    let (_folder, archive) = open([
        TestEntry::stored(OWNER, owner()),
        TestEntry::stored(DAMAGED_OWNER, owner()).with_crc32c(0),
        TestEntry::stored(TARGET, identity(TARGET_GUID)).encrypted(),
        TestEntry::stored(DAMAGED_TARGET, identity(OWNER_GUID)).with_crc32c(0),
    ])?;
    let stored_path = OWNER.replace('/', "\\");
    let index = archive
        .entries()
        .iter()
        .find(|entry| entry.path().eq_ignore_ascii_case(&stored_path))
        .ok_or("physical original soil Entry")?
        .index();
    let direct = read_soils(&archive, Selection::EntryIndex(u32::try_from(index)?), None)?;
    let soil = verify_single_owner(&direct)?;
    let link = soil
        .texture_set
        .as_ref()
        .ok_or("original soil texture-set link")?;
    assert_eq!(
        link.state.value(),
        Some(&TerrainGuidLinkState::Unavailable),
        "an unreadable possible texture-set alias leaves the link undecidable"
    );
    assert_eq!(
        link.candidates.len(),
        1,
        "the original readable candidate survives unknown membership elsewhere"
    );
    assert_eq!(
        link.state.origin().contains_encrypted_source(),
        Some(true),
        "positive evidence retains target encryption beside uncertainty"
    );
    assert!(
        !link.state.origin().source_set_complete(),
        "the owner verdict is not link completeness"
    );
    let guid = read_soils(&archive, Selection::Guid(OWNER_GUID.parse()?), None)?;
    assert_eq!(
        guid.rows.len(),
        1,
        "the admitted original readable owner survives"
    );
    assert_eq!(
        guid.entries.failed_selection_candidates, 1,
        "unreadable owner might have matched the declared GUID"
    );
    assert!(
        !guid.is_selection_complete(),
        "the GUID selection cannot prove all physical aliases"
    );
    Ok(())
}

#[test]
fn direct_soil_selection_does_not_claim_global_guid_uniqueness() -> TestResult {
    let (_folder, archive) = open([
        TestEntry::stored(OWNER, owner()),
        TestEntry::stored(ALIAS, owner()),
        TestEntry::stored(TARGET, identity(TARGET_GUID)),
    ])?;
    let selected = read_soils(&archive, Selection::ArchivePath(OWNER), None)?;
    let row = verify_single_owner(&selected)?;
    assert!(
        !duplicate(row),
        "the direct scope does not report an unobserved owner alias"
    );
    let guid = read_soils(&archive, Selection::Guid(OWNER_GUID.parse()?), None)?;
    assert_eq!(
        guid.rows.len(),
        2,
        "GUID scope retains both physical owners"
    );
    assert!(
        guid.rows.iter().all(duplicate),
        "scope-wide duplicate issue is reported for every selected alias"
    );
    Ok(())
}

fn duplicate(row: &Soil) -> bool {
    row.issues.iter().any(|issue| {
        issue.code() == ExportDiagnosticCode::AmbiguousIdentity && issue.field() == "GUID"
    })
}

fn open(entries: impl IntoIterator<Item = TestEntry>) -> TestResult<(tempfile::TempDir, Archive)> {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("soil-fixture.p4k");
    entries
        .into_iter()
        .fold(ArchiveBuilder::v2(), ArchiveBuilder::entry)
        .write_to(&path)?;
    Ok((
        folder,
        Archive::open_with_key(path, Some(p4kx_testkit::test_key()))?,
    ))
}

fn normal_entries() -> [TestEntry; 6] {
    let terrain =
        format!(r#"{{"GUID":"{TARGET_GUID}","groundLayers":[{{"textureName":"{MEDIA}"}}]}}"#);
    [
        TestEntry::stored("Data/Game2.dcb", DataForgeBuilder::new().version(8).build()),
        TestEntry::stored(
            "Data/Libs/CosmoTools/LayerRecovery/LayerBackups.xml",
            b"<LayerBackups/>",
        ),
        TestEntry::stored(OWNER, owner()),
        TestEntry::stored(TARGET, terrain).encrypted(),
        TestEntry::stored(WRONG_FAMILY, identity(TARGET_GUID)),
        TestEntry::stored(MEDIA, b"unreadable synthetic media").with_crc32c(0),
    ]
}

fn owner() -> String {
    format!(
        r#"{{"GUID":"{OWNER_GUID}","soilTextureSetGUID":"{TARGET_GUID}","oceanSecondaryColourWeight":1.0000}}"#
    )
}

fn identity(guid: &str) -> String {
    format!(r#"{{"GUID":"{guid}"}}"#)
}
