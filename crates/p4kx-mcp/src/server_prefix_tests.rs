// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs::{self, File, FileTimes};
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, UNIX_EPOCH};

use p4kx_channels::{DiscoveryLimits, SearchPaths, SearchRecipe, WineEnvironment};
use p4kx_settings::KeyText;
use p4kx_testkit::{ArchiveBuilder, TestEntry, write_build_manifest};
use rmcp::handler::server::wrapper::Parameters;
use serde_json::{Value, json};

use super::Server;
use crate::keys::KeyChoice;
use crate::locate::Locations;
use crate::params::{ArchiveParams, ChannelsParams};

type TestResult = Result<(), Box<dyn Error>>;

const FIXTURE_EPOCH_SECONDS: u64 = 1_792_000_000;

#[tokio::test]
async fn one_server_refreshes_prefix_patches_and_channel_builds_after_source_changes() -> TestResult
{
    let root = tempfile::tempdir()?;
    let prefix = root.path().join("prefix");
    let library = prefix.join("drive_c/Games/SC");
    let channel = library.join("LIVE");
    fs::create_dir_all(&channel)?;
    write_build_manifest(
        &channel,
        "sc-alpha-4.3.1",
        "4.3.1-live.10000001",
        "10000001",
        "9001",
    )?;
    ArchiveBuilder::new()
        .entry(TestEntry::stored("readme.txt", b"synthetic".to_vec()))
        .write_to(channel.join("Data.p4k"))?;
    let logs = prefix.join("drive_c/users/Pilot/AppData/Roaming/rsilauncher/logs");
    fs::create_dir_all(&logs)?;
    let log = logs.join("log.log");
    write_log(&log, "4.3.1")?;
    set_time(&log, 0)?;
    let locations = Locations::with_recipe(
        SearchRecipe::with_environment(
            SearchPaths::default(),
            WineEnvironment::new(None, Some(prefix.into_os_string())),
            DiscoveryLimits::default(),
        ),
        None,
    );
    let server = Server::new(
        locations.clone(),
        Some(channel.join("Data.p4k")),
        KeyChoice::new(Some(KeyText::new(p4kx_testkit::test_key().to_hex())), None),
    );
    assert_info(&server, "4.3.1").await?;
    assert_channels(&server, "4.3.1").await?;
    let cached = locations.public_patches();
    assert!(
        Arc::ptr_eq(&cached, &locations.public_patches()),
        "the actual server owner reuses a complete map"
    );
    write_log(&log, "4.4.1")?;
    set_time(&log, 10)?;
    assert_info(&server, "4.4.1").await?;
    assert_channels(&server, "4.4.1").await?;
    fs::remove_file(&log)?;
    let response = server
        .archive_info(Parameters(ArchiveParams::default()))
        .await?;
    let value = serde_json::to_value(response.value)?;
    assert_eq!(
        value.pointer("/build/public_patch"),
        Some(&Value::Null),
        "one persistent server clears a removed prefix patch rather than retaining stale state"
    );
    Ok(())
}

fn write_log(path: &Path, version: &str) -> Result<(), std::io::Error> {
    fs::write(
        path,
        format!(
            "Installing Star Citizen LIVE at C:\\\\Games\\\\SC (force DP)\n\
         Delta update applied (SC LIVE {version}-live.10000001)\n",
        ),
    )
}

async fn assert_info(server: &Server, expected: &str) -> TestResult {
    let response = server
        .archive_info(Parameters(ArchiveParams::default()))
        .await?;
    let value = serde_json::to_value(response.value)?;
    assert_eq!(
        value.pointer("/build/public_patch"),
        Some(&json!(expected)),
        "archive_info attaches the patch from the current prefix snapshot"
    );
    Ok(())
}

async fn assert_channels(server: &Server, expected: &str) -> TestResult {
    let response = server
        .list_channels(Parameters(ChannelsParams::default()))
        .await?;
    let value = serde_json::to_value(response.value)?;
    let channels = value
        .get("channels")
        .and_then(Value::as_array)
        .ok_or("channel rows")?;
    assert_eq!(
        channels.len(),
        1,
        "the prefix-only library produces exactly one Channel"
    );
    let channel = channels.first().ok_or("LIVE")?;
    assert_eq!(
        channel.get("name"),
        Some(&json!("LIVE")),
        "the actual detected Channel is LIVE"
    );
    assert_eq!(
        channel.pointer("/build/public_patch"),
        Some(&json!(expected)),
        "list_channels and archive_info agree on the current patch"
    );
    Ok(())
}

fn set_time(path: &Path, seconds: u64) -> TestResult {
    let time = UNIX_EPOCH + Duration::from_secs(FIXTURE_EPOCH_SECONDS + seconds);
    File::options()
        .write(true)
        .open(path)?
        .set_times(FileTimes::new().set_modified(time))?;
    Ok(())
}

#[tokio::test]
async fn missing_channel_errors_preserve_measured_discovery_failure_reasons() -> TestResult {
    let root = tempfile::tempdir()?;
    let steam = root.path().join("steam");
    fs::create_dir_all(steam.join("steamapps"))?;
    fs::write(
        steam.join("steamapps/libraryfolders.vdf"),
        "\"libraryfolders\" {",
    )?;
    let recipe = SearchRecipe::with_environment(
        SearchPaths::default(),
        WineEnvironment::new(None, None).with_steam_roots(vec![steam]),
        DiscoveryLimits::default(),
    );
    let server = Server::new(
        Locations::with_recipe(recipe, None),
        None,
        KeyChoice::default(),
    );
    let outcome = server
        .archive_info(Parameters(ArchiveParams {
            archive: Some("LIVE".to_owned()),
        }))
        .await;
    let Err(message) = outcome else {
        return Err("a missing Channel must remain an error".into());
    };
    assert!(
        message.contains("no detected Channel"),
        "the primary resolution error is preserved"
    );
    assert!(
        message.contains("libraryfolders.vdf"),
        "the affected discovery source is reported"
    );
    assert!(
        message.contains("malformed"),
        "the measured configuration failure is not discarded"
    );
    let preload = server.preload(Some("LIVE".to_owned())).await;
    let Err(message) = preload else {
        return Err("preload must retain missing-Channel failure".into());
    };
    assert!(
        message.contains("libraryfolders.vdf"),
        "preload preserves the discovery source"
    );
    assert!(
        message.contains("malformed"),
        "preload preserves the measured failure reason"
    );
    Ok(())
}
