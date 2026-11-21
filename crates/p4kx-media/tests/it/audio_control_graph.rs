// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, Filter};
use p4kx_media::{ExportOptions, MediaKind, MediaRecord};
use p4kx_testkit::{ArchiveBuilder, TestEntry};
use serde_json::json;

use crate::support;
use crate::support::audio_provider::TestResult;

const FIRST: &str = r#"
<ATLConfig>
  <AudioRTPCs><ATLRtpc atl_name="SharedRtpc"><ATLPreload atl_name="PreloadA"/></ATLRtpc></AudioRTPCs>
  <AudioPreloads>
    <ATLPreloadRequest atl_name="PreloadA">
      <RTPCDependees><ATLRTPCDependee atl_name="SharedRtpc"/><ATLRTPCDependee atl_name="AbsentRtpc"/></RTPCDependees>
      <ATLPlatforms><Platform atl_name="test" atl_config_group_name="Default"/></ATLPlatforms>
      <ATLConfigGroup atl_name="Default"/>
    </ATLPreloadRequest>
    <ATLPreloadRequest atl_name="PreloadB"><ATLConfigGroup atl_name="Default"/></ATLPreloadRequest>
  </AudioPreloads>
</ATLConfig>
"#;

const SECOND: &str = r#"
<ATLConfig><AudioRTPCs><ATLRtpc atl_name="SharedRtpc"/></AudioRTPCs></ATLConfig>
"#;

#[test]
fn atl_graph_preserves_each_authored_edge_and_physical_candidate_without_following_cycles()
-> TestResult {
    let scratch = tempfile::tempdir()?;
    let archive_path = scratch.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored(
            "Data/Libs/GameAudio/First.xml",
            FIRST.as_bytes(),
        ))
        .entry(TestEntry::stored(
            "Data/Libs/GameAudio/Second.xml",
            SECOND.as_bytes(),
        ))
        .write_to(&archive_path)?;
    let archive = Archive::open(archive_path)?;
    let output = scratch.path().join("out");
    let options = ExportOptions {
        kinds: vec![MediaKind::AudioControl],
        ..ExportOptions::default()
    };
    let index = support::published(p4kx_media::export(&archive, &output, &options)?)?;
    let rows = support::records(&output, &index, "audio-control.jsonl")?;
    check_edges(&rows)
}

fn check_edges(rows: &[MediaRecord]) -> TestResult {
    for (query, kind, name, resolution, candidates) in [
        (0, "preload-request", "PreloadA", "resolved", vec![0]),
        (1, "rtpc", "SharedRtpc", "ambiguous", vec![0, 1]),
        (2, "rtpc", "AbsentRtpc", "missing", vec![]),
        (3, "config-group", "Default", "resolved", vec![0]),
    ] {
        let edge = rows
            .iter()
            .find(|row| {
                row.subresource.namespace == "audio-control-reference"
                    && row.data.pointer("/payload/link/query_index")
                        == Some(&json!(query.to_string()))
            })
            .ok_or("original edge with scoped lookup key")?;
        assert_eq!(
            edge.data.pointer("/payload/target/kind"),
            Some(&json!(kind)),
            "authored target category for query {query}"
        );
        assert_eq!(
            edge.data.pointer("/payload/target/name"),
            Some(&json!(name)),
            "exact authored target for query {query}"
        );
        let summary = rows
            .iter()
            .find(|row| {
                row.subresource.namespace == "audio-control-reference-resolution"
                    && row.subresource.indices == [query]
            })
            .ok_or("directory-local resolution")?;
        assert_eq!(
            summary.data.pointer("/payload/resolution"),
            Some(&json!(resolution)),
            "physical target coverage for query {query}"
        );
        let observed = rows
            .iter()
            .filter(|row| {
                row.subresource.namespace == "audio-control-reference-candidate"
                    && row.subresource.indices.first() == Some(&query)
            })
            .map(|row| {
                row.data
                    .pointer("/payload/target_entry_index")
                    .and_then(serde_json::Value::as_str)
                    .ok_or("candidate physical Entry")?
                    .parse::<usize>()
                    .map_err(Into::into)
            })
            .collect::<TestResult<Vec<_>>>()?;
        assert_eq!(
            observed, candidates,
            "original physical candidates for query {query}"
        );
    }
    Ok(())
}

#[test]
fn filtered_atl_definitions_block_directory_search_without_blocking_local_preload_groups()
-> TestResult {
    let scratch = tempfile::tempdir()?;
    let archive_path = scratch.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored(
            "Data/Libs/GameAudio/First.xml",
            FIRST.as_bytes(),
        ))
        .entry(TestEntry::stored("Data/Libs/GameAudio/Second.xml", SECOND.as_bytes()).encrypted())
        .write_to(&archive_path)?;
    let archive = Archive::open(archive_path)?;
    let output = scratch.path().join("out");
    let options = ExportOptions {
        kinds: vec![MediaKind::AudioControl],
        include: Some(Filter::parse("First.xml")?),
        ..ExportOptions::default()
    };
    let index = support::published(p4kx_media::export(&archive, &output, &options)?)?;
    let rows = support::records(&output, &index, "audio-control.jsonl")?;
    for (query, blocked, expected) in [
        (0, "1", "unavailable"),
        (1, "1", "unavailable"),
        (2, "1", "unavailable"),
        (3, "0", "resolved"),
    ] {
        let summary = rows
            .iter()
            .find(|row| {
                row.subresource.namespace == "audio-control-reference-resolution"
                    && row.subresource.indices == [query]
            })
            .ok_or("physical scoped ATL query result")?;
        assert_eq!(
            summary.data.pointer("/payload/blocked_documents"),
            Some(&json!(blocked)),
            "only unread documents in the relevant scope can block query {query}"
        );
        assert_eq!(
            summary.data.pointer("/payload/resolution"),
            Some(&json!(expected)),
            "scope completeness is explicit for query {query}"
        );
    }
    Ok(())
}
