// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::io::{Read, Write};
use std::path::PathBuf;

use p4kx_extraction::{Archive, CancelToken, Exclusion};
use p4kx_media::{
    Capability, ExportOptions, MediaError, MediaKind, MediaWriter, OriginalIdentifier, Projection,
    Provider, ProviderRegistry, SourceContext, Subresource,
};
use p4kx_testkit::{ArchiveBuilder, TestEntry};

use crate::exit;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[derive(Clone, Copy)]
enum Mode {
    Changed,
    Cancelled,
    Limit,
}

struct ReaderProvider {
    source: PathBuf,
    cancel: CancelToken,
    mode: Mode,
}

impl Provider for ReaderProvider {
    fn capability(&self) -> Capability {
        Capability::new(
            MediaKind::Texture,
            "synthetic.reader-exit",
            "1.0.0",
            "required-input reader interruption classification",
        )
    }

    fn source(
        &self,
        context: &SourceContext<'_>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<Projection, MediaError> {
        if context.entry.path() != "owner.dds" {
            return Ok(Projection::Skipped);
        }
        let lookup = context.directory.lookup_input(
            OriginalIdentifier::new_with_path("owner.dds.1"),
            "owner.dds.1",
            Some(MediaKind::Texture),
            writer,
        )?;
        let input = lookup
            .input
            .ok_or_else(|| MediaError::Contract("unique synthetic part".into()))?;
        context.with_required_input(
            &input,
            &Subresource::new("synthetic.required", vec![0]),
            writer,
            |reader, _writer| {
                match self.mode {
                    Mode::Changed => {
                        fs::OpenOptions::new()
                            .append(true)
                            .open(&self.source)
                            .and_then(|mut file| file.write_all(b"changed"))
                            .map_err(|source| MediaError::Io {
                                path: self.source.clone(),
                                source,
                            })?;
                        std::thread::sleep(p4kx_extraction::DEFAULT_WATCH_INTERVAL);
                    }
                    Mode::Cancelled => self.cancel.cancel(),
                    Mode::Limit => {}
                }
                let mut byte = [0];
                reader
                    .read_exact(&mut byte)
                    .map_err(|source| MediaError::Io {
                        path: "synthetic-reader".into(),
                        source,
                    })
            },
        )?;
        Err(MediaError::Contract(
            "interrupted reader unexpectedly succeeded".into(),
        ))
    }
}

#[test]
fn changed_from_the_actual_required_reader_keeps_the_archive_changed_exit() -> TestResult {
    assert_exit(Mode::Changed, exit::ARCHIVE_CHANGED)
}

#[test]
fn cancelled_from_the_actual_required_reader_keeps_partial_failure_exit() -> TestResult {
    assert_exit(Mode::Cancelled, exit::PARTIAL_FAILURE)
}

#[test]
fn a_required_reader_budget_failure_keeps_partial_failure_exit() -> TestResult {
    assert_exit(Mode::Limit, exit::PARTIAL_FAILURE)
}

fn assert_exit(mode: Mode, expected: u8) -> TestResult {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("owner.dds", b"H"))
        .entry(TestEntry::stored("owner.dds.1", b"needed"))
        .write_to(&path)?;
    let archive = Archive::open(&path)?;
    let cancel = CancelToken::new();
    let provider = ReaderProvider {
        source: path,
        cancel: cancel.clone(),
        mode,
    };
    let providers = ProviderRegistry::try_new(vec![Box::new(provider)])?;
    let mut options = ExportOptions {
        exclude: Exclusion::parse(["*.dds.1"])?,
        cancel,
        ..ExportOptions::default()
    };
    if matches!(mode, Mode::Limit) {
        options.limits.source_bytes = 7;
    }
    let output = folder.path().join("media");
    let failure = p4kx_media::export_with(&archive, &output, &options, &providers)
        .err()
        .ok_or("interrupted input published")?;
    assert!(
        !output.join("media-index.json").exists(),
        "reader interruption cannot publish"
    );
    let failure = anyhow::Error::new(failure);
    assert_eq!(
        super::error_code(&failure),
        Some(expected),
        "actual reader error uses the existing Media CLI classification",
    );
    Ok(())
}
