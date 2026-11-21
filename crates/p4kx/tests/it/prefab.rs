// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! `p4kx prefab list` and `p4kx prefab dump` end to end over a synthetic nested library.

use std::error::Error;
use std::path::{Path, PathBuf};

use p4kx_testkit::{ArchiveBuilder, TestEntry, TestPrefabLibrary, TestPrefabObject};
use serde_json::Value;

#[path = "prefab_dump_origins.rs"]
mod dump_origins;
#[path = "prefab_guid_remapping.rs"]
mod guid_remapping;

type TestResult = Result<(), Box<dyn Error>>;

const IDENTITY: [f64; 4] = [1.0, 0.0, 0.0, 0.0];
const QUARTER_Z: [f64; 4] = [
    std::f64::consts::FRAC_1_SQRT_2,
    0.0,
    0.0,
    std::f64::consts::FRAC_1_SQRT_2,
];

struct Run {
    stdout: String,
    stderr: String,
    code: Option<i32>,
}

fn write_archive(dir: &Path) -> Result<PathBuf, Box<dyn Error>> {
    let rooms = TestPrefabLibrary::new("test/rooms")
        .prefab(
            "hall",
            vec![
                TestPrefabObject::brush("wall", "objects/test/wall.cgf", [1.0, 2.0, 3.0], IDENTITY),
                TestPrefabObject::nested(
                    "sub",
                    "test/rooms",
                    "closet",
                    [10.0, 0.0, 0.0],
                    QUARTER_Z,
                ),
                TestPrefabObject::nested("ghost", "test/rooms", "nothing", [0.0; 3], IDENTITY),
            ],
        )
        .prefab(
            "closet",
            vec![TestPrefabObject::brush(
                "shelf",
                "objects/test/shelf.cgf",
                [1.0, 0.0, 0.0],
                IDENTITY,
            )],
        );
    let path = dir.join("Data.p4k");
    ArchiveBuilder::new().entry(rooms.entry()).write_to(&path)?;
    Ok(path)
}

fn run(args: &[&str], archive: &Path, json: bool) -> Result<Run, Box<dyn Error>> {
    let (action, rest) = args.split_first().ok_or("no action")?;
    let mut command = p4kx_testkit::p4kx()?;
    if json {
        command.arg("--json");
    }
    let output = command
        .arg("prefab")
        .arg(action)
        .arg(archive)
        .args(rest)
        .output()?;
    Ok(Run {
        stdout: String::from_utf8(output.stdout)?,
        stderr: String::from_utf8(output.stderr)?,
        code: output.status.code(),
    })
}

#[test]
fn the_libraries_and_their_prefabs_are_listed() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path())?;

    let libraries = run(&["list"], &archive, false)?;
    assert_eq!(libraries.code, Some(0), "{}", libraries.stderr);
    assert_eq!(libraries.stdout.trim(), "test/rooms");

    let prefabs = run(&["list", "TEST/Rooms"], &archive, true)?;
    assert_eq!(prefabs.code, Some(0), "{}", prefabs.stderr);
    let json: Value = serde_json::from_str(&prefabs.stdout)?;
    assert_eq!(json["library"], "test/rooms");
    assert_eq!(json["file"], "test/rooms");
    assert_eq!(json["prefabs"][0]["name"], "hall");
    assert_eq!(json["prefabs"][0]["objects"], 3);
    assert_eq!(json["prefabs"][0]["nested"], 2);
    assert_eq!(json["prefabs"][1]["name"], "closet");

    let table = run(&["list", "test/rooms"], &archive, false)?;
    assert!(table.stdout.contains("hall"), "{}", table.stdout);
    Ok(())
}

#[test]
fn duplicate_library_names_keep_their_physical_directory_sources() -> TestResult {
    const NAME: &str = "Data/Prefabs/test/shared.xml";
    let directory = tempfile::tempdir()?;
    let archive = directory.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored(NAME, b"not an XML payload").encrypted())
        .entry(TestEntry::stored(NAME, b"another unread payload"))
        .write_to(&archive)?;

    let result = run(&["list"], &archive, true)?;
    assert_eq!(result.code, Some(0), "{}", result.stderr);
    let json: Value = serde_json::from_str(&result.stdout)?;
    assert_eq!(
        json["libraries"],
        serde_json::json!(["test/shared", "test/shared"]),
        "the directory list retains physical duplicates"
    );
    for (index, encryption) in [(0, "encrypted"), (1, "unencrypted")] {
        let key = format!("/libraries/{index}");
        let origin = json["fieldOrigins"]
            .get(&key)
            .ok_or("missing list origin")?;
        assert_eq!(
            origin["containsEncryptedSource"], false,
            "the root directory supplied the name without reading the payload"
        );
        assert_eq!(origin["sourceSetComplete"], true);
        let source = library_source(&json, origin)?;
        assert_eq!(source["entryIndex"], index);
        assert_eq!(source["storedEncryption"], encryption);
    }
    Ok(())
}

fn library_source<'report>(
    json: &'report Value,
    origin: &Value,
) -> Result<&'report Value, Box<dyn Error>> {
    let usage = source_use(json, origin)?;

    assert_eq!(
        usage["scope"], "archiveDirectoryMetadata",
        "name from the directory"
    );
    assert_eq!(
        usage["role"], "conversion",
        "display name derives from the path"
    );
    assert_eq!(
        usage.pointer("/location/field").and_then(Value::as_str),
        Some("path"),
        "exact path field"
    );
    json.pointer("/provenance/sources")
        .and_then(Value::as_array)
        .and_then(|sources| {
            sources
                .iter()
                .find(|source| source["id"] == usage["source"])
        })
        .ok_or_else(|| "unresolved physical directory source".into())
}

fn source_use<'report>(
    json: &'report Value,
    origin: &Value,
) -> Result<&'report Value, Box<dyn Error>> {
    let [reference] = origin["sourceUsesRefs"]
        .as_array()
        .ok_or("source use refs")?
        .as_slice()
    else {
        return Err("expected one original source use".into());
    };
    json.pointer("/provenance/uses")
        .and_then(Value::as_array)
        .and_then(|uses| uses.iter().find(|usage| &usage["id"] == reference))
        .ok_or_else(|| "unresolved source use".into())
}

#[test]
fn library_details_keep_directory_and_payload_origins_separate() -> TestResult {
    let directory = tempfile::tempdir()?;
    let archive = directory.path().join("Data.p4k");
    let xml = concat!(
        "\u{feff}",
        r#"<PrefabsLibrary Name="Room"><Prefab Name="Hall" Id="room-id"><Objects/></Prefab></PrefabsLibrary>"#,
    );
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Prefabs/test/room.xml", xml.as_bytes()).encrypted())
        .write_to(&archive)?;
    let result = run(&["list", "test/room"], &archive, true)?;
    assert_eq!(result.code, Some(0), "{}", result.stderr);
    let json: Value = serde_json::from_str(&result.stdout)?;
    assert_eq!(json["library"], "Room");
    assert_eq!(json["prefabs"][0]["name"], "Hall");
    let origins = json["fieldOrigins"].as_object().ok_or("field origins")?;
    let file = origins.get("/file").ok_or("directory-backed file")?;
    let path_use = source_use(&json, file)?;
    assert_eq!(file["containsEncryptedSource"], false);
    assert_eq!(path_use["scope"], "archiveDirectoryMetadata");

    for (path, literal) in [
        ("/library", "Name=\"Room\""),
        ("/prefabs/0/name", "Name=\"Hall\""),
        ("/prefabs/0/id", "Id=\"room-id\""),
        ("/prefabs/0/objects", "<Objects/>"),
        ("/prefabs/0/nested", "<Objects/>"),
        ("/prefabs/0/left_out", "<Objects/>"),
    ] {
        let origin = origins.get(path).ok_or("payload-backed field")?;
        let usage = source_use(&json, origin)?;
        assert_eq!(origin["containsEncryptedSource"], true, "{path}");
        assert_eq!(origin["sourceSetComplete"], true, "{path}");
        assert_eq!(usage["source"], path_use["source"], "{path}");
        assert_eq!(usage["scope"], "payload", "{path}");
        assert_eq!(original_bytes(xml, usage)?, literal, "{path}");
    }
    Ok(())
}

fn original_bytes<'xml>(xml: &'xml str, usage: &Value) -> Result<&'xml str, Box<dyn Error>> {
    let [start, end] = usage
        .pointer("/location/indices")
        .and_then(Value::as_array)
        .ok_or("XML range")?
        .as_slice()
    else {
        return Err("expected a text XML range".into());
    };
    let start = start.as_str().ok_or("start")?.parse::<usize>()?;
    let end = end.as_str().ok_or("end")?.parse::<usize>()?;
    xml.get(start..end)
        .ok_or_else(|| "original source bytes".into())
}

#[test]
fn a_prefab_is_dumped_with_its_nested_prefabs_resolved() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path())?;

    let dump = run(&["dump", "test/rooms", "hall"], &archive, true)?;

    assert_eq!(dump.code, Some(0), "{}", dump.stderr);
    let json: Value = serde_json::from_str(&dump.stdout)?;
    assert_eq!(json["file"], "test/rooms");
    assert_eq!(json["prefabs_entered"], 2);
    let objects = json["objects"].as_array().ok_or("no objects")?;
    assert_eq!(objects.len(), 3);
    let shelf = objects
        .iter()
        .find(|object| object["name"] == "shelf")
        .ok_or("no shelf")?;
    assert_eq!(shelf["container"], "sub");
    assert_eq!(shelf["model"], "objects/test/shelf.cgf");
    let position: Vec<f64> = shelf["position"]
        .as_array()
        .ok_or("no position")?
        .iter()
        .filter_map(Value::as_f64)
        .collect();
    assert!(
        position
            .iter()
            .zip([10.0, 1.0, 0.0])
            .all(|(a, e)| (a - e).abs() < 1e-4),
        "{position:?}"
    );
    assert_eq!(json["issues"][0]["kind"], "missing");

    let text = run(&["dump", "test/rooms", "HALL"], &archive, false)?;
    assert_eq!(text.code, Some(0), "{}", text.stderr);
    assert!(text.stdout.contains("sub/shelf"), "{}", text.stdout);
    assert!(text.stdout.contains("note ghost"), "{}", text.stdout);
    Ok(())
}

#[test]
fn an_unknown_library_or_prefab_is_invalid_input() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path())?;
    for args in [
        &["list", "test/none"][..],
        &["dump", "test/none", "hall"],
        &["dump", "test/rooms", "none"],
    ] {
        let run = run(args, &archive, false)?;
        assert_eq!(run.code, Some(2), "{args:?}: {}", run.stderr);
        assert!(run.stdout.is_empty(), "{}", run.stdout);
    }
    Ok(())
}
