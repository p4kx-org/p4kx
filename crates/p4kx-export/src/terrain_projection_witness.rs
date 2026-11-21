// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::error::Error;
use std::io::Read as _;
use std::time::UNIX_EPOCH;

use p4kx_extraction::{Archive, ArchiveWatch, Filter};
use p4kx_testkit::real_archive_path;
use serde_json::Value;

use super::planet_tech_definitions;
use crate::CatalogOptions;
use crate::build::original_input::test_support::with_archive_input;
use crate::catalog_read::original::EntryCapture;
use crate::export_ledger::ExportLedger;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and P4KX_TERRAIN_PROJECTION_PATHS, then pass --ignored"]
fn installed_terrain_projection_pairs_original_values_and_declared_links() -> TestResult {
    let archive = Archive::open(real_archive_path()?)?;
    let build = archive.build().ok_or("original Build ID")?;
    assert_eq!(build.change_number(), "13189690", "exact PTU build");
    let file =
        std::env::var_os("P4KX_TERRAIN_PROJECTION_PATHS").ok_or("private family selectors")?;
    let selectors = std::fs::read_to_string(file)?;
    let selected: Vec<_> = selectors.lines().collect();
    assert_eq!(
        selected.len(),
        7,
        "six terrain families and one climate definition"
    );
    let watch = ArchiveWatch::new(&archive);
    let input = read_original(&archive, &watch, &selected)?;
    with_archive_input(&archive, |forge, original| {
        let capture = EntryCapture::for_input(original);
        for &source_path in &selected {
            watch.poll()?;
            let filter = Filter::parse(source_path)?;
            let mut ledger = ExportLedger::default();
            let mut options = CatalogOptions::new(UNIX_EPOCH);
            options.filter = Some(&filter);
            let table = planet_tech_definitions::read(
                &archive,
                forge,
                original,
                &capture,
                &mut ledger,
                &options,
            )?;
            let expected = input.get(source_path).ok_or("exact original JSON")?;
            verify_row(source_path, expected, &serde_json::to_value(table)?)?;
        }
        Ok(())
    })?;
    watch.check()?;
    Ok(())
}

fn read_original<'a>(
    archive: &Archive,
    watch: &ArchiveWatch,
    selected: &[&'a str],
) -> TestResult<HashMap<&'a str, Value>> {
    let mut originals = HashMap::new();
    for &path in selected {
        watch.poll()?;
        let key = crate::starmap_index::key(path);
        let entry = archive
            .entries()
            .iter()
            .find(|entry| crate::starmap_index::key(entry.path()) == key)
            .ok_or("exact original Entry selection")?;
        let mut payload = Vec::new();
        archive
            .open_entry_verified(entry)?
            .read_to_end(&mut payload)?;
        let parsed = serde_json::from_slice::<Value>(&payload)?;
        originals.insert(path, parsed);
    }
    Ok(originals)
}

fn verify_row(source_path: &str, expected: &Value, catalog: &Value) -> TestResult {
    let table = family_table(source_path).ok_or("private witness family")?;
    let rows = catalog[table]["rows"]
        .as_array()
        .ok_or("family table rows")?;
    let [row] = rows.as_slice() else {
        return Err("one projected representative per family".into());
    };
    let original_guid = expected["GUID"].as_str().ok_or("original GUID")?;
    assert!(
        row["guidRaw"]["value"].as_str() == Some(original_guid),
        "exact original GUID"
    );
    assert!(
        row["guid"]["sourceUsesRefs"]
            .as_array()
            .is_some_and(|uses| !uses.is_empty()),
        "original field provenance"
    );
    match table {
        "geologySets" => verify_geology(expected, row)?,
        "soils" => verify_soil(expected, row)?,
        "soilTextureSets" => verify_height_scales(expected, row)?,
        "geologyTextureSets" => verify_layer_count(expected, row)?,
        "shadingClasses" => verify_shading(expected, row)?,
        "ecosystems" => verify_ecosystem(expected, row)?,
        "climateAssets" => verify_climate(expected, row)?,
        _ => return Err("private witness family classification".into()),
    }
    Ok(())
}

fn family_table(path: &str) -> Option<&'static str> {
    Some(match path.rsplit_once('.')?.1 {
        "geology" => "geologySets",
        "geotextureset" => "geologyTextureSets",
        "soil" => "soils",
        "soiltextureset" => "soilTextureSets",
        "shadingclass" => "shadingClasses",
        "eco" => "ecosystems",
        "climasset" => "climateAssets",
        _ => return None,
    })
}

fn verify_geology(source: &Value, row: &Value) -> TestResult {
    let expected = source["scatteredAssets"]
        .as_array()
        .ok_or("original asset selectors")?;
    let actual = row["scatteredAssetsRaw"]["value"]
        .as_array()
        .ok_or("projected asset selectors")?;
    assert_eq!(expected.len(), actual.len());
    for (original, stored) in expected.iter().zip(actual) {
        assert!(
            original == &stored["value"],
            "original array order and spelling"
        );
    }
    check_guid_link(
        &source["geologyTextureSetGUID"],
        &row["links"]["geologyTextureSet"],
        "filtered",
    )
}

fn verify_soil(source: &Value, row: &Value) -> TestResult {
    assert!(
        source["bBeach"] == row["beach"]["value"],
        "original boolean"
    );
    check_guid_link(
        &source["soilTextureSetGUID"],
        &row["soilTextureSet"],
        "filtered",
    )
}

fn verify_height_scales(source: &Value, row: &Value) -> TestResult {
    let values = source["mediumLayerHeightScales"]
        .as_array()
        .ok_or("source height array")?;
    let observed = row["mediumLayerHeightScales"]["value"]
        .as_array()
        .ok_or("exported height array")?;
    assert_eq!(values.len(), observed.len());
    for (original, cell) in values.iter().zip(observed) {
        assert!(
            original.as_f64() == cell["value"].as_str().and_then(|v| v.parse::<f64>().ok()),
            "independently decoded height value and position"
        );
    }
    verify_layer_count(source, row)
}

fn verify_layer_count(source: &Value, row: &Value) -> TestResult {
    let list = source["groundLayers"]
        .as_array()
        .ok_or("source ground-layer list")?;
    let actual = row["groundLayers"]["value"]
        .as_array()
        .ok_or("projected layer list")?;
    assert_eq!(list.len(), actual.len());
    for (layer, projected) in list.iter().zip(actual) {
        assert!(
            layer["textureName"] == projected["value"]["textureName"]["value"],
            "original texture slot"
        );
    }
    Ok(())
}

fn verify_shading(source: &Value, row: &Value) -> TestResult {
    assert!(
        source["groundLayer"]["textureName"] == row["groundLayer"]["value"]["textureName"]["value"],
        "original ground texture"
    );
    let expected = source["lightingModel"]
        .as_i64()
        .ok_or("original stored lighting enumerant")?;
    assert!(
        Some(expected)
            == row["lightingModel"]["value"]
                .as_str()
                .and_then(|x| x.parse::<i64>().ok()),
        "original numeric lighting value"
    );
    Ok(())
}

fn verify_ecosystem(source: &Value, row: &Value) -> TestResult {
    let original = source["SoundEffect"]["audioRTPCArray"]
        .as_array()
        .ok_or("original audio slots")?;
    let count = row["soundEffect"]["value"]["audioRtpcArrayCount"]["value"]
        .as_str()
        .and_then(|x| x.parse::<usize>().ok());
    assert_eq!(
        count,
        Some(original.len()),
        "exact original array cardinality"
    );
    assert!(
        source["Textures"]["ObjectPresetSpawnMask"]
            == row["textures"]["value"]["objectPresetSpawnMask"]["value"],
        "original mask path"
    );
    Ok(())
}

fn verify_climate(source: &Value, row: &Value) -> TestResult {
    let actual = &row["appearance"]["shadingLinks"]["close"];
    check_guid_link(&source["shadingClassCloseGUID"], actual, "filtered")
}

fn check_guid_link(expected: &Value, link: &Value, state: &str) -> TestResult {
    assert!(
        link["state"]["value"].as_str() == Some(state),
        "structural directory lookup"
    );
    let target = link["candidates"]
        .as_array()
        .and_then(|rows| rows.first())
        .ok_or("exact target candidate")?;
    let expected = expected
        .as_str()
        .ok_or("authored target UUID")?
        .trim_matches(['{', '}'])
        .to_ascii_lowercase();
    assert!(
        target["guid"]["value"].as_str() == Some(expected.as_str()),
        "exact source GUID equals target"
    );
    assert!(
        target["guid"]["sourceUsesRefs"]
            .as_array()
            .is_some_and(|uses| !uses.is_empty()),
        "physical target GUID receipt"
    );
    Ok(())
}
