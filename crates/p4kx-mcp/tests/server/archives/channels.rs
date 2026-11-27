// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use serde_json::{Value, json};

use crate::support::{Install, Server, TestResult};

#[test]
fn list_channels_names_each_channel_with_archive_and_build() -> TestResult {
    let install = Install::standard()?;
    super::write_launcher_log(&install)?;
    let mut server = install.serve(&[])?;
    let channels = server.ok("list_channels", &json!({}))?;
    assert_eq!(
        channels,
        json!({
            "originApplicability": "notApplicable",
            "total": 1,
            "channels": [{
                "name": "LIVE",
                "archive": super::text(&install.archive()),
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
    verify_discovery_schema(&mut server, channels)
}

fn verify_discovery_schema(server: &mut Server, mut channels: Value) -> TestResult {
    let tool = server
        .tools()?
        .into_iter()
        .find(|tool| tool["name"] == "list_channels")
        .ok_or("Channel discovery tool")?;
    let schema = tool
        .get("outputSchema")
        .ok_or("Channel discovery output schema")?;
    let validator = jsonschema::validator_for(schema)?;
    validator
        .validate(&channels)
        .map_err(|error| error.to_string())?;
    channels
        .as_object_mut()
        .ok_or("Channel result object")?
        .remove("originApplicability");
    assert!(
        !validator.is_valid(&channels),
        "the origin status is required"
    );
    channels
        .as_object_mut()
        .ok_or("Channel result object")?
        .insert("originApplicability".to_owned(), json!("unknown"));
    assert!(
        !validator.is_valid(&channels),
        "unknown origin differs from not applicable"
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
        .map_or_default(|list| list.iter().filter_map(|c| c["name"].as_str()).collect());
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
