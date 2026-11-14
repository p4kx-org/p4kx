// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;

use p4kx_testkit::{ArchiveBuilder, DataForgeBuilder, TestEntry};
use serde_json::{Value, json};

use super::{GUN, MISSILE, TestResult, WEAPON, archive, database, json as run_json, text};

#[test]
fn selected_tag_fields_retain_distinct_original_uses() -> TestResult {
    let (_directory, archive) = archive()?;
    let document = run_json("tags", &archive, &[&text(GUN), "--json"])?;
    let tags = document["tags"].as_array().ok_or("Tag rows")?;
    let [row] = tags.as_slice() else {
        return Err("one selected Tag row".into());
    };
    assert_eq!(row["guid"], text(GUN));
    assert_eq!(row["name"], "Gun");
    assert_eq!(row["path"], "Weapon.Gun");
    assert_eq!(row["legacy_id"], Value::Null);
    assert_eq!(row["depth"], 1);

    let guid = contributors(&document, row, "/guid")?;
    let [guid] = guid.as_slice() else {
        return Err("one intrinsic GUID contribution".into());
    };
    assert_eq!(guid["role"], "payload");
    assert_eq!(
        guid["location"],
        json!({
            "type": "subresource",
            "namespace": "dataforgeRecordHeader",
            "indices": ["2"],
            "field": "guid"
        }),
        "the emitted GUID comes from the selected physical Record header"
    );

    let name = contributors(&document, row, "/name")?;
    assert_eq!(name.len(), 1);
    assert_eq!(
        body_uses(&name)?,
        [(2, "tagName", "payload")],
        "the name uses its own body occurrence"
    );
    let legacy_id = contributors(&document, row, "/legacy_id")?;
    assert_eq!(legacy_id.len(), 1);
    assert_eq!(
        body_uses(&legacy_id)?,
        [(2, "legacyGUID", "conversion")],
        "the stored sentinel contributes to the emitted null"
    );
    assert_eq!(
        body_uses(&contributors(&document, row, "/path")?)?,
        [
            (0, "tags[0]", "selector"),
            (1, "children[0]", "selector"),
            (1, "tagName", "conversion"),
            (2, "tagName", "conversion"),
        ],
        "the selected path includes the hidden ancestor and both original edges"
    );
    assert_eq!(
        body_uses(&contributors(&document, row, "/depth")?)?,
        [(0, "tags[0]", "selector"), (1, "children[0]", "selector")],
        "depth uses the topology without consuming the path names"
    );
    Ok(())
}

#[test]
fn identical_dcb_bytes_keep_their_three_physical_origins() -> TestResult {
    let directory = tempfile::tempdir()?;
    let encrypted = directory.path().join("encrypted.p4k");
    let plain = directory.path().join("plain.p4k");
    let standalone = directory.path().join("copied.dcb");
    let bytes = database().build();
    fs::write(&standalone, &bytes)?;
    ArchiveBuilder::new()
        .entry(TestEntry::zstd("Data\\Game2.dcb", bytes.clone()))
        .write_to(&plain)?;
    ArchiveBuilder::new()
        .entry(TestEntry::zstd("Data\\Game2.dcb", bytes).encrypted())
        .write_to(&encrypted)?;

    for (path, mode) in [
        (encrypted, InputMode::Encrypted),
        (plain, InputMode::Plain),
        (standalone, InputMode::Standalone),
    ] {
        let document = run_json("tags", &path, &[&text(GUN), "--json"])?;
        assert_eq!(document["tag_count"], 4, "{mode:?}");
        assert_eq!(document["shown_count"], 1, "{mode:?}");
        let row = &document["tags"][0];
        assert_eq!(row["path"], "Weapon.Gun", "{mode:?}");
        let name = contributors_for(&document, row, "/name", mode)?;
        assert_eq!(body_uses(&name)?, [(2, "tagName", "payload")], "{mode:?}");
        let path = contributors_for(&document, row, "/path", mode)?;
        assert!(
            body_uses(&path)?.contains(&(1, "children[0]", "selector")),
            "{mode:?} retains the authored selector"
        );
        for count in ["/tag_count", "/shown_count"] {
            let uses = contributors_for(&document, &document, count, mode)?;
            assert!(!uses.is_empty(), "{mode:?} {count} has physical evidence");
        }
    }
    Ok(())
}

#[test]
fn flat_and_nested_child_origins_keep_each_original_relationship() -> TestResult {
    let (_directory, archive) = archive()?;
    let flat = run_json("tags", &archive, &["--json"])?;
    let nested = run_json("tags", &archive, &["--tree", "--format", "json"])?;
    let flat_parent = &flat["tags"][0];
    let nested_parent = &nested["tags"][0];
    assert_eq!(flat_parent["guid"], text(WEAPON));
    assert_eq!(nested_parent["guid"], text(WEAPON));

    for (output, guid, record, source_field) in
        [(0, GUN, 2, "children[0]"), (1, MISSILE, 3, "children[1]")]
    {
        assert_eq!(flat_parent["children"][output], text(guid));
        let nested_child = &nested_parent["children"][output];
        assert_eq!(nested_child["guid"], text(guid));
        let address = format!("/children/{output}");
        let flat_link = contributors(&flat, flat_parent, &address)?;
        let nested_link = contributors(&nested, nested_parent, &address)?;
        assert_eq!(body_uses(&flat_link)?, [(1, source_field, "selector")]);
        assert_eq!(body_uses(&nested_link)?, [(1, source_field, "selector")]);
        assert!(has_guid(&flat_link, record, "payload"));
        assert!(has_guid(&nested_link, record, "selector"));

        let intrinsic = contributors(&nested, nested_child, "/guid")?;
        assert_eq!(intrinsic.len(), 1, "the nested Tag owns its intrinsic GUID");
        assert!(has_guid(&intrinsic, record, "payload"));
        assert!(nested_child["fieldOrigins"].get("/parent").is_none());
        assert!(nested_child["fieldOrigins"].get("/depth").is_none());
    }

    let flat_gun = &flat["tags"][1];
    assert_eq!(flat_gun["parent"], text(WEAPON));
    let parent_link = contributors(&flat, flat_gun, "/parent")?;
    assert_eq!(body_uses(&parent_link)?, [(1, "children[0]", "selector")]);
    assert!(has_guid(&parent_link, 1, "payload"));

    let nested_gun = &nested_parent["children"][0];
    assert_eq!(nested_gun["children"], json!([]));
    let empty_children = contributors(&nested, nested_gun, "/children")?;
    assert_eq!(body_uses(&empty_children)?, [(2, "children", "selector")]);
    assert!(nested_gun["fieldOrigins"].get("/children/0").is_none());
    Ok(())
}

#[test]
fn tag_count_origins_distinguish_queries_and_observed_zeroes() -> TestResult {
    let (_directory, archive) = archive()?;
    let all = run_json("tags", &archive, &["--json"])?;
    let guid = run_json("tags", &archive, &[&text(GUN), "--json"])?;
    let path = run_json("tags", &archive, &["weapon.gun", "--json"])?;

    for (document, shown) in [(&all, 4), (&guid, 1), (&path, 1)] {
        assert_eq!(document["tag_count"], 4);
        assert_eq!(document["shown_count"], shown);
        let total = contributors(document, document, "/tag_count")?;
        assert!(has_guid(&total, 4, "selector"), "the total includes Armour");
        assert!(
            body_uses(&total)?.is_empty(),
            "the total consumes no Tag names"
        );
    }
    let guid_shown = contributors(&guid, &guid, "/shown_count")?;
    assert!(has_guid(&guid_shown, 2, "selector"));
    assert!(!has_guid(&guid_shown, 4, "selector"));
    assert_eq!(body_uses(&guid_shown)?, [(2, "children", "selector")]);
    let path_shown = contributors(&path, &path, "/shown_count")?;
    assert!(
        body_uses(&path_shown)?.contains(&(4, "tagName", "conversion")),
        "the path query also tests nonmatching paths"
    );

    count_issues(&all)
}

fn count_issues(all: &Value) -> TestResult {
    for issue in [
        "unclassified",
        "unreadable",
        "dangling",
        "repeated",
        "duplicate",
        "unlisted",
    ] {
        let path = format!("/issues/{issue}");
        assert_eq!(all.pointer(&path), Some(&json!(0)), "{issue}");
        let uses = contributors(all, all, &path)?;
        assert!(!uses.is_empty(), "{issue} zero is an observed result");
    }
    let classified = contributors(all, all, "/issues/unclassified")?;
    assert!(
        classified.iter().any(|used| {
            used["role"] == "selector"
                && used["location"]
                    == json!({
                        "type": "subresource",
                        "namespace": "dataforgeRecordHeaderRange",
                        "indices": ["0", "6"],
                        "field": "sourceType"
                    })
        }),
        "the classification interval includes non-Tag Records"
    );
    let origins = all.get("fieldOrigins").ok_or("tag count field origins")?;
    for issue in ["too_deep", "too_long"] {
        let path = format!("/issues/{issue}");
        assert_eq!(all.pointer(&path), Some(&json!(0)), "{issue}");
        assert_eq!(
            origins.get(&path),
            Some(&json!({
                "containsEncryptedSource": false,
                "sourceSetComplete": true,
                "sourceUsesRefs": []
            })),
            "{issue} is a compatibility constant"
        );
    }
    Ok(())
}

#[test]
fn empty_tag_listing_keeps_its_physical_zero_count_origins() -> TestResult {
    let directory = tempfile::tempdir()?;
    let archive = directory.path().join("empty.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::zstd("Data\\Game2.dcb", DataForgeBuilder::new().build()).encrypted())
        .write_to(&archive)?;
    let document = run_json("tags", &archive, &["--json"])?;
    assert_eq!(document["tags"], json!([]));
    assert_eq!(document["tag_count"], 0);
    assert_eq!(document["shown_count"], 0);
    for field in ["/tag_count", "/shown_count", "/issues/unclassified"] {
        let uses = contributors(&document, &document, field)?;
        assert_eq!(
            uses.len(),
            1,
            "{field} is bounded by the original empty Record table"
        );
        assert_eq!(
            uses[0]["location"],
            json!({
                "type": "subresource",
                "namespace": "dataforgeHeader",
                "indices": [],
                "field": "recordCount"
            })
        );
    }
    Ok(())
}

fn has_guid(uses: &[&Value], index: u32, role: &str) -> bool {
    let index = index.to_string();
    uses.iter().any(|used| {
        let location = &used["location"];
        used["role"] == role
            && location["type"] == "subresource"
            && location["namespace"] == "dataforgeRecordHeader"
            && location.pointer("/indices/0").and_then(Value::as_str) == Some(index.as_str())
            && location["field"] == "guid"
    })
}

#[derive(Debug, Clone, Copy)]
enum InputMode {
    Encrypted,
    Plain,
    Standalone,
}

impl InputMode {
    const fn encryption(self) -> (Option<bool>, &'static str, &'static str) {
        match self {
            Self::Encrypted => (Some(true), "encrypted", "archiveEntry"),
            Self::Plain => (Some(false), "unencrypted", "archiveEntry"),
            Self::Standalone => (None, "unknown", "file"),
        }
    }
}

fn contributors<'a>(
    document: &'a Value,
    row: &Value,
    field: &str,
) -> Result<Vec<&'a Value>, Box<dyn std::error::Error>> {
    contributors_for(document, row, field, InputMode::Encrypted)
}

fn contributors_for<'a>(
    document: &'a Value,
    row: &Value,
    field: &str,
    mode: InputMode,
) -> Result<Vec<&'a Value>, Box<dyn std::error::Error>> {
    let (marker, encryption, kind) = mode.encryption();
    let origin = row["fieldOrigins"]
        .get(field)
        .ok_or_else(|| format!("immediate Tag origin {field}"))?;
    assert_eq!(
        origin["containsEncryptedSource"],
        json!(marker),
        "{mode:?} {field}"
    );
    assert_eq!(origin["sourceSetComplete"], true, "{field}");
    let references = origin["sourceUsesRefs"]
        .as_array()
        .ok_or("source use refs")?;
    let observations = document
        .pointer("/provenance/uses")
        .and_then(Value::as_array)
        .ok_or("source uses")?;
    let sources = document
        .pointer("/provenance/sources")
        .and_then(Value::as_array)
        .ok_or("physical sources")?;
    let mut retained = Vec::new();
    for reference in references {
        let used = observations
            .iter()
            .find(|used| &used["id"] == reference)
            .ok_or("resolving source use")?;
        let source = sources
            .iter()
            .find(|source| source["id"] == used["source"])
            .ok_or("resolving physical source")?;
        assert_eq!(used["scope"], "payload", "{field}");
        assert_eq!(used["effectiveEncryption"], encryption, "{mode:?} {field}");
        assert_eq!(source["storedEncryption"], encryption, "{mode:?} {field}");
        assert_eq!(source["kind"], kind, "{mode:?} {field}");
        retained.push(used);
    }
    Ok(retained)
}

type BodyUse<'a> = (u64, &'a str, &'a str);

fn body_uses<'a>(uses: &[&'a Value]) -> Result<Vec<BodyUse<'a>>, Box<dyn std::error::Error>> {
    let mut body = Vec::new();
    for &used in uses {
        let location = &used["location"];
        if location["type"] == "dataForge" {
            let index = location["recordIndex"]
                .as_u64()
                .ok_or("physical Record index")?;
            let field = location["field"]
                .as_str()
                .ok_or("original body coordinate")?;
            let role = used["role"].as_str().ok_or("contribution role")?;
            body.push((index, field, role));
        }
    }
    body.sort_unstable();
    Ok(body)
}
