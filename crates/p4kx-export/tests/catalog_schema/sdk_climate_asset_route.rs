// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::time::UNIX_EPOCH;

use p4kx_export::catalog_queries::climate::{
    CatalogJsonNumber, ClimateAssetSnapshot, LooseArchivePathState,
    LooseDefinitionSelection as Selection, PlanetClimateAsset, TerrainGuidLinkState,
    read_climate_assets, read_climate_assets_from,
};
use p4kx_export::{CatalogError, CatalogOptions, build_catalog};
use p4kx_extraction::{Archive, DataForgeEntry, FieldOrigin};
use p4kx_testkit::{ArchiveBuilder, DataForgeBuilder, TestEntry, TestRecord, TestStruct};
use serde_json::{Value, json};

use super::{Route, TestResult};

pub(super) const DATASET: &str = "/definitions/climateAssets";
const OWNER: &str = "Data/Libs/Planets/climate_assets/fixture.climasset";
const MALFORMED: &str = "Data/Libs/Planets/climate_assets/malformed.climasset";
const DAMAGED: &str = "Data/Libs/Planets/climate_assets/damaged.climasset";
const RECORD_OWNER: &str = "Data/Libs/Planets/climate_assets/record.climasset";
const RESOURCE: &str = "Data/Libs/Planets/resources/fixture.resource";
const SHADE: &str = "Data/Libs/Planets/shading_classes/fixture.shadingclass";
const GEOMETRY: &str = "Data/Objects/Fixtures/terrain.cgf";
const OWNER_GUID: &str = "eeeeeeee-0000-0000-0000-000000000001";
const RECORD_OWNER_GUID: &str = "eeeeeeee-0000-0000-0000-000000000002";
const RESOURCE_GUID: &str = "eeeeeeee-0000-0000-0000-000000000003";
const SHADE_GUID: &str = "eeeeeeee-0000-0000-0000-000000000004";
const SETUP_GUID: &str = "00000000-0000-0000-0000-000000000007";
const ZERO: &str = "00000000-0000-0000-0000-000000000000";

pub(super) fn read() -> TestResult<Route> {
    let (_folder, archive) = open(parity_entries())?;
    let built = build_catalog(&archive, &CatalogOptions::new(UNIX_EPOCH))?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    let scoped = read_climate_assets(&archive, Selection::ArchivePath(OWNER), None)?;
    verify_dependencies(&scoped)?;
    let expected = catalog
        .pointer(DATASET)
        .ok_or("canonical climate-asset table")?;
    assert_eq!(
        table(&scoped),
        *expected,
        "exact selection has canonical values and origins"
    );
    let all = read_climate_assets(&archive, Selection::All, None)?;
    assert_eq!(
        table(&all),
        *expected,
        "family selection has canonical values and origins"
    );

    let forge = DataForgeEntry::read_until(&archive, &|| false)?;
    let parsed =
        read_climate_assets_from(&archive, &forge, Selection::Guid(OWNER_GUID.parse()?), None)?;
    assert_eq!(
        table(&parsed),
        *expected,
        "preloaded DataForge preserves the same field evidence"
    );

    Route::queried_rows(DATASET, &scoped.rows, None)
}

fn table(snapshot: &ClimateAssetSnapshot) -> Value {
    json!({"availability": snapshot.availability, "rows": snapshot.rows})
}

fn only_row(snapshot: &ClimateAssetSnapshot) -> TestResult<&PlanetClimateAsset> {
    let [row] = snapshot.rows.as_slice() else {
        return Err("one original climate asset".into());
    };
    Ok(row)
}

fn verify_dependencies(snapshot: &ClimateAssetSnapshot) -> TestResult {
    assert!(
        snapshot.is_selection_complete(),
        "the selected physical owner is reachable"
    );
    let row = only_row(snapshot)?;
    assert_eq!(
        row.envelope
            .min_annual_rainfall
            .value()
            .map(CatalogJsonNumber::as_str),
        Some("0.1000000003"),
        "the selected owner retains the authored numeric spelling"
    );
    assert_eq!(
        row.envelope
            .min_annual_rainfall
            .origin()
            .contains_encrypted_source(),
        Some(false),
        "the owner value comes from the clear climate definition"
    );
    let shading = row
        .appearance
        .shading_links
        .as_ref()
        .ok_or("authored shading links")?;
    assert_eq!(
        shading.default.state.value(),
        Some(&TerrainGuidLinkState::Resolved),
        "the authored default shading GUID resolves"
    );
    assert_eq!(
        shading.far.state.value(),
        Some(&TerrainGuidLinkState::Null),
        "the explicit null shading selector survives"
    );
    assert_encrypted_use(snapshot, shading.default.state.origin());
    verify_sub_asset(snapshot, row)
}

fn verify_sub_asset(snapshot: &ClimateAssetSnapshot, row: &PlanetClimateAsset) -> TestResult {
    let [asset, empty] = row
        .sub_assets
        .value()
        .ok_or("authored sub-asset slots")?
        .as_slice()
    else {
        return Err("two original sub-asset slots".into());
    };
    assert_eq!(
        empty.state(),
        p4kx_extraction::SourceValueState::Null,
        "the second original sub-asset slot is null"
    );
    let asset = asset.value().ok_or("first sub-asset")?;
    let resource = asset
        .harvestable_resource_link
        .as_ref()
        .ok_or("authored resource link")?;
    assert_eq!(
        resource.state.value(),
        Some(&TerrainGuidLinkState::Resolved),
        "the sub-asset resource GUID resolves to its loose definition"
    );
    assert_encrypted_use(snapshot, resource.state.origin());
    assert_eq!(
        asset.harvestable_setup.state().value(),
        Some(&LooseArchivePathState::Resolved),
        "the authored setup GUID resolves to a readable Record header"
    );
    let [header] = asset.harvestable_setup.candidates() else {
        return Err("one original harvestable setup header".into());
    };
    assert_eq!(
        header.source_type().value().map(String::as_str),
        Some("HarvestableSetup"),
        "the linked header has the expected Record family"
    );
    assert_encrypted_use(snapshot, header.guid().origin());
    assert_eq!(
        asset.geometry_path.value().map(String::as_str),
        Some(GEOMETRY),
        "the original geometry path is preserved"
    );
    Ok(())
}

fn assert_encrypted_use(snapshot: &ClimateAssetSnapshot, origin: &FieldOrigin) {
    assert_eq!(
        origin.contains_encrypted_source(),
        Some(true),
        "the linked value identifies its encrypted source"
    );
    let references = origin.source_uses_refs();
    assert!(
        !references.is_empty()
            && references.iter().all(|reference| {
                snapshot
                    .source_inventory
                    .iter_uses()
                    .any(|used| used.id() == reference)
            }),
        "every linked field operand is resolvable in this snapshot's inventory"
    );
}

#[test]
fn exact_and_guid_selection_skip_unrelated_record_requirements() -> TestResult {
    let states = format!(
        r#"{{"GUID":"{OWNER_GUID}","subAssets":[{{}},{{"entityClass":null,"harvestableSetupRecord":""}},{{"entityClass":"{ZERO}","harvestableSetupRecord":"invalid","harvestableResource":"{RESOURCE_GUID}"}}]}}"#
    );
    let required = format!(
        r#"{{"GUID":"{RECORD_OWNER_GUID}","subAssets":[{{"harvestableSetupRecord":"{SETUP_GUID}"}}]}}"#
    );
    let (_folder, archive) = open([
        TestEntry::stored(OWNER, states),
        TestEntry::stored(RECORD_OWNER, required),
        TestEntry::stored(MALFORMED, r#"{"GUID":"invalid"}"#),
        TestEntry::stored(DAMAGED, b"unreadable owner").with_crc32c(0),
        TestEntry::stored(RESOURCE, b"unreadable original resource").with_crc32c(0),
    ])?;
    let stored = OWNER.replace('/', "\\");
    let entry = archive
        .entries()
        .iter()
        .find(|entry| entry.path().eq_ignore_ascii_case(&stored))
        .ok_or("original owner directory Entry")?;
    let index = u32::try_from(entry.index())?;
    let exact = read_climate_assets(&archive, Selection::EntryIndex(index), None)?;
    assert!(
        exact.is_selection_complete(),
        "the exact selected climate owner is readable without a DCB"
    );
    verify_empty_record_targets(only_row(&exact)?)?;
    let selected = read_climate_assets(&archive, Selection::Guid(OWNER_GUID.parse()?), None)?;
    assert_eq!(selected.entries.failed_selection_candidates, 2);
    assert!(
        !selected.is_selection_complete(),
        "malformed and unreadable owner candidates remain visible"
    );
    assert_eq!(
        table(&selected),
        table(&exact),
        "unrelated candidate problems preserve the known row"
    );
    assert!(
        matches!(
            read_climate_assets(&archive, Selection::ArchivePath(RECORD_OWNER), None),
            Err(CatalogError::Load(_))
        ),
        "a genuine nonzero Record selector exposes missing DataForge"
    );
    Ok(())
}

fn verify_empty_record_targets(row: &PlanetClimateAsset) -> TestResult {
    let [missing, empty, invalid] = row.sub_assets.value().ok_or("original slots")?.as_slice()
    else {
        return Err("three original sub-assets".into());
    };
    let missing = missing.value().ok_or("absent Record selector")?;
    let empty = empty.value().ok_or("empty Record selector")?;
    let invalid = invalid.value().ok_or("invalid Record selector")?;
    assert_eq!(
        missing.entity.state().value(),
        Some(&LooseArchivePathState::Absent),
        "an absent Entity selector remains absent"
    );
    assert_eq!(
        empty.entity.state().value(),
        Some(&LooseArchivePathState::Null),
        "an explicitly null Entity selector remains null"
    );
    assert_eq!(
        empty.harvestable_setup.state().value(),
        Some(&LooseArchivePathState::Empty),
        "an explicitly empty setup selector requires no DataForge"
    );
    assert_eq!(
        invalid.entity.state().value(),
        Some(&LooseArchivePathState::Empty),
        "the zero Entity GUID denotes an empty target"
    );
    assert_eq!(
        invalid.harvestable_setup.state().value(),
        Some(&LooseArchivePathState::Invalid),
        "invalid setup GUID text remains visible without opening DataForge"
    );
    let link = invalid
        .harvestable_resource_link
        .as_ref()
        .ok_or("resource relation")?;
    assert_eq!(
        link.state.value(),
        Some(&TerrainGuidLinkState::Unavailable),
        "an unreadable requested resource cannot prove absence"
    );
    Ok(())
}

#[test]
fn a_foreign_preloaded_dataforge_is_rejected() -> TestResult {
    let (_owner_folder, archive) = open(parity_entries())?;
    let (_foreign_folder, foreign) = open([TestEntry::stored("Data/Game2.dcb", forge())])?;
    let entry = DataForgeEntry::read_until(&foreign, &|| false)?;
    assert!(
        matches!(
            read_climate_assets_from(&archive, &entry, Selection::All, None),
            Err(CatalogError::SourceOriginMismatch)
        ),
        "catalog field provenance requires the original archive DataForge"
    );
    Ok(())
}

fn parity_entries() -> [TestEntry; 6] {
    let climate = format!(
        r#"{{"GUID":"{OWNER_GUID}","minAnnualRainfall":0.1000000003,"shadingClassGUID":"{SHADE_GUID}","shadingClassFarGUID":null,"subAssets":[{{"harvestableResource":"{RESOURCE_GUID}","harvestableSetupRecord":"{SETUP_GUID}","geomPath":"{GEOMETRY}"}},null],"growthStageBuckets":[{{"shadeDensity":2.00}}]}}"#
    );
    [
        TestEntry::stored("Data/Game2.dcb", forge()).encrypted(),
        TestEntry::stored(
            "Data/Libs/CosmoTools/LayerRecovery/LayerBackups.xml",
            b"<LayerBackups/>",
        ),
        TestEntry::stored(OWNER, climate),
        TestEntry::stored(
            RESOURCE,
            format!(r#"{{"guid":"{RESOURCE_GUID}","name":"fixture","bMinable":true}}"#),
        )
        .encrypted(),
        TestEntry::stored(SHADE, format!(r#"{{"GUID":"{SHADE_GUID}"}}"#)).encrypted(),
        TestEntry::stored(GEOMETRY, b"unreadable unrelated geometry").with_crc32c(0),
    ]
}

fn forge() -> Vec<u8> {
    DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("HarvestableSetup"))
        .record(TestRecord::new(
            7,
            "HarvestableSetup",
            "Fixture",
            "fixtures/setup.xml",
        ))
        .build()
}

fn open(entries: impl IntoIterator<Item = TestEntry>) -> TestResult<(tempfile::TempDir, Archive)> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("climate-fixture.p4k");
    entries
        .into_iter()
        .fold(ArchiveBuilder::v2(), ArchiveBuilder::entry)
        .write_to(&path)?;
    Ok((
        directory,
        Archive::open_with_key(path, Some(p4kx_testkit::test_key()))?,
    ))
}
