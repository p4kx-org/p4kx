// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::io::Write as _;

use p4kx_extraction::ConflictPolicy;
use p4kx_media::MediaError;
use serde_json::Value;

use super::build_origin_fixture::{self as fixture, Channel, FIELDS, Input, MANIFEST, TestResult};
use super::build_origin_support as check;

#[test]
fn adjacent_build_values_have_their_actual_independent_file_fields_and_default_patch() -> TestResult
{
    let bytes = fixture::manifest("synthetic")?;
    let channel = Channel::new(Input::File(&bytes), false)?;
    let run = channel.export("media", &fixture::options(), &fixture::registry()?)?;
    for (field, logical) in FIELDS {
        assert_eq!(
            run.root()?
                .1
                .pointer(&format!("/data/build/{field}"))
                .and_then(Value::as_str),
            Some("synthetic")
        );
        check::file_field(
            &run,
            "records",
            &format!("/data/build/{field}"),
            Some(logical),
        )?;
    }
    assert!(
        run.root()?
            .1
            .pointer("/data/build/public_patch")
            .is_some_and(Value::is_null)
    );
    check::independent(&run, "records", "/data/build/public_patch")?;
    let files = run
        .sources
        .iter()
        .filter(|source| {
            source.get("kind").and_then(Value::as_str) == Some("file")
                && source.pointer("/name/value").and_then(Value::as_str) == Some(MANIFEST)
        })
        .count();
    assert_eq!(
        files, 1,
        "all four fields share one actual standalone read identity"
    );
    Ok(())
}

#[test]
fn an_equal_encrypted_p4k_entry_is_not_the_adjacent_build_manifest_input() -> TestResult {
    let bytes = fixture::manifest("twin")?;
    let channel = Channel::new(Input::File(&bytes), true)?;
    assert!(
        channel
            .archive
            .entries()
            .first()
            .ok_or("actual encrypted twin")?
            .is_encrypted()
    );
    let run = channel.export("media", &fixture::options(), &fixture::registry()?)?;
    for (field, logical) in FIELDS {
        check::file_field(
            &run,
            "records",
            &format!("/data/build/{field}"),
            Some(logical),
        )?;
    }
    Ok(())
}

#[test]
fn authored_empty_build_strings_remain_present_file_values() -> TestResult {
    let bytes = fixture::manifest("")?;
    let channel = Channel::new(Input::File(&bytes), false)?;
    assert_eq!(
        channel
            .archive
            .build()
            .ok_or("present empty Build")?
            .branch(),
        ""
    );
    let run = channel.export("media", &fixture::options(), &fixture::registry()?)?;
    for (field, logical) in FIELDS {
        assert_eq!(
            run.root()?.1.pointer(&format!("/data/build/{field}")),
            Some(&Value::String(String::new()))
        );
        check::file_field(
            &run,
            "records",
            &format!("/data/build/{field}"),
            Some(logical),
        )?;
    }
    Ok(())
}

#[test]
fn absent_and_unreadable_manifests_do_not_create_completed_file_payload_contributors() -> TestResult
{
    for input in [Input::Missing, Input::Unreadable] {
        let channel = Channel::new(input, false)?;
        assert!(channel.archive.build().is_none());
        let run = channel.export("media", &fixture::options(), &fixture::registry()?)?;
        assert!(
            run.root()?
                .1
                .pointer("/data/build")
                .is_some_and(Value::is_null)
        );
        check::unknown(&run, "records", "/data/build")?;
    }
    Ok(())
}

#[test]
fn completed_malformed_manifest_reads_keep_the_actual_selected_null_input() -> TestResult {
    for bytes in [b"".as_slice(), b"{bad", b"{\"Data\":{}}", b"{\"Data\":{\"Branch\":null,\"Version\":\"v\",\"RequestedP4ChangeNum\":\"c\",\"BuildId\":\"b\"}}", b"{\"Data\":{\"Branch\":\"a\",\"Branch\":\"b\",\"Version\":\"v\",\"RequestedP4ChangeNum\":\"c\",\"BuildId\":\"b\"}}"] {
        let channel = Channel::new(Input::File(bytes), false)?;
        assert!(channel.archive.build().is_none());
        let run = channel.export("media", &fixture::options(), &fixture::registry()?)?;
        assert!(run.root()?.1.pointer("/data/build").is_some_and(Value::is_null));
        check::file_field(&run, "records", "/data/build", None)?;
        let uses = run.contributions(run.field("records", "/data/build")?)?;
        assert!(uses.iter().any(|used| matches!(used.get("role").and_then(Value::as_str), Some("selector" | "conversion"))));
        assert!(!uses.iter().any(|used| used.get("role").and_then(Value::as_str) == Some("payload")), "a generated parse-null is not a stored null Payload");
    }
    Ok(())
}

#[test]
fn cached_build_origin_keeps_the_original_read_length_after_an_equivalent_manifest_rewrite()
-> TestResult {
    let bytes = fixture::manifest("cached")?;
    let channel = Channel::new(Input::File(&bytes), false)?;
    fs::OpenOptions::new()
        .append(true)
        .open(channel.folder.path().join(MANIFEST))?
        .write_all(&[b' '; 137])?;
    assert!(
        fs::metadata(channel.folder.path().join(MANIFEST))?.len() > u64::try_from(bytes.len())?
    );
    let run = channel.export("media", &fixture::options(), &fixture::registry()?)?;
    check::file_field(&run, "records", "/data/build/branch", Some("Data.Branch"))?;
    let uses = run.contributions(run.field("records", "/data/build/branch")?)?;
    let expected = bytes.len().to_string();
    for used in uses {
        assert_eq!(
            run.physical(used)?
                .pointer("/byteLength/value")
                .and_then(Value::as_str),
            Some(expected.as_str()),
            "origin is bound to the cached original read, not the rewritten pathname"
        );
    }
    Ok(())
}

#[test]
fn actual_manifest_build_changes_keep_watch_refusal_and_the_previous_complete_generation()
-> TestResult {
    for remove in [false, true] {
        let bytes = fixture::manifest("first")?;
        let channel = Channel::new(Input::File(&bytes), false)?;
        let registry = fixture::registry()?;
        let run = channel.export("media", &fixture::options(), &registry)?;
        let committed = fs::read(run.output.join("media-index.json"))?;
        if remove {
            fs::remove_file(channel.folder.path().join(MANIFEST))?;
        } else {
            fs::write(
                channel.folder.path().join(MANIFEST),
                fixture::manifest("changed")?,
            )?;
        }
        let mut options = fixture::options();
        options.conflict = ConflictPolicy::Overwrite;
        let failure = p4kx_media::export_with(&channel.archive, &run.output, &options, &registry)
            .err()
            .ok_or("changed Build published")?;
        assert!(
            super::converted_artifact_support::contains(&failure, |cause| matches!(
                cause,
                MediaError::Changed(_)
            )),
            "the original Build Watch still refuses changed parsed values"
        );
        assert_eq!(fs::read(run.output.join("media-index.json"))?, committed);
    }
    Ok(())
}
