// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;

use p4kx_extraction::Archive;
use p4kx_media::{
    Capability, ExportOptions, MediaError, MediaKind, MediaWriter, Projection, Provider,
    ProviderRegistry, SourceContext, Status, Subresource,
};
use p4kx_testkit::{ArchiveBuilder, TestEntry};
use serde::{Serialize, Serializer};

use crate::exit;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[derive(Clone, Copy)]
enum Mode {
    Changed,
    DataAndChanged,
}

struct SerialisingProvider {
    path: PathBuf,
    mode: Mode,
}

impl Provider for SerialisingProvider {
    fn capability(&self) -> Capability {
        Capability::new(
            MediaKind::Texture,
            "synthetic.serialization-exit",
            "1.0.0",
            "actual guarded record serialization failures",
        )
    }

    fn source(
        &self,
        _context: &SourceContext<'_>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<Projection, MediaError> {
        writer.record(
            &Subresource::new("synthetic.serialization", vec![0]),
            Status::Complete,
            self,
            &[],
        )?;
        Err(MediaError::Contract(
            "interrupted record serialization unexpectedly succeeded".into(),
        ))
    }
}

impl Serialize for SerialisingProvider {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::Error as _;
        const LARGE_RECORD_BYTES: usize = 128 << 10;
        OpenOptions::new()
            .append(true)
            .open(&self.path)
            .and_then(|mut file| file.write_all(b"actual serialization source change"))
            .map_err(S::Error::custom)?;
        std::thread::sleep(p4kx_extraction::DEFAULT_WATCH_INTERVAL);
        match self.mode {
            Mode::Changed => serializer.serialize_str(&"x".repeat(LARGE_RECORD_BYTES)),
            Mode::DataAndChanged => Err(S::Error::custom(
                "synthetic independent serialization failure",
            )),
        }
    }
}

#[test]
fn actual_guarded_record_serialization_changed_keeps_archive_changed_exit() -> TestResult {
    assert_exit(Mode::Changed, exit::ARCHIVE_CHANGED)
}

#[test]
fn actual_independent_record_json_and_source_failures_keep_partial_failure_exit() -> TestResult {
    assert_exit(Mode::DataAndChanged, exit::PARTIAL_FAILURE)
}

fn assert_exit(mode: Mode, expected: u8) -> TestResult {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("source.bin", b"H"))
        .write_to(&path)?;
    let archive = Archive::open(&path)?;
    let providers = ProviderRegistry::try_new(vec![Box::new(SerialisingProvider { path, mode })])?;
    let output = folder.path().join("media");
    let failure = p4kx_media::export_with(&archive, &output, &ExportOptions::default(), &providers)
        .err()
        .ok_or("interrupted serialization published")?;
    assert!(!output.join("media-index.json").exists());
    assert_eq!(
        super::error_code(&anyhow::Error::new(failure)),
        Some(expected)
    );
    Ok(())
}
