// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The `--json` output of `p4kx audio`, as snapshots.

use std::error::Error;
use std::path::{Path, PathBuf};

use p4kx_testkit::{ArchiveBuilder, TestCodec, TestEntry, TestWem, test_signal};
use serde_json::Value;

#[path = "json_audio_origins.rs"]
mod origins;

type TestResult = Result<(), Box<dyn Error>>;

const SOUND: &str = "Data\\Sounds\\wwise\\Media\\100.wem";

fn sound_archive(dir: &Path) -> Result<PathBuf, Box<dyn Error>> {
    let archive = dir.join("Data.p4k");
    let wem = TestWem::pcm(TestCodec::WwisePcm, 2, 48_000, &test_signal(1_200, 2));
    ArchiveBuilder::new()
        .entry(TestEntry::stored(SOUND, wem.bytes()))
        .write_to(&archive)?;
    Ok(archive)
}

fn json(dir: &Path, args: &[&std::ffi::OsStr]) -> Result<Value, Box<dyn Error>> {
    let output = p4kx_testkit::p4kx()?.arg("--json").args(args).output()?;
    assert_eq!(output.status.code(), Some(0), "exit code: {output:?}");
    let mut value: Value = serde_json::from_slice(&output.stdout)?;
    if let Some(Value::String(text)) = value.get_mut("output")
        && let Some(rest) = text.strip_prefix(&dir.display().to_string())
    {
        *text = format!("[DIR]{}", rest.replace('\\', "/"));
    }
    Ok(value)
}

#[test]
fn audio_info() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = sound_archive(dir.path())?;

    let mut value = json(
        dir.path(),
        &[
            "audio".as_ref(),
            "info".as_ref(),
            archive.as_os_str(),
            SOUND.as_ref(),
        ],
    )?;

    remove_info_metadata(&mut value)?;
    let json = serde_json::to_string_pretty(&value)?;
    insta::assert_snapshot!(json);
    Ok(())
}

pub(super) fn remove_info_metadata(value: &mut Value) -> TestResult {
    let fields = value.as_object_mut().ok_or("Audio Info JSON object")?;
    for key in ["fieldOrigins", "provenance"] {
        assert!(fields.remove(key).is_some(), "Audio Info includes {key}");
    }
    Ok(())
}

#[test]
fn audio_wav() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = sound_archive(dir.path())?;
    let out = dir.path().join("100.wav");

    let value = json(
        dir.path(),
        &[
            "audio".as_ref(),
            "wav".as_ref(),
            archive.as_os_str(),
            SOUND.as_ref(),
            "--out".as_ref(),
            out.as_os_str(),
        ],
    )?;

    let json = serde_json::to_string_pretty(&value)?;
    insta::assert_snapshot!(json);
    Ok(())
}
