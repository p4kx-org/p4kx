// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use p4kx_extraction::{Archive, ArchiveWatch};
use p4kx_media::{
    ArtifactExtension, Capability, ExportOptions, MediaError, MediaKind, MediaWriter, Projection,
    Provider, ProviderRegistry, SourceContext,
};
use p4kx_testkit::{ArchiveBuilder, TestEntry};

use crate::exit;

type TestResult = Result<(), Box<dyn std::error::Error>>;

struct ChangedProvider {
    path: PathBuf,
}

impl Provider for ChangedProvider {
    fn capability(&self) -> Capability {
        Capability::new(
            MediaKind::Texture,
            "synthetic.writer-exit",
            "1.0.0",
            "actual changed source during encoded writing",
        )
    }

    fn source(
        &self,
        _context: &SourceContext<'_>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<Projection, MediaError> {
        writer.converted_artifact(ArtifactExtension::new("bin")?, |output| {
            fs::OpenOptions::new()
                .append(true)
                .open(&self.path)
                .and_then(|mut file| file.write_all(b"actual source change"))
                .map_err(|source| MediaError::Io {
                    path: self.path.clone(),
                    source,
                })?;
            std::thread::sleep(p4kx_extraction::DEFAULT_WATCH_INTERVAL);
            output
                .write_all(b"encoded")
                .map_err(|source| MediaError::Io {
                    path: "synthetic encoder".into(),
                    source,
                })
        })?;
        Err(MediaError::Contract(
            "changed writer unexpectedly succeeded".into(),
        ))
    }
}

#[test]
fn actual_changed_from_the_bounded_output_writer_keeps_archive_changed_exit() -> TestResult {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("source.bin", b"H"))
        .write_to(&path)?;
    let archive = Archive::open(&path)?;
    let providers = ProviderRegistry::try_new(vec![Box::new(ChangedProvider { path })])?;
    let output = folder.path().join("media");
    let failure = p4kx_media::export_with(&archive, &output, &ExportOptions::default(), &providers)
        .err()
        .ok_or("changed writer published")?;
    assert!(!output.join("media-index.json").exists());
    assert_eq!(
        super::error_code(&anyhow::Error::new(failure)),
        Some(exit::ARCHIVE_CHANGED)
    );
    Ok(())
}

#[test]
fn an_output_io_wrapper_preserves_the_actual_archive_changed_exit() -> TestResult {
    let changed = changed()?;
    let wrapped = MediaError::Output {
        cause: Arc::new(MediaError::Changed(changed)),
        callback: None,
    };
    assert_eq!(
        super::error_code(&anyhow::Error::new(wrapped)),
        Some(exit::ARCHIVE_CHANGED)
    );
    Ok(())
}

#[test]
fn independent_output_callback_or_postcheck_errors_remain_partial_failures() -> TestResult {
    let changed = changed()?;
    let double = MediaError::OutputCheckFailed {
        operation: Box::new(MediaError::Cancelled),
        checkpoint: Box::new(MediaError::Changed(changed)),
    };
    assert_eq!(
        super::error_code(&anyhow::Error::new(double)),
        Some(exit::PARTIAL_FAILURE)
    );
    let double = MediaError::Output {
        cause: Arc::new(MediaError::Cancelled),
        callback: Some(Box::new(MediaError::Io {
            path: "encoder".into(),
            source: io::Error::from(io::ErrorKind::NotFound),
        })),
    };
    assert_eq!(
        super::error_code(&anyhow::Error::new(double)),
        Some(exit::PARTIAL_FAILURE)
    );
    Ok(())
}

fn changed() -> Result<p4kx_extraction::ArchiveChanged, Box<dyn std::error::Error>> {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("source.bin", b"H"))
        .write_to(&path)?;
    let archive = Archive::open(&path)?;
    let watch = ArchiveWatch::new(&archive).with_interval(Duration::ZERO);
    fs::OpenOptions::new()
        .append(true)
        .open(path)?
        .write_all(b"actual synthetic change")?;
    watch
        .check()
        .err()
        .ok_or_else(|| "actual source change was not observed".into())
}
