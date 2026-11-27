// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{TestContainer, TestEntry};
use serde_json::{Value, json};

use crate::support::{Install, TestResult};

const SHIP: &str = "Data\\Ships\\ship.socpak";
const ROOM: &str = "ship\\rooms\\cockpit.socpak";

fn cockpit() -> TestContainer {
    TestContainer::new()
        .entry(TestEntry::deflated(
            "cockpit\\entdata\\1001.entxml",
            "<Entity Name=\"seat\"/>",
        ))
        .entry(TestEntry::stored(
            "cockpit\\cockpit.xml",
            "<ObjectContainer/>",
        ))
}

fn ship() -> TestContainer {
    TestContainer::new()
        .entry(TestEntry::stored(
            "ship\\ship.xml",
            "<ObjectContainer Name=\"ship\"/>",
        ))
        .entry(TestEntry::deflated(
            "ship/entdata/2002.entxml",
            "<Entity Name=\"door\"/>",
        ))
        .container(ROOM, &cockpit())
}

fn install() -> TestResult<Install> {
    Install::new(vec![
        TestEntry::stored("Data\\readme.txt", "hello"),
        TestEntry::zstd(SHIP, ship().build()),
        TestEntry::stored("Data\\broken.socpak", "no container"),
    ])
}

fn paths(listing: &Value) -> Vec<String> {
    listing["items"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|item| item["path"].as_str().map(str::to_owned))
        .collect()
}

#[test]
fn list_entries_lists_the_top_level_and_folders_of_a_container() -> TestResult {
    let install = install()?;
    let mut server = install.serve(&["--archive", "LIVE"])?;

    let top = server.ok(
        "list_entries",
        &json!({ "folder": "data/ships/SHIP.socpak::" }),
    )?;
    let folders = paths(&top);

    assert_eq!(
        top["container"], SHIP,
        "the container as the Archive spells it"
    );
    assert_eq!(
        folders,
        [format!("{SHIP}::ship")],
        "one folder, addressed with its container"
    );
    let inside = server.ok("list_entries", &json!({ "folder": folders[0] }))?;
    let inside = paths(&inside);
    for expected in ["ship\\rooms", "ship\\entdata", "ship\\ship.xml"] {
        assert!(
            inside.contains(&format!("{SHIP}::{expected}")),
            "{expected} is listed with its container: {inside:?}"
        );
    }
    Ok(())
}

#[test]
fn a_path_listed_in_a_container_reads_as_text_and_as_bytes() -> TestResult {
    let install = install()?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let entdata = server.ok(
        "list_entries",
        &json!({ "folder": format!("{SHIP}::ship/entdata") }),
    )?;
    let path = paths(&entdata).pop().ok_or("an Entry")?;

    let text = server.ok("read_entry_text", &json!({ "path": path }))?;
    let bytes = server.ok("read_entry_bytes", &json!({ "path": path }))?;

    assert_eq!(
        text["text"], "<Entity Name=\"door\"/>",
        "the Entry inside the container"
    );
    assert_eq!(text["path"], path, "the path as listed");
    assert_eq!(bytes["size"], 21, "the size of the Entry data");
    Ok(())
}

#[test]
fn containers_inside_containers_open_in_a_chain() -> TestResult {
    let install = install()?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let chain = format!("{SHIP}::{ROOM}");

    let listing = server.ok(
        "list_entries",
        &json!({ "folder": format!("{chain}::cockpit") }),
    )?;
    let text = server.ok(
        "read_entry_text",
        &json!({ "path": format!("{chain}::cockpit/cockpit.xml") }),
    )?;

    assert_eq!(listing["total"], 2, "both Entries of the inner container");
    assert_eq!(
        text["text"], "<ObjectContainer/>",
        "read through two containers"
    );
    Ok(())
}

#[test]
fn search_entries_searches_inside_a_container() -> TestResult {
    let install = install()?;
    let mut server = install.serve(&["--archive", "LIVE"])?;

    let found = server.ok(
        "search_entries",
        &json!({ "filter": "*.entxml", "container": SHIP }),
    )?;

    assert_eq!(found["total"], 1, "one match");
    assert_eq!(found["container"], SHIP, "the container searched");
    assert_eq!(
        found["entries"][0]["path"],
        format!("{SHIP}::ship\\entdata\\2002.entxml")
    );
    Ok(())
}

#[test]
fn listings_inside_a_container_page_with_cursors() -> TestResult {
    let install = install()?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let folder = format!("{SHIP}::ship");
    let first = server.ok("list_entries", &json!({ "folder": folder, "limit": 1 }))?;
    let cursor = first["next_cursor"].as_str().ok_or("a next page")?;

    let second = server.ok(
        "list_entries",
        &json!({ "folder": folder, "limit": 1, "cursor": cursor }),
    )?;

    assert_ne!(paths(&first), paths(&second), "the next page continues");
    assert_eq!(first["total"], second["total"], "the same listing");
    Ok(())
}

#[test]
fn bad_container_paths_are_errors_that_say_what_is_wrong() -> TestResult {
    let install = install()?;
    let mut server = install.serve(&["--archive", "LIVE"])?;

    let unknown = server.error("list_entries", &json!({ "folder": "Data/none.socpak::" }))?;
    let broken = server.error("list_entries", &json!({ "folder": "Data/broken.socpak::" }))?;
    let empty = server.error("read_entry_text", &json!({ "path": "::x" }))?;
    let missing = server.error(
        "read_entry_text",
        &json!({ "path": format!("{SHIP}::none.xml") }),
    )?;

    assert!(unknown.contains("holds no Entry"), "{unknown}");
    assert!(broken.contains("cannot open the container"), "{broken}");
    assert!(empty.contains("no container path"), "{empty}");
    assert!(missing.contains("holds no Entry"), "{missing}");
    Ok(())
}

#[test]
fn nested_list_and_search_metadata_keep_source_order_before_folder_admission() -> TestResult {
    let container = TestContainer::new()
        .entry(TestEntry::stored("Data/", b""))
        .entry(TestEntry::stored("Data/file.txt", b"file"))
        .build();
    let install = Install::new(vec![TestEntry::deflated("nested.socpak", container)])?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let listed = server.ok("list_entries", &json!({"folder": "nested.socpak::Data"}))?;
    let found = server.ok(
        "search_entries",
        &json!({"container": "nested.socpak", "filter": "file.txt"}),
    )?;
    for item in [&listed["items"][0], &found["entries"][0]] {
        assert_eq!(item["metadata"]["owner_entry_index"], 0, "admitted owner");
        assert_eq!(
            item["metadata"]["original_directory_ordinal"], "1",
            "source folder precedes file"
        );
        assert_eq!(
            item["metadata"]["zip_header"]["raw_version_made_by"], 20,
            "nested ZIP source"
        );
        assert_eq!(
            item["metadata"]["raw_v2_encryption_word"],
            Value::Null,
            "ZIP has no v2 encryption word"
        );
        assert_eq!(item["size"], 4, "existing numeric size field");
    }
    Ok(())
}
