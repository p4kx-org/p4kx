// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_testkit::TestEntry;
use serde_json::{Value, json};

use crate::support::{Install, TestResult};

fn text(path: &Path) -> String {
    path.display().to_string()
}

fn write_launcher_log(install: &Install) -> TestResult {
    std::fs::write(
        install.launcher().join("logs").join("log.log"),
        "Delta update applied (SC LIVE 4.3.1-live.10000001) in LIVE\n",
    )?;
    Ok(())
}

#[test]
fn list_channels_names_each_channel_with_archive_and_build() -> TestResult {
    let install = Install::standard()?;
    write_launcher_log(&install)?;
    let mut server = install.serve(&[])?;
    let channels = server.ok("list_channels", &json!({}))?;
    assert_eq!(
        channels,
        json!({
            "total": 1,
            "channels": [{
                "name": "LIVE",
                "archive": text(&install.archive()),
                "source": "default",
                "build": {
                    "branch": "sc-alpha-4.3.1",
                    "version": "4.3.1-live.10000001",
                    "change_number": "10000001",
                    "build_id": "9001",
                    "public_patch": "4.3.1"
                }
            }]
        }),
        "the one detected Channel"
    );
    Ok(())
}

#[test]
fn list_channels_includes_manual_archives_from_the_settings() -> TestResult {
    let install = Install::standard()?;
    let manual = install.path().join("manual").join("PTU");
    std::fs::create_dir_all(&manual)?;
    std::fs::copy(install.archive(), manual.join("Data.p4k"))?;
    std::fs::write(
        install.config().join("settings.toml"),
        format!(
            "[channels]\nfolders = [{}]\narchives = []\n",
            toml_string(&manual)
        ),
    )?;
    let mut server = install.serve(&[])?;
    let channels = server.ok("list_channels", &json!({}))?;
    let names: Vec<&str> = channels["channels"]
        .as_array()
        .map(|list| list.iter().filter_map(|c| c["name"].as_str()).collect())
        .unwrap_or_default();
    assert_eq!(names, ["LIVE", "PTU"], "default first, then manual");
    assert_eq!(channels["channels"][1]["source"], "manual", "PTU is manual");
    let first = server.ok("list_channels", &json!({ "limit": 1 }))?;
    assert_eq!(first["channels"][0]["name"], "LIVE", "the first page");
    let cursor = first["next_cursor"].as_str().unwrap_or_default();
    let second = server.ok("list_channels", &json!({ "limit": 1, "cursor": cursor }))?;
    assert_eq!(second["channels"][0]["name"], "PTU", "the second page");
    assert!(second.get("next_cursor").is_none(), "the last page");
    Ok(())
}

fn toml_string(path: &Path) -> String {
    format!("'{}'", path.display())
}

#[test]
fn archive_info_by_channel_name_summarises_the_archive() -> TestResult {
    let install = Install::standard()?;
    write_launcher_log(&install)?;
    let mut server = install.serve(&[])?;
    let mut info = server.ok("archive_info", &json!({ "archive": "live" }))?;
    let metadata = info
        .as_object_mut()
        .ok_or("Archive summary object")?
        .remove("metadata")
        .ok_or("cached source metadata")?;
    assert_cached_layout(&metadata, &install)?;
    assert_eq!(
        info,
        json!({
            "archive": text(&install.archive()),
            "channel": "LIVE",
            "build": {
                "branch": "sc-alpha-4.3.1",
                "version": "4.3.1-live.10000001",
                "change_number": "10000001",
                "build_id": "9001",
                "public_patch": "4.3.1"
            },
            "entry_count": 6,
            "encrypted_count": 1,
            "issue_count": 0,
            "issues": [],
            "compressed_size": info["compressed_size"],
            "size": 4 + 80 + 50 + 3 + 3 + 5
        }),
        "the summary of LIVE"
    );
    assert!(
        info["compressed_size"]
            .as_u64()
            .is_some_and(|size| size > 0),
        "a stored size"
    );
    Ok(())
}

fn assert_cached_layout(metadata: &Value, install: &Install) -> TestResult {
    assert_eq!(
        metadata.get("version"),
        Some(&json!("1.0")),
        "layout payload version"
    );
    assert_eq!(
        metadata.get("layout"),
        Some(&json!("zip64")),
        "observed ZIP64 reader route"
    );
    assert_eq!(
        metadata.get("declarationState"),
        Some(&json!("available")),
        "cached header availability is separate from lazy row checks"
    );
    assert_eq!(
        metadata.get("sourceLength"),
        Some(&json!(
            std::fs::metadata(install.archive())?.len().to_string()
        )),
        "source length covers the immediately owning Archive"
    );
    assert_eq!(
        metadata.get("v2"),
        Some(&Value::Null),
        "ZIP has no v2 declarations"
    );
    assert_eq!(
        metadata.pointer("/zip/count"),
        Some(&json!("6")),
        "exact directory count"
    );
    assert_eq!(
        metadata.pointer("/zip/end/disk"),
        Some(&json!(65535)),
        "original disk marker"
    );
    assert_eq!(
        metadata.pointer("/zip/end/directoryOffset"),
        Some(&json!(4_294_967_295_u32)),
        "original ZIP64 offset marker"
    );
    assert_eq!(
        metadata.pointer("/zip/zip64/entries"),
        Some(&json!("6")),
        "exact ZIP64 count"
    );
    assert_eq!(
        metadata.pointer("/zip/installation"),
        Some(&Value::Null),
        "absent installation table"
    );
    Ok(())
}

#[test]
fn archive_info_by_absolute_path_opens_that_archive() -> TestResult {
    let install = Install::new(vec![TestEntry::stored("only.txt", b"x".to_vec())])?;
    let mut server = install.serve(&[])?;
    let info = server.ok(
        "archive_info",
        &json!({ "archive": text(&install.archive()) }),
    )?;
    assert_eq!(info["entry_count"], 1, "the Archive at the path");
    Ok(())
}

#[test]
fn the_server_argument_is_the_archive_of_calls_without_one() -> TestResult {
    let install = Install::standard()?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let info = server.ok("archive_info", &json!({}))?;
    assert_eq!(info["channel"], "LIVE", "the default Archive");
    let archive = text(&install.archive());
    let mut by_path = install.serve(&["--archive", &archive])?;
    assert_eq!(
        by_path.ok("archive_info", &json!({}))?["entry_count"],
        6,
        "a path as server argument"
    );
    Ok(())
}

#[test]
fn a_call_without_any_archive_says_how_to_name_one() -> TestResult {
    let install = Install::standard()?;
    let mut server = install.serve(&[])?;
    let message = server.error("archive_info", &json!({}))?;
    assert!(
        message.contains("--archive"),
        "names the options: {message}"
    );
    Ok(())
}

#[test]
fn unusable_archive_arguments_are_tool_errors() -> TestResult {
    let install = Install::standard()?;
    let not_p4k = install.path().join("settings.txt");
    std::fs::write(&not_p4k, "x")?;
    let not_archive = install.path().join("fake.p4k");
    std::fs::write(&not_archive, "not an archive")?;
    let folder = install.path().join("folder.p4k");
    std::fs::create_dir(&folder)?;
    let mut server = install.serve(&[])?;
    let cases = [
        (text(&folder), "not a file"),
        (format!("{}:stream", text(&install.archive())), "absolute"),
        ("PTU".to_owned(), "LIVE"),
        ("Data.p4k".to_owned(), "detected"),
        ("../LIVE/Data.p4k".to_owned(), "absolute"),
        (text(&not_p4k), "absolute"),
        (text(&install.path().join("missing.p4k")), "does not exist"),
        (text(&not_archive), "cannot open"),
        ("x".repeat(5000), "longer than"),
    ];
    for (archive, expected) in cases {
        let message = server.error("archive_info", &json!({ "archive": archive }))?;
        assert!(
            message.contains(expected),
            "{archive:.40}: {message} contains {expected}"
        );
    }
    Ok(())
}

#[test]
fn an_invalid_server_argument_stops_the_server() -> TestResult {
    let install = Install::standard()?;
    let output = install.command(&["--archive", "PTU"]).output()?;
    assert!(!output.status.success(), "the server does not start");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("PTU"), "names the argument: {stderr}");
    Ok(())
}

#[test]
fn unknown_arguments_are_rejected() -> TestResult {
    let install = Install::standard()?;
    let mut server = install.serve(&[])?;
    let response = server.request(
        "tools/call",
        &json!({ "name": "archive_info", "arguments": { "archive": "LIVE", "write": true } }),
    )?;
    let rejected = response.get("error").is_some()
        || response.pointer("/result/isError") == Some(&Value::Bool(true));
    assert!(rejected, "an unknown argument fails: {response}");
    Ok(())
}

#[test]
fn archive_info_lists_the_issues_of_defective_entries() -> TestResult {
    let install = Install::new(vec![
        TestEntry::stored("ok.txt", b"x".to_vec()),
        TestEntry::stored("bad.txt", b"y".to_vec()).with_encryption_field(5),
    ])?;
    let mut server = install.serve(&[])?;
    let info = server.ok("archive_info", &json!({ "archive": "live" }))?;
    assert_eq!(info["issue_count"], 1, "the count");
    assert_eq!(info["issues"][0]["path"], "bad.txt", "the Entry");
    assert!(
        info["issues"][0]["issue"]
            .as_str()
            .is_some_and(|issue| issue.contains("encryption field is 5")),
        "the defect: {info}"
    );
    Ok(())
}
