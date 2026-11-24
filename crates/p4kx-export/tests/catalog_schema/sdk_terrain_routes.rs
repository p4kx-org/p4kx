// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::time::UNIX_EPOCH;

use p4kx_export::catalog_queries::terrain::{
    CatalogJsonNumber, Ecosystem, LooseArchivePathState, LooseDefinitionSelection as Selection,
    ShadingClass, read_ecosystems, read_shading_classes,
};
use p4kx_export::{CatalogOptions, build_catalog};
use p4kx_extraction::Archive;
use p4kx_testkit::{ArchiveBuilder, DataForgeBuilder, TestEntry};
use schemars::JsonSchema;
use serde::Serialize;
use serde_json::{Value, json};

use super::{Route, TestResult};

#[path = "sdk_geology_routes.rs"]
mod geology;

const ECOSYSTEMS: &str = "/definitions/ecosystems";
const SHADING: &str = "/definitions/shadingClasses";
pub(super) const DATASETS: [&str; 4] = [ECOSYSTEMS, SHADING, geology::SETS, geology::TEXTURE_SETS];

const ECO: &str = "Data/Libs/EcoSystems/fixture.eco";
const DAMAGED: &str = "Data/Libs/EcoSystems/damaged.eco";
const SHADE: &str = "Data/Libs/Planets/shading_classes/fixture.shadingclass";
const MEDIA: &str = "Data/Textures/fixture/terrain.dds";
const ECO_GUID: &str = "cccccccc-0000-0000-0000-000000000101";
const SHADE_GUID: &str = "cccccccc-0000-0000-0000-000000000102";
const RAW: &str = r#"{"slots":[1.00,null]}"#;

pub(super) fn read() -> TestResult<[Route; 4]> {
    let (_folder, archive) = open(
        [
            TestEntry::stored("Data/Game2.dcb", DataForgeBuilder::new().version(8).build()),
            TestEntry::stored(
                "Data/Libs/CosmoTools/LayerRecovery/LayerBackups.xml",
                b"<LayerBackups/>",
            ),
            TestEntry::stored(ECO, ecosystem()).encrypted(),
            TestEntry::stored(SHADE, shading_class()),
            media(),
        ]
        .into_iter()
        .chain(geology::definitions()),
    )?;
    let catalog: Value =
        serde_json::from_slice(build_catalog(&archive, &CatalogOptions::new(UNIX_EPOCH))?.json())?;
    let ecosystem = read_ecosystems(&archive, Selection::ArchivePath(ECO), None)?;
    assert!(
        ecosystem.is_selection_complete(),
        "the physical ecosystem owner was read"
    );
    let [row] = ecosystem.rows.as_slice() else {
        return Err("one scoped ecosystem".into());
    };
    verify_ecosystem(row)?;
    let shading = read_shading_classes(&archive, Selection::Guid(SHADE_GUID.parse()?), None)?;
    let [row] = shading.rows.as_slice() else {
        return Err("one scoped shading class".into());
    };
    verify_layer(row)?;
    let [geology_sets, geology_texture_sets] = geology::read(&archive, &catalog)?;
    Ok([
        parity(
            &catalog,
            ECOSYSTEMS,
            ecosystem.availability,
            &ecosystem.rows,
        )?,
        parity(&catalog, SHADING, shading.availability, &shading.rows)?,
        geology_sets,
        geology_texture_sets,
    ])
}

fn parity<Row: JsonSchema + Serialize>(
    catalog: &Value,
    path: &'static str,
    availability: p4kx_export::catalog_queries::DatasetAvailability,
    rows: &[Row],
) -> TestResult<Route> {
    assert_eq!(
        catalog.pointer(path),
        Some(&json!({"availability": availability, "rows": rows})),
        "selected canonical terrain rows and original origins match the normal Catalog"
    );
    Route::queried_rows(path, rows, None)
}

fn verify_ecosystem(row: &Ecosystem) -> TestResult {
    assert_eq!(
        row.name.value().map(String::as_str),
        Some("@fixture_raw_label"),
        "the protected label retains its original spelling"
    );
    let sound = row.sound_effect.value().ok_or("original sound settings")?;
    assert_eq!(
        sound
            .audio_rtpc_array_count
            .value()
            .copied()
            .map(p4kx_extraction::DecimalU64::get),
        Some(0),
        "the original audio array count is preserved"
    );
    assert_eq!(
        sound
            .audio_terrain_eccentricity
            .value()
            .map(CatalogJsonNumber::as_str),
        Some("2.00e-1"),
        "the original exponent and digits remain intact"
    );
    let [raw] = sound.additional_fields.as_slice() else {
        return Err("one uninterpreted sound declaration".into());
    };
    assert_eq!(
        raw.token.value().map(String::as_str),
        Some(RAW),
        "the unprojected member token is lossless"
    );
    assert_eq!(
        raw.token.origin().contains_encrypted_source(),
        Some(true),
        "the raw member retains its own payload evidence"
    );
    let link = &row
        .texture_links
        .as_ref()
        .ok_or("original texture selectors")?
        .climate;
    assert_eq!(
        link.state().value(),
        Some(&LooseArchivePathState::Resolved),
        "an original climate path uses directory evidence"
    );
    assert_eq!(
        link.targets().len(),
        1,
        "only the actual matching directory Entry is linked"
    );
    Ok(())
}

fn verify_layer(row: &ShadingClass) -> TestResult {
    assert_eq!(
        row.roughness.value().map(CatalogJsonNumber::as_str),
        Some("1.2500"),
        "the original roughness number is kept verbatim"
    );
    assert_eq!(
        row.roughness.origin().contains_encrypted_source(),
        Some(false),
        "the roughness source belongs to the plain owner"
    );
    let layer = row.ground_layer.value().ok_or("original ground layer")?;
    assert_eq!(
        layer.gloss.value().map(CatalogJsonNumber::as_str),
        Some("7"),
        "the authored integer gloss survives"
    );
    let selection = row
        .ground_texture
        .as_ref()
        .ok_or("original ground texture selector")?;
    assert_eq!(
        selection.state().value(),
        Some(&LooseArchivePathState::Resolved),
        "directory resolution does not open the unreadable texture payload"
    );
    assert_eq!(
        selection.targets().len(),
        1,
        "one actual directory candidate"
    );
    Ok(())
}

#[test]
fn exact_ecosystem_scope_ignores_damaged_sibling_and_requires_no_dataforge() -> TestResult {
    let (_folder, archive) = open(
        [
            TestEntry::stored(ECO, ecosystem()).encrypted(),
            TestEntry::stored(DAMAGED, ecosystem()).with_crc32c(0),
            media(),
        ]
        .into_iter()
        .chain(geology::definitions()),
    )?;
    let stored_path = ECO.replace('/', "\\");
    let index = archive
        .entries()
        .iter()
        .find(|entry| entry.path().eq_ignore_ascii_case(&stored_path))
        .ok_or("physical ecosystem Entry index")?
        .index();
    let exact = read_ecosystems(&archive, Selection::EntryIndex(u32::try_from(index)?), None)?;
    assert!(
        exact.is_selection_complete(),
        "a damaged sibling is outside the exact-index selection"
    );
    let [row] = exact.rows.as_slice() else {
        return Err("one exact ecosystem".into());
    };
    verify_ecosystem(row)?;
    let guid = read_ecosystems(&archive, Selection::Guid(ECO_GUID.parse()?), None)?;
    assert_eq!(
        guid.entries.failed_selection_candidates, 1,
        "GUID lookup cannot decide a damaged candidate"
    );
    assert!(
        !guid.is_selection_complete(),
        "the uncertain GUID scope stays incomplete"
    );
    assert_eq!(
        guid.rows.len(),
        1,
        "the known matching owner still survives"
    );
    Ok(())
}

fn open(entries: impl IntoIterator<Item = TestEntry>) -> TestResult<(tempfile::TempDir, Archive)> {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("terrain-fixture.p4k");
    entries
        .into_iter()
        .fold(ArchiveBuilder::v2(), ArchiveBuilder::entry)
        .write_to(&path)?;
    Ok((
        folder,
        Archive::open_with_key(path, Some(p4kx_testkit::test_key()))?,
    ))
}

fn ecosystem() -> String {
    format!(
        r#"{{"GUID":"{ECO_GUID}","name":"@fixture_raw_label","Textures":{{"ecoSystemClimateTexture":"{MEDIA}"}},"SoundEffect":{{"audioRTPCArray":[],"audioSwitchArray":[],"fAudioTerrainEccentricity":2.00e-1,"fixtureOpaque":{RAW}}}}}"#
    )
}

fn shading_class() -> String {
    format!(
        r#"{{"GUID":"{SHADE_GUID}","groundLayer":{{"textureName":"{MEDIA}","gloss":7}},"fRoughness":1.2500}}"#
    )
}

fn media() -> TestEntry {
    TestEntry::stored(MEDIA, b"unreadable synthetic media").with_crc32c(0)
}
