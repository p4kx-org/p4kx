// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;
use std::path::Path;

use p4kx_extraction::Wem;
use p4kx_testkit::{ArchiveBuilder, TestCodec, TestEntry, TestWem};
use serde_json::{Value, json};

use super::{SOUND, TestResult};

const SELECTED_FORMAT_BODY: u64 = 44;

#[test]
fn audio_info_keeps_original_format_and_directory_origins_separate() -> TestResult {
    let directory = tempfile::tempdir()?;
    let encrypted = directory.path().join("encrypted.p4k");
    let plain = directory.path().join("plain.p4k");
    let standalone = directory.path().join("standalone.wem");
    let bytes = duplicate_format_wem();
    check_original_format(&bytes)?;
    fs::write(&standalone, &bytes)?;
    ArchiveBuilder::new()
        .entry(TestEntry::stored(SOUND, bytes.clone()))
        .write_to(&plain)?;
    ArchiveBuilder::new()
        .entry(TestEntry::stored(SOUND, bytes).encrypted())
        .write_to(&encrypted)?;

    for (path, input) in [
        (encrypted.as_path(), Input::Encrypted),
        (plain.as_path(), Input::Plain),
        (standalone.as_path(), Input::Standalone),
    ] {
        let document = audio_info(path, input.entry())?;
        assert_eq!(document.get("channels"), Some(&json!(2)), "{input:?}");
        assert_eq!(document.get("format_tag"), Some(&json!(1)), "{input:?}");
        assert_eq!(
            document.get("sample_rate"),
            Some(&json!(48_000)),
            "{input:?}"
        );
        assert_eq!(document.get("sample_count"), Some(&json!(2)), "{input:?}");

        let source = check_source(&document, input)?;
        check_name_origin(&document, source, path, input)?;
        check_format_origins(&document, source, input)?;
    }
    Ok(())
}

fn duplicate_format_wem() -> Vec<u8> {
    let stereo_format = [1, 0, 2, 0, 0x80, 0xBB, 0, 0, 0, 0xEE, 2, 0, 4, 0, 16, 0];
    TestWem::pcm(TestCodec::Pcm, 1, 22_050, &[0, 1, 2, 3])
        .chunk(*b"fmt ", stereo_format)
        .tail_chunk(*b"fmt ", [0; 16])
        .bytes()
}

fn check_original_format(bytes: &[u8]) -> TestResult {
    let wem = Wem::parse(bytes)?;
    let first = wem.chunk(*b"fmt ").ok_or("first format chunk")?;
    assert_eq!(first.get(2..4), Some([1, 0].as_slice()), "first channels");
    assert_eq!(
        bytes.get(46..48),
        Some([2, 0].as_slice()),
        "second channels"
    );
    assert_eq!(wem.channels(), 2, "the decoder keeps the second format");
    assert_eq!(
        wem.sample_rate(),
        48_000,
        "the selected format supplies the rate"
    );
    assert_eq!(
        wem.sample_count(),
        Some(2),
        "stereo data has two complete frames"
    );
    Ok(())
}

fn audio_info(path: &Path, entry: Option<&str>) -> Result<Value, Box<dyn Error>> {
    let output = p4kx_testkit::p4kx()?
        .args(["--json", "audio", "info"])
        .arg(path)
        .args(entry)
        .output()?;
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    Ok(serde_json::from_slice(&output.stdout)?)
}

#[derive(Debug, Clone, Copy)]
enum Input {
    Encrypted,
    Plain,
    Standalone,
}

impl Input {
    const fn entry(self) -> Option<&'static str> {
        match self {
            Self::Encrypted | Self::Plain => Some(SOUND),
            Self::Standalone => None,
        }
    }

    const fn payload_marker(self) -> Option<bool> {
        match self {
            Self::Encrypted => Some(true),
            Self::Plain => Some(false),
            Self::Standalone => None,
        }
    }

    const fn metadata_marker(self) -> Option<bool> {
        match self {
            Self::Encrypted | Self::Plain => Some(false),
            Self::Standalone => None,
        }
    }

    const fn stored_encryption(self) -> &'static str {
        match self {
            Self::Encrypted => "encrypted",
            Self::Plain => "unencrypted",
            Self::Standalone => "unknown",
        }
    }
}

fn check_source(document: &Value, input: Input) -> Result<&Value, Box<dyn Error>> {
    let sources = document
        .pointer("/provenance/sources")
        .and_then(Value::as_array)
        .ok_or("Audio Info physical sources")?;
    let [source] = sources.as_slice() else {
        return Err("Audio Info retains the one physical input".into());
    };
    assert_eq!(
        source.get("storedEncryption").and_then(Value::as_str),
        Some(input.stored_encryption()),
        "{input:?}"
    );
    let expected_kind = match input {
        Input::Encrypted | Input::Plain => {
            assert_eq!(
                source.get("entryIndex"),
                Some(&json!(0)),
                "physical Entry index"
            );
            assert_eq!(
                source.pointer("/path/value"),
                Some(&json!(SOUND)),
                "original Entry path"
            );
            "archiveEntry"
        }
        Input::Standalone => {
            assert_eq!(
                source.pointer("/name/value"),
                Some(&json!("standalone.wem")),
                "the external source exposes its basename"
            );
            "file"
        }
    };
    assert_eq!(
        source.get("kind").and_then(Value::as_str),
        Some(expected_kind),
        "the actual input supplies its own physical source kind"
    );
    assert_eq!(
        source.get("ancestors"),
        Some(&json!([])),
        "these sources are top-level"
    );
    Ok(source.get("id").ok_or("physical source identifier")?)
}

fn check_name_origin(document: &Value, source: &Value, path: &Path, input: Input) -> TestResult {
    let displayed_path = path.display().to_string();
    let name = input.entry().unwrap_or(displayed_path.as_str());
    assert_eq!(
        document.get("sound").and_then(Value::as_str),
        Some(name),
        "established sound display value"
    );
    let uses = contributors(document, "/sound", input.metadata_marker())?;
    let [name_use] = uses.as_slice() else {
        return Err("sound name retains its single original metadata contribution".into());
    };
    let scope = if input.entry().is_some() {
        "archiveDirectoryMetadata"
    } else {
        "fileMetadata"
    };
    assert_eq!(
        name_use.get("scope").and_then(Value::as_str),
        Some(scope),
        "name uses input metadata"
    );
    assert_eq!(
        name_use.get("source"),
        Some(source),
        "name and format share the physical owner"
    );
    assert_eq!(
        name_use.get("role"),
        Some(&json!("payload")),
        "path metadata supplies the emitted name"
    );
    assert_eq!(
        name_use.get("location"),
        Some(&json!({"type":"metadata", "field":"path"})),
        "the name points to its original metadata path"
    );
    Ok(())
}

fn check_format_origins(document: &Value, source: &Value, input: Input) -> TestResult {
    for (output, original) in [("/channels", "channels"), ("/format_tag", "formatTag")] {
        let uses = contributors(document, output, input.payload_marker())?;
        let mut payloads = Vec::new();
        for used in uses {
            assert_eq!(used.get("source"), Some(source), "{input:?} {output}");
            assert_eq!(used.get("scope"), Some(&json!("payload")), "{output}");
            if used.get("role") == Some(&json!("payload")) {
                payloads.push(used.get("location").ok_or("format source coordinate")?);
            }
        }
        assert_eq!(
            payloads,
            [&json!({
                "type": "subresource",
                "namespace": "wemChunk",
                "indices": [SELECTED_FORMAT_BODY.to_string()],
                "field": original
            })],
            "{input:?} {output} retains the value from the actual selected format body"
        );
    }
    Ok(())
}

fn contributors<'a>(
    document: &'a Value,
    field: &str,
    marker: Option<bool>,
) -> Result<Vec<&'a Value>, Box<dyn Error>> {
    let origin = document
        .get("fieldOrigins")
        .and_then(|origins| origins.get(field))
        .ok_or_else(|| format!("immediate Audio Info origin {field}"))?;
    assert_eq!(
        origin.get("containsEncryptedSource"),
        Some(&json!(marker)),
        "{field}"
    );
    assert_eq!(
        origin.get("sourceSetComplete"),
        Some(&json!(true)),
        "{field}"
    );
    let references = origin
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or("source use references")?;
    let observations = document
        .pointer("/provenance/uses")
        .and_then(Value::as_array)
        .ok_or("Audio Info source uses")?;
    references
        .iter()
        .map(|reference| {
            observations
                .iter()
                .find(|observation| observation.get("id") == Some(reference))
                .ok_or_else(|| format!("unresolved Audio Info source use {reference}").into())
        })
        .collect()
}
