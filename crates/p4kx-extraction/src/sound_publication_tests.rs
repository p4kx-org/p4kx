// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;
use std::path::Path;

use p4kx_testkit::{ArchiveBuilder, TestCodec, TestEntry, TestWem};

use crate::{
    Archive, Checksum, ConversionError, Sound, SourceUses, WriteProtectionError,
    write_origin_companion,
};

use super::SoundWrite;

type TestResult = Result<(), Box<dyn Error>>;

const ENTRY: &str = "Data/Sounds/payload.wem";
const AUDIO: &[u8] = b"measured audio";
const FOREIGN: &[u8] = b"different regular file";

#[test]
fn a_replaced_sound_cannot_receive_the_committed_descriptors_receipt() -> TestResult {
    let directory = tempfile::tempdir()?;
    let sound = archive_sound(directory.path())?;
    let output = directory.path().join("result.wav");
    let displaced = directory.path().join("displaced.wav");
    let receipt = crate::file_receipt::companion_path(
        &fs::canonicalize(directory.path())?.join("result.wav"),
    )?;
    let inventory = SourceUses::new().freeze()?;

    let error = sound
        .publish_sound(
            &output,
            |writer, _| {
                writer
                    .write_all(AUDIO)
                    .map_err(|cause| crate::output_file::write_error(&output, cause))?;
                Ok(SoundWrite {
                    report: (),
                    byte_length: AUDIO.len() as u64,
                    origins: (),
                })
            },
            |(), writer, binding| {
                write_origin_companion(writer, binding, &inventory, [])?;
                fs::rename(&output, &displaced)?;
                fs::write(&output, FOREIGN)
            },
        )
        .err()
        .ok_or("the replacement received the committed sound's receipt")?;
    assert!(
        matches!(
            error,
            ConversionError::Write { ref path, ref source }
                if path == &receipt
                    && source
                        .get_ref()
                        .and_then(|cause| cause.downcast_ref::<WriteProtectionError>())
                        .is_some_and(|cause| matches!(cause, WriteProtectionError::TargetIdentity { .. }))
        ),
        "{error:?}"
    );
    assert_eq!(fs::read(&output)?, FOREIGN);
    assert_eq!(fs::read(&displaced)?, AUDIO);
    assert!(!receipt.exists(), "no stale companion is left");
    assert_eq!(
        fs::read_dir(directory.path())?.count(),
        3,
        "retire only the owned staging and lease"
    );
    Ok(())
}

#[test]
fn a_linear_wav_publishes_its_exact_byte_binding_and_source_summary() -> TestResult {
    use sha2::{Digest as _, Sha256};

    let directory = tempfile::tempdir()?;
    let sound = archive_sound(directory.path())?;
    let output = directory.path().join("result.wav");
    let written = sound.write_linear_wav_with_receipt(&output)?;
    let bytes = fs::read(output)?;
    assert!(bytes.starts_with(b"RIFF"));
    assert_eq!(written.report.size, bytes.len() as u64);
    let rows = fs::read_to_string(&written.companion)?;
    let mut rows = rows.lines();
    let header: serde_json::Value = serde_json::from_str(rows.next().ok_or("receipt header")?)?;
    let digest = Checksum::Sha256(Sha256::digest(&bytes).into()).hex();
    assert_eq!(
        header
            .pointer("/binding/sha256")
            .and_then(serde_json::Value::as_str),
        Some(digest.as_str())
    );
    let end: serde_json::Value = serde_json::from_str(rows.last().ok_or("receipt terminator")?)?;
    assert_eq!(
        end.get("type").and_then(serde_json::Value::as_str),
        Some("end")
    );
    assert_ne!(written.summary_provenance()?.iter_sources().len(), 0);
    Ok(())
}

fn archive_sound(directory: &Path) -> Result<Sound, Box<dyn Error>> {
    let path = directory.join("source.p4k");
    let bytes = TestWem::pcm(TestCodec::WwisePcm, 1, 48_000, &[1, 2, 3, 4]).bytes();
    ArchiveBuilder::new()
        .entry(TestEntry::stored(ENTRY, bytes.as_slice()))
        .write_to(&path)?;
    Ok(Sound::from_archive_observed(&Archive::open(&path)?, ENTRY)?)
}
