// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::io::Write;
use std::path::PathBuf;

use p4kx_extraction::{CancelToken, ConflictPolicy};
use p4kx_media::{
    Capability, ExportOptions, MediaError, MediaKind, MediaWriter, Projection, Provider,
    ProviderRegistry, SourceContext, Status, Subresource,
};

use super::support::{self, TestResult};

enum Interruption {
    Cancel(CancelToken),
    Change(PathBuf),
}

struct InterruptingProvider(Interruption);

impl Provider for InterruptingProvider {
    fn capability(&self) -> Capability {
        Capability::new(
            MediaKind::Material,
            "synthetic.interruption",
            "1.0.0",
            "centrally retained decoded source",
        )
    }

    fn source(
        &self,
        _source: &SourceContext<'_>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<Projection, MediaError> {
        match &self.0 {
            Interruption::Cancel(token) => token.cancel(),
            Interruption::Change(path) => fs::OpenOptions::new()
                .append(true)
                .open(path)
                .and_then(|mut file| file.write_all(b"changed"))
                .map_err(|source| MediaError::Io {
                    path: path.clone(),
                    source,
                })?,
        }
        writer.record(
            &Subresource::new("interruption", vec![0]),
            Status::Complete,
            &0,
            &[],
        )?;
        Ok(Projection::Written)
    }
}

#[test]
fn cancellation_after_source_retention_keeps_the_prior_complete_index() -> TestResult {
    let folder = tempfile::tempdir()?;
    let archive = support::archive(folder.path())?;
    let output = folder.path().join("media");
    p4kx_media::export(&archive, &output, &ExportOptions::default())?;
    let prior = fs::read(output.join("media-index.json"))?;
    let token = CancelToken::new();
    let registry = ProviderRegistry::try_new(vec![Box::new(InterruptingProvider(
        Interruption::Cancel(token.clone()),
    ))])?;
    let options = ExportOptions {
        conflict: ConflictPolicy::Overwrite,
        cancel: token,
        ..ExportOptions::default()
    };
    let error = p4kx_media::export_with(&archive, &output, &options, &registry)
        .err()
        .ok_or("cancelled provider published")?;
    let MediaError::Source { cause, entry } = error else {
        return Err("cancelled source provenance".into());
    };
    assert!(
        entry.raw.is_some(),
        "cancellation occurred after verified retention"
    );
    assert!(matches!(*cause, MediaError::Cancelled));
    assert_eq!(fs::read(output.join("media-index.json"))?, prior);
    Ok(())
}

#[test]
fn a_real_source_change_during_provider_work_is_checked_before_commit() -> TestResult {
    let folder = tempfile::tempdir()?;
    let archive = support::archive(folder.path())?;
    let output = folder.path().join("media");
    p4kx_media::export(&archive, &output, &ExportOptions::default())?;
    let prior = fs::read(output.join("media-index.json"))?;
    let registry = ProviderRegistry::try_new(vec![Box::new(InterruptingProvider(
        Interruption::Change(archive.path().into()),
    ))])?;
    let options = ExportOptions {
        conflict: ConflictPolicy::Overwrite,
        ..ExportOptions::default()
    };
    let error = p4kx_media::export_with(&archive, &output, &options, &registry)
        .err()
        .ok_or("changed Archive published")?;
    assert!(
        is_changed(&error),
        "original typed ArchiveWatch error must survive"
    );
    assert_eq!(fs::read(output.join("media-index.json"))?, prior);
    Ok(())
}

fn is_changed(error: &MediaError) -> bool {
    match error {
        MediaError::Changed(_) => true,
        MediaError::Source { cause, .. } | MediaError::ExtractionInterrupted { cause, .. } => {
            is_changed(cause)
        }
        _ => false,
    }
}
