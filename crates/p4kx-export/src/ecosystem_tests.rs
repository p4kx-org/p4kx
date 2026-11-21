// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::time::UNIX_EPOCH;

use p4kx_extraction::{Archive, Filter};
use p4kx_testkit::{ArchiveBuilder, DataForgeBuilder, TestEntry, TestStruct, test_key};
use serde_json::{Value, json};

use crate::{CatalogOptions, CatalogPublicationMode, build_catalog};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

const GUID: &str = "AAAAAAAA-0000-0000-0000-000000000020";
const ZERO: &str = "00000000-0000-0000-0000-000000000000";
const ROOT: &str = "Data/Libs/EcoSystems/PU/fixture/";

#[test]
fn ecosystem_fields_keep_original_tokens_and_independent_directory_links() -> TestResult {
    let document = r#"{
        "GUID":"{AAAAAAAA-0000-0000-0000-000000000020}",
        "name":"fixture",
        "SoundEffect":{
            "audioRTPCArray":[],
            "audioSwitchArray":[],
            "audioTrigger":"",
            "fAudioTerrainEccentricity":-0.012300e+3
        },
        "Textures":{
            "ecoSystemClimateTexture":"Fixtures/Climate.tif",
            "ecoSystemDataTexture":null,
            "ecoSystemNormalTexture":"Fixtures/Missing.dds",
            "elevationTexture":"Fixtures/../invalid.r16",
            "ObjectPresetSpawnMask":"Fixtures/Mask.r8"
        }
    }"#;
    let (_folder, archive) = fixture(vec![eco("sample", document).encrypted()])?;
    let catalog = catalog(&archive, None)?;
    let rows = rows(&catalog)?;
    let [row] = rows else {
        panic!("one physical ecosystem")
    };
    assert_eq!(row["guid"]["value"], GUID.to_ascii_lowercase());
    assert_eq!(row["guidRaw"]["value"], format!("{{{GUID}}}"));
    assert_eq!(row["name"]["containsEncryptedSource"], true);
    let sound = &row["soundEffect"]["value"];
    for field in ["audioRtpcArrayCount", "audioSwitchArrayCount"] {
        assert_eq!(sound[field]["state"], "present");
        assert_eq!(sound[field]["value"], "0");
        assert_eq!(sound[field]["containsEncryptedSource"], true);
    }
    assert_eq!(sound["audioTrigger"]["value"], "");
    assert_eq!(sound["audioTerrainEccentricity"]["value"], "-0.012300e+3");
    assert_textures(row);
    Ok(())
}

fn assert_textures(row: &Value) {
    let textures = &row["textures"]["value"];
    assert_eq!(textures["climate"]["value"], "Fixtures/Climate.tif");
    assert_eq!(textures["data"]["state"], "null");
    assert_eq!(textures["elevation"]["value"], "Fixtures/../invalid.r16");
    let references = &row["textureLinks"];
    for (name, state) in [
        ("climate", "resolved"),
        ("data", "null"),
        ("normal", "missing"),
        ("elevation", "invalid"),
        ("objectPresetSpawnMask", "resolved"),
    ] {
        assert_eq!(references[name]["state"]["value"], state);
    }
    assert_eq!(references["climate"]["targets"][0]["state"], "present");
    assert_eq!(
        references["climate"]["targets"][0]["containsEncryptedSource"],
        false
    );
    assert_ne!(
        references["climate"]["state"]["sourceUsesRefs"], textures["climate"]["sourceUsesRefs"],
        "only the resolved relationship depends on directory evidence"
    );
    assert_eq!(
        issue(row, "Textures.ecoSystemNormalTexture"),
        Some("danglingReference")
    );
    assert_eq!(
        issue(row, "Textures.elevationTexture"),
        Some("malformedSource")
    );
}

#[test]
fn ecosystem_diagnostics_preserve_readable_siblings_and_order() -> TestResult {
    let first = format!(
        r#"{{"GUID":"{GUID}","SoundEffect":{{"audioRTPCArray":[{{"unknown":4}}],"audioSwitchArray":false,"fAudioTerrainEccentricity":null}},"Textures":null}}"#
    );
    let second = format!(r#"{{"GUID":"{GUID}"}}"#);
    let third = format!(r#"{{"GUID":"{ZERO}"}}"#);
    let (_folder, archive) = fixture(vec![
        eco("first", first),
        eco("malformed", b"[".as_slice()),
        eco("second", second),
        eco("third", third),
        TestEntry::stored("Data/Libs/EcoSystemsUnexpected/decoy.eco", "{}"),
    ])?;
    let full = catalog(&archive, None)?;
    let [first, second, third] = rows(&full)? else {
        panic!("all three readable sources")
    };
    for (row, suffix) in [
        (first, "first.eco"),
        (second, "second.eco"),
        (third, "third.eco"),
    ] {
        assert!(
            row["identity"]["sourcePath"]["value"]
                .as_str()
                .is_some_and(|path| path.ends_with(suffix))
        );
    }
    for row in [first, second] {
        assert_eq!(issue(row, "GUID"), Some("ambiguousIdentity"));
    }
    assert_eq!(third["guid"]["value"], ZERO);
    assert_eq!(issue(third, "GUID"), Some("emptyValue"));
    let sound = &first["soundEffect"]["value"];
    assert_eq!(sound["audioRtpcArrayCount"]["value"], "1");
    assert_eq!(sound["audioRtpcArrayCount"]["state"], "present");
    assert_eq!(sound["audioSwitchArrayCount"]["state"], "invalid");
    assert_eq!(sound["audioTerrainEccentricity"]["state"], "null");
    assert_eq!(first["textures"]["state"], "null");
    assert_eq!(second["textures"]["state"], "absent");
    assert_eq!(
        issue(first, "SoundEffect.audioRTPCArray"),
        Some("unsupportedValue")
    );
    assert_eq!(
        issue(first, "SoundEffect.audioSwitchArray"),
        Some("malformedSource")
    );
    let filter = Filter::new("first.eco");
    let scoped = catalog(&archive, Some(&filter))?;
    assert_eq!(rows(&scoped)?.len(), 1);
    assert_eq!(
        scoped.pointer("/completeness/ecosystems/status"),
        Some(&json!("partial"))
    );
    assert_eq!(scoped, catalog(&archive, Some(&filter))?);
    Ok(())
}

fn issue<'a>(row: &'a Value, field: &str) -> Option<&'a str> {
    row["issues"]
        .as_array()?
        .iter()
        .find(|issue| issue["field"] == field)?
        .get("code")?
        .as_str()
}

fn eco(stem: &str, text: impl Into<Vec<u8>>) -> TestEntry {
    TestEntry::stored(format!("{ROOT}{stem}.eco"), text)
}

fn fixture(entries: Vec<TestEntry>) -> TestResult<(tempfile::TempDir, Archive)> {
    let dir = tempfile::tempdir()?;
    let file = dir.path().join("Data.p4k");
    let forge = DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("SyntheticDefinition"))
        .build();
    entries
        .into_iter()
        .fold(
            ArchiveBuilder::new()
                .entry(TestEntry::stored("Data/Game2.dcb", forge))
                .entry(TestEntry::stored("Data/Fixtures/Climate.tif", Vec::new()))
                .entry(TestEntry::stored("Data/Fixtures/Mask.r8", Vec::new())),
            ArchiveBuilder::entry,
        )
        .write_to(&file)?;
    Ok((dir, Archive::open_with_key(file, Some(test_key()))?))
}

fn catalog(archive: &Archive, filter: Option<&Filter>) -> TestResult<Value> {
    let mut options = CatalogOptions::new(UNIX_EPOCH);
    options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    options.filter = filter;
    let catalog = build_catalog(archive, &options)?;
    Ok(serde_json::from_slice(catalog.json())?)
}

fn rows(catalog: &Value) -> TestResult<&[Value]> {
    catalog
        .pointer("/definitions/ecosystems/rows")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| "ecosystem rows missing".into())
}
