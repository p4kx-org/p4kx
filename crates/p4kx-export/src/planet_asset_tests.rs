// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::time::UNIX_EPOCH;

use p4kx_extraction::{Archive, Filter};
use p4kx_testkit::{ArchiveBuilder, DataForgeBuilder, TestEntry, TestRecord, TestStruct};
use serde_json::{Value, json};

use crate::{CatalogOptions, CatalogPublicationMode, build_catalog};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;
const GUID: &str = "12300000-0000-0000-0000-000000000012";
const SHADING: &str = "12300000-0000-0000-0000-000000000015";
const SETUP: &str = "00000000-0000-0000-0000-000000000007";
const RESOURCE: &str = "12300000-0000-0000-0000-000000000020";
const CLIMATE: &str = "Data/Libs/Planets/climate_assets/fixture.climasset";
const GEOLOGY: &str = "Data/Libs/Planets/geology_assets/fixture.geoasset";

fn archive() -> TestResult<(tempfile::TempDir, Archive)> {
    let temp = tempfile::tempdir()?;
    let archive = temp.path().join("Data.p4k");
    let forge = DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("HarvestableSetup"))
        .record(TestRecord::new(
            7,
            "HarvestableSetup",
            "Fixture",
            "fixtures/setup.xml",
        ))
        .build();
    let climate = format!(
        r#"{{
        "GUID":"{GUID}", "minAnnualRainfall":-0.1000000003,
        "shadingClassGUID":"{SHADING}", "shadingClassFarGUID":null,
        "subAssets":[{{"harvestableResource":"{RESOURCE}","harvestableSetupRecord":"{SETUP}","bGroundCover":false}},
                     null, {{"assetSize":2e1,"extraAttribute":{{"ordinal":2}}}}],
        "growthStageBuckets":[{{"shadeDensity":1.0}},{{"shadeDensity":2.00}}],
        "extraRoot":true
    }}"#
    );
    let geology = format!(
        r#"{{
        "GUID":"{GUID}","SunShadowMode":2, "size":[1.0,2e1,{{"unexpected":true}}],
        "Mineable_cfg_path":"Fixtures/a.cgf","Mineable_Material_path":"Fixtures/missing.mtl",
        "geomPath":"Fixtures/a.cgf","matPath":"","resources":[{{"unknown":1}},null]
    }}"#
    );
    let resource =
        format!(r#"{{"guid":"{RESOURCE}","name":"Fixture","bMinable":true,"tintPalettePath":""}}"#);
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", forge))
        .entry(TestEntry::stored(CLIMATE, climate).encrypted())
        .entry(TestEntry::stored(GEOLOGY, geology))
        .entry(TestEntry::stored(
            "Data/Libs/Planets/resources/fixture.resource",
            resource,
        ))
        .entry(TestEntry::stored("Data/Fixtures/a.cgf", "fixture asset"))
        .write_to(&archive)?;
    Ok((
        temp,
        Archive::open_with_key(archive, Some(p4kx_testkit::test_key()))?,
    ))
}

fn export(archive: &Archive, filter: Option<&Filter>) -> TestResult<Value> {
    let mut options = CatalogOptions::new(UNIX_EPOCH);
    options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    options.filter = filter;
    Ok(serde_json::from_slice(
        build_catalog(archive, options.with_full_provenance())?.json(),
    )?)
}

#[test]
fn climate_retains_ordered_fields_and_declared_original_targets() -> TestResult {
    let (_temp, archive) = archive()?;
    let output = export(&archive, None)?;
    let row = output
        .pointer("/definitions/climateAssets/rows/0")
        .ok_or("climate row")?;
    assert_eq!(
        row.pointer("/envelope/minAnnualRainfall/value"),
        Some(&json!("-0.1000000003")),
        "exact numeric spelling"
    );
    assert_eq!(
        row.pointer("/appearance/shadingClass/value"),
        Some(&json!(SHADING)),
        "legacy independent shading value"
    );
    assert_eq!(
        row.pointer("/appearance/shadingClassFar/state"),
        Some(&json!("null")),
        "explicit null role"
    );
    assert_eq!(
        row.pointer("/subAssets/value/0/value/harvestableSetup/state/value"),
        Some(&json!("resolved")),
        "actual declared setup Record"
    );
    assert_eq!(
        row.pointer("/subAssets/value/0/value/harvestableResourceLink/state/value"),
        Some(&json!("resolved")),
        "actual original loose resource"
    );
    assert_eq!(
        row.pointer("/subAssets/value/1/state"),
        Some(&json!("null")),
        "ordinal null element preserved"
    );
    assert_eq!(
        row.pointer("/subAssets/value/2/value/assetSize/value"),
        Some(&json!("2e1")),
        "later numeric occurrence"
    );
    assert_eq!(
        row.pointer("/subAssets/value/2/value/additionalFields/0/token/value"),
        Some(&json!(r#"{"ordinal":2}"#)),
        "additional authored token"
    );
    assert_eq!(
        row.pointer("/growth/buckets/value/1/value/shadeDensity/value"),
        Some(&json!("2.00")),
        "ordered exact number token"
    );
    assert_eq!(
        row.pointer("/growth/buckets/value/1/value/shadeDensity/containsEncryptedSource"),
        Some(&json!(true)),
        "direct per-value origin"
    );
    Ok(())
}

#[test]
fn climate_links_distinguish_filtered_targets() -> TestResult {
    let (_temp, archive) = archive()?;
    let filter = Filter::new("fixture.climasset");
    let filtered = export(&archive, Some(&filter))?;
    assert_eq!(filtered.pointer("/definitions/climateAssets/rows/0/subAssets/value/0/value/harvestableResourceLink/state/value"), Some(&json!("filtered")), "parent and target selection differ");
    Ok(())
}

#[test]
fn geology_retains_legacy_modes_raw_resources_and_exact_paths() -> TestResult {
    let (_temp, archive) = archive()?;
    let output = export(&archive, None)?;
    let row = output
        .pointer("/definitions/geologyAssets/rows/0")
        .ok_or("geology row")?;
    assert_eq!(
        row.pointer("/geometry/sunShadowMode/value/type"),
        Some(&json!("numeric")),
        "legacy storage is explicit"
    );
    assert_eq!(
        row.pointer("/geometry/sunShadowMode/value/value"),
        Some(&json!("2")),
        "numeric token retained"
    );
    assert_eq!(
        row.pointer("/geometry/size/value/1/value"),
        Some(&json!("2e1")),
        "ordered vector components"
    );
    assert_eq!(
        row.pointer("/geometry/size/value/2/state"),
        Some(&json!("invalid")),
        "malformed vector element retains its slot"
    );
    assert_eq!(
        row.pointer("/additionalFields/0/token/value"),
        Some(&json!(r#"[1.0,2e1,{"unexpected":true}]"#)),
        "a partially projected array retains its original token"
    );
    assert_eq!(
        row.pointer("/references/mineableConfig/state/value"),
        Some(&json!("resolved")),
        "actual exact archive path"
    );
    assert_eq!(
        row.pointer("/references/mineableMaterial/state/value"),
        Some(&json!("missing")),
        "target absence is represented"
    );
    assert_eq!(
        row.pointer("/resources/value/0/value"),
        Some(&json!(r#"{"unknown":1}"#)),
        "unrecognized populated resource raw token"
    );
    assert_eq!(
        row.pointer("/resources/value/1/state"),
        Some(&json!("null")),
        "unknown array preserves explicit null element"
    );
    assert!(
        row.get("issues")
            .and_then(Value::as_array)
            .is_some_and(|issues| issues
                .iter()
                .any(|issue| issue.get("code") == Some(&json!("unsupportedValue")))),
        "reported unknown resource model"
    );
    Ok(())
}

#[cfg(test)]
#[path = "planet_asset_witness.rs"]
mod planet_asset_witness;

#[path = "planet_asset_projection_witness.rs"]
mod planet_asset_projection_witness;
