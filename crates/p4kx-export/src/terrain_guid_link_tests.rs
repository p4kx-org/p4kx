// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;
use std::error::Error;

use p4kx_extraction::{Archive, Filter, SourceValue, SourcedValue};
use p4kx_testkit::{ArchiveBuilder, TestEntry};
use serde_json::Value;

use super::{
    geology_set, soil,
    terrain_guid_link::{TerrainGuidIndex, TerrainTargetFamily},
};
use crate::catalog_read::loose_definition::DefinitionFamily;
use crate::catalog_read::original::with_entry_capture;
use crate::export_ledger::ExportLedger;
use crate::guid_text::CatalogGuid;
use crate::{CatalogOptions, Kind};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

const ASSET: &str = "aaaaaaaa-0000-4000-8000-000000000001";
const GEO_TEXTURE: &str = "aaaaaaaa-0000-4000-8000-000000000002";
const SOIL_TEXTURE: &str = "aaaaaaaa-0000-4000-8000-000000000003";
const ABSENT: &str = "aaaaaaaa-0000-4000-8000-000000000004";
const GEOLOGY: &str = "aaaaaaaa-0000-4000-8000-000000000005";
const SOIL: &str = "aaaaaaaa-0000-4000-8000-000000000006";
const ZERO: &str = "00000000-0000-0000-0000-000000000000";

const ROOT: &str = "Data/Libs/Planets/";
const TARGETS: [TerrainTargetFamily; 3] = [
    TerrainTargetFamily {
        kind: Kind::GeologyAssets,
        selector: DefinitionFamily::new(
            Kind::GeologyAssets,
            "data/libs/planets/geology_assets/",
            "geoasset",
            "GeologyAsset",
            "GUID",
        ),
        guid_field: "GUID",
    },
    TerrainTargetFamily {
        kind: Kind::GeologyTextureSets,
        selector: DefinitionFamily::new(
            Kind::GeologyTextureSets,
            "data/libs/planets/geology_sets/",
            "geotextureset",
            "GeologyTextureSet",
            "GUID",
        ),
        guid_field: "GUID",
    },
    TerrainTargetFamily {
        kind: Kind::SoilTextureSets,
        selector: DefinitionFamily::new(
            Kind::SoilTextureSets,
            "data/libs/planets/soil_sets/",
            "soiltextureset",
            "SoilTextureSet",
            "GUID",
        ),
        guid_field: "GUID",
    },
];

#[test]
fn ordered_authored_geology_and_soil_resolve_only_expected_families() -> TestResult {
    let geology_doc = format!(
        r#"{{"GUID":"{GEOLOGY}",
        "scatteredAssets":["{ASSET}","{{{ASSET}}}","{ZERO}",null,"not-guid","{ABSENT}","{GEO_TEXTURE}"],
        "cliffAssets":[],
        "sizeProbabilities":[0.000,1.00e-2,null],
        "sizeNumberOfAssets":[0,9999999999999999999999999999999,12.5],
        "cliffProbability":0.25,
        "geologyTextureSetGUID":"{GEO_TEXTURE}"}}"#
    );
    let soil_doc = format!(
        r#"{{"GUID":"{SOIL}","soilTextureSetGUID":"{SOIL_TEXTURE}","bBeach":false,
        "bStagnantWater":null,"bTrash":true,"oceanSecondaryColourWeight":1.0000}}"#
    );
    let (_folder, archive) = archive(vec![
        entry("geology_sets/fixture.geology", geology_doc),
        entry("soil_sets/fixture.soil", soil_doc),
        entry("geology_assets/a.geoasset", identity(ASSET)).encrypted(),
        entry("geology_sets/a.geotextureset", identity(GEO_TEXTURE)),
        entry("soil_sets/a.soiltextureset", identity(SOIL_TEXTURE)),
    ])?;
    let (geology, soil) = projected(&archive, None)?;
    assert_projection(&geology, &soil, "resolved");

    let filter = Filter::new("fixture");
    let (filtered_geology, filtered_soil) = projected(&archive, Some(&filter))?;
    assert_projection(&filtered_geology, &filtered_soil, "filtered");
    Ok(())
}

fn assert_projection(geology: &Value, soil: &Value, resolved: &str) {
    let geology = &geology["rows"][0];
    assert_soil(&soil["rows"][0], resolved);
    assert_eq!(geology["sizeProbabilities"]["value"][0]["value"], "0.000");
    assert_eq!(geology["sizeProbabilities"]["value"][1]["value"], "1.00e-2");
    assert_eq!(geology["sizeProbabilities"]["value"][2]["state"], "null");
    assert_eq!(
        geology["sizeNumberOfAssets"]["value"][1]["value"],
        "9999999999999999999999999999999"
    );
    assert_eq!(
        geology["sizeNumberOfAssets"]["value"][2]["state"],
        "invalid"
    );
    assert_eq!(
        geology["cliffAssets"]["value"].as_array().map(Vec::len),
        Some(0)
    );
    assert_eq!(
        geology["links"]["cliffAssets"].as_array().map(Vec::len),
        Some(0)
    );
    let scattered = geology["links"]["scatteredAssets"]
        .as_array()
        .expect("ordered relation slots");
    for (index, state) in [
        resolved,
        resolved,
        "zero",
        "null",
        "invalid",
        "missing",
        "wrongFamily",
    ]
    .into_iter()
    .enumerate()
    {
        assert_eq!(scattered[index]["ordinal"], index.to_string());
        assert_eq!(scattered[index]["link"]["state"]["value"], state);
    }
    assert_asset_evidence(geology, scattered);
    assert_eq!(
        geology["links"]["geologyTextureSet"]["state"]["value"],
        resolved
    );
}

fn assert_asset_evidence(geology: &Value, scattered: &[Value]) {
    assert_eq!(
        geology["scatteredAssets"]["value"][0]["containsEncryptedSource"],
        false
    );
    let target = &scattered[0]["link"]["candidates"][0];
    assert_eq!(target["guid"]["containsEncryptedSource"], true);
    assert_eq!(
        scattered[0]["link"]["state"]["containsEncryptedSource"],
        true
    );
    assert_eq!(
        geology["scatteredAssetsRaw"]["value"][1]["value"],
        format!("{{{ASSET}}}")
    );
    assert_ne!(
        geology["scatteredAssets"]["value"][0]["sourceUsesRefs"],
        geology["scatteredAssets"]["value"][1]["sourceUsesRefs"]
    );
}

fn assert_soil(soil: &Value, resolved: &str) {
    assert_eq!(soil["beach"]["value"], false);
    assert_eq!(soil["stagnantWater"]["state"], "null");
    assert_eq!(soil["acid"]["state"], "absent");
    assert_eq!(soil["oceanSecondaryColourWeight"]["value"], "1.0000");
    assert_eq!(soil["soilTextureSet"]["state"]["value"], resolved);
}

#[test]
fn duplicate_and_unreadable_target_members_preserve_admitted_identity_evidence() -> TestResult {
    let (_folder, archive) = archive(vec![
        entry("geology_assets/z.geoasset", identity(ASSET)),
        entry("geology_assets/a.geoasset", identity(ASSET)),
        entry("geology_assets/unreadable.geoasset", identity(ABSENT)).with_crc32c(0),
    ])?;
    let actual = with_entry_capture(&archive, |capture| {
        let target = guid(ASSET);
        let absent = guid(ABSENT);
        let wanted = HashSet::from([target, absent]);
        let index =
            TerrainGuidIndex::read(&archive, capture, None, None, TARGETS.to_vec(), &wanted)?;
        let selector: SourcedValue<CatalogGuid> =
            capture
                .source_uses()
                .value(SourceValue::Present(target), &[], true);
        let ambiguous = index.resolve(Kind::GeologyAssets, &selector, capture)?;
        let selector = capture
            .source_uses()
            .value(SourceValue::Present(absent), &[], true);
        let unknown = index.resolve(Kind::GeologyAssets, &selector, capture)?;
        Ok((ambiguous, unknown))
    })?;
    let ambiguous = serde_json::to_value(actual.0)?;
    let unavailable = serde_json::to_value(actual.1)?;
    assert_eq!(ambiguous["state"]["value"], "ambiguous");
    assert!(
        ambiguous["candidates"][0]["entry"]["value"]["entryIndex"].as_u64()
            < ambiguous["candidates"][1]["entry"]["value"]["entryIndex"].as_u64()
    );
    assert_eq!(unavailable["state"]["value"], "unavailable");
    assert_eq!(unavailable["state"]["sourceSetComplete"], false);
    Ok(())
}

fn projected(archive: &Archive, filter: Option<&Filter>) -> TestResult<(Value, Value)> {
    let (geology, soil) = with_entry_capture(archive, |capture| {
        let mut ledger = ExportLedger::default();
        let mut options = CatalogOptions::new(std::time::UNIX_EPOCH);
        options.filter = filter;
        let mut geology = geology_set::read(archive, capture, &mut ledger, &options)?;
        let mut soil = soil::read(archive, capture, &mut ledger, &options)?;
        let mut wanted = HashSet::new();
        for row in &geology.rows {
            wanted.extend(row.required_guids());
        }
        for row in &soil.rows {
            wanted.extend(row.required_guids());
        }
        let index =
            TerrainGuidIndex::read(archive, capture, None, filter, TARGETS.to_vec(), &wanted)?;
        for row in &mut geology.rows {
            row.link(&index, capture, &mut ledger, None)?;
        }
        for row in &mut soil.rows {
            row.link(&index, capture, &mut ledger)?;
        }
        Ok((geology, soil))
    })?;
    Ok((serde_json::to_value(geology)?, serde_json::to_value(soil)?))
}

fn archive(entries: Vec<TestEntry>) -> TestResult<(tempfile::TempDir, Archive)> {
    let dir = tempfile::tempdir()?;
    let file = dir.path().join("Data.p4k");
    entries
        .into_iter()
        .fold(ArchiveBuilder::v2(), ArchiveBuilder::entry)
        .write_to(&file)?;
    Ok((
        dir,
        Archive::open_with_key(file, Some(p4kx_testkit::test_key()))?,
    ))
}

fn entry(path: &str, document: String) -> TestEntry {
    TestEntry::stored(format!("{ROOT}{path}"), document)
}

fn identity(guid: &str) -> String {
    format!(r#"{{"GUID":"{guid}"}}"#)
}

fn guid(text: &str) -> CatalogGuid {
    CatalogGuid(text.parse().expect("synthetic GUID"))
}
