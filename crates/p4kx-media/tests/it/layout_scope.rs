// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::io::Write;
use std::path::PathBuf;

use p4kx_extraction::{Archive, CancelToken, ConflictPolicy, Filter, MetadataSection};
use p4kx_media::{
    Capability, DirectoryContext, ExportOptions, Identity, Limits, MediaError, MediaKind,
    MediaWriter, Projection, Provider, ProviderRegistry, SourceContext, Status, Subresource,
};
use p4kx_testkit::{ArchiveBuilder, TestEntry};

use super::layout_fixture;
use super::layout_health::leaf;
use super::support::TestResult;

enum Interruption {
    Cancel(CancelToken),
    Change(PathBuf),
}

struct Probe {
    interruption: Option<Interruption>,
}

impl Provider for Probe {
    fn capability(&self) -> Capability {
        Capability::new(
            MediaKind::Archive,
            "test.directory-one-byte",
            "1.0.0",
            "one source-bound byte per opened Archive",
        )
    }

    fn directory(
        &self,
        context: &mut DirectoryContext<'_, '_, '_>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        if matches!(context.source.identity, Identity::Entry { .. }) {
            match &self.interruption {
                Some(Interruption::Cancel(cancel)) => cancel.cancel(),
                Some(Interruption::Change(path)) => {
                    fs::OpenOptions::new()
                        .append(true)
                        .open(path)
                        .and_then(|mut file| file.write_all(b"changed"))
                        .map_err(|source| MediaError::Io {
                            path: path.clone(),
                            source,
                        })?;
                }
                None => {}
            }
        }
        let span = context
            .view
            .span(MetadataSection::ZipEnd)?
            .ok_or_else(|| MediaError::Contract("probe ZIP end missing".into()))?;
        let mut byte = [0; 1];
        context.access.read_into(span, 0, &mut byte)?;
        writer.record(
            &Subresource::new("test-directory-byte", Vec::new()),
            Status::Complete,
            &byte,
            &[],
        )
    }

    fn source(
        &self,
        _context: &SourceContext<'_>,
        _writer: &mut MediaWriter<'_>,
    ) -> Result<Projection, MediaError> {
        Ok(Projection::Skipped)
    }
}

fn outer(root: &std::path::Path) -> Result<(Archive, u64), Box<dyn std::error::Error>> {
    let fixture = layout_fixture::zip(&[])?;
    let size = u64::try_from(fixture.bytes.len())?;
    let path = root.join("outer.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("inner.socpak", fixture.bytes.as_slice()))
        .write_to(&path)?;
    Ok((Archive::open(path)?, size))
}

#[test]
fn metadata_bytes_and_work_are_shared_across_root_and_nested_views() -> TestResult {
    let folder = tempfile::tempdir()?;
    let (archive, nested_size) = outer(folder.path())?;
    let registry = ProviderRegistry::try_new(vec![Box::new(Probe { interruption: None })])?;
    for (budget, limit) in [("source_bytes", nested_size + 1), ("metadata_work", 1)] {
        let mut limits = Limits::default();
        match budget {
            "source_bytes" => limits.source_bytes = limit,
            _ => limits.metadata_work = limit,
        }
        let output = folder.path().join(budget);
        let options = ExportOptions {
            limits,
            include: Some(Filter::parse("*.not-selected")?),
            ..ExportOptions::default()
        };
        let failure = p4kx_media::export_with(&archive, &output, &options, &registry)
            .err()
            .ok_or("a reset per-view budget accepted the second read")?;
        let MediaError::Source { entry, .. } = &failure else {
            return Err("missing failed source provenance".into());
        };
        assert!(
            matches!(&entry.identity, Identity::Entry { containers, entry } if containers.is_empty() && entry.entry_index == 0),
            "second view is the exact parent container"
        );
        assert!(
            matches!(leaf(&failure), MediaError::Limit { budget: actual, limit: actual_limit } if *actual == budget && *actual_limit == limit),
            "original global budget failure"
        );
        assert!(!output.join("media-index.json").exists());
    }
    Ok(())
}

#[test]
fn actual_cancel_and_source_watch_during_directory_access_preserve_the_old_index() -> TestResult {
    for name in ["cancel", "change"] {
        let folder = tempfile::tempdir()?;
        let (archive, _) = outer(folder.path())?;
        let output = folder.path().join("media");
        p4kx_media::export(&archive, &output, &ExportOptions::default())?;
        let prior = fs::read(output.join("media-index.json"))?;
        let cancel = CancelToken::new();
        let interruption = if name == "cancel" {
            Interruption::Cancel(cancel.clone())
        } else {
            Interruption::Change(archive.path().to_path_buf())
        };
        let registry = ProviderRegistry::try_new(vec![Box::new(Probe {
            interruption: Some(interruption),
        })])?;
        let options = ExportOptions {
            conflict: ConflictPolicy::Overwrite,
            include: Some(Filter::parse("*.not-selected")?),
            cancel,
            ..ExportOptions::default()
        };
        let failure = p4kx_media::export_with(&archive, &output, &options, &registry)
            .err()
            .ok_or("interrupted metadata export published")?;
        if name == "cancel" {
            assert!(
                matches!(leaf(&failure), MediaError::Cancelled),
                "actual cancellation flag reaches the reader"
            );
        } else {
            assert!(
                matches!(leaf(&failure), MediaError::Changed(_)),
                "forced final source watch retains its typed cause"
            );
        }
        assert_eq!(fs::read(output.join("media-index.json"))?, prior);
    }
    Ok(())
}
