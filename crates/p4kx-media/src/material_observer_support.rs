// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use p4kx_extraction::model::{MaterialReadField, MaterialReadObservation, MaterialSourceLimits};
use p4kx_extraction::{Archive, CancelToken, ConflictPolicy};
use p4kx_testkit::{ArchiveBuilder, TestEntry};

use crate::{
    Artifact, Capability, ExportOptions, ExportOutcome, Index, MaterialProvider, MediaError,
    MediaKind, MediaWriter, Projection, Provider, ProviderRegistry, SourceContext,
};

pub(crate) type TestResult<T = ()> = Result<T, Box<dyn Error>>;

pub(crate) struct Probe {
    cancel: CancelToken,
    stop_at: Option<usize>,
    pub(crate) events: AtomicUsize,
    pub(crate) palette_events: AtomicUsize,
}

impl Probe {
    pub(crate) const fn new(cancel: CancelToken, stop_at: Option<usize>) -> Self {
        Self {
            cancel,
            stop_at,
            events: AtomicUsize::new(0),
            palette_events: AtomicUsize::new(0),
        }
    }

    fn observe(&self, event: MaterialReadObservation<'_>) {
        self.events.fetch_add(1, Ordering::SeqCst);
        if event.field == MaterialReadField::Palette && event.source_field == "PaletteTint" {
            let count = self.palette_events.fetch_add(1, Ordering::SeqCst) + 1;
            if self.stop_at == Some(count) {
                self.cancel.cancel();
            }
        }
    }
}

struct ObservedMaterial {
    probe: Arc<Probe>,
    limits: MaterialSourceLimits,
}

impl Provider for ObservedMaterial {
    fn capability(&self) -> Capability {
        MaterialProvider::new_with_limits(self.limits).capability()
    }

    fn source(
        &self,
        context: &SourceContext<'_>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<Projection, MediaError> {
        crate::material_projection::project_observed(context, self.limits, writer, |event| {
            self.probe.observe(event);
            Ok(())
        })
    }
}

pub(crate) fn observed(
    probe: Arc<Probe>,
    limits: MaterialSourceLimits,
) -> Result<ProviderRegistry, MediaError> {
    ProviderRegistry::try_new(vec![Box::new(ObservedMaterial { probe, limits })])
}

pub(crate) fn standard() -> Result<ProviderRegistry, MediaError> {
    ProviderRegistry::try_new(vec![Box::new(MaterialProvider::default())])
}

pub(crate) fn archive(folder: &Path, name: &str, material: &[u8]) -> TestResult<Archive> {
    let path = folder.join(name);
    ArchiveBuilder::new()
        .entry(TestEntry::stored("source.mtl", material))
        .write_to(&path)?;
    Ok(Archive::open(path)?)
}

pub(crate) fn options() -> ExportOptions {
    ExportOptions {
        kinds: vec![MediaKind::Material],
        diagnostic: true,
        ..ExportOptions::default()
    }
}

pub(crate) fn published(outcome: ExportOutcome) -> TestResult<Index> {
    match outcome {
        ExportOutcome::Published { index, .. } => Ok(*index),
        _ => Err("expected actual complete Material generation".into()),
    }
}

pub(crate) fn is_cancelled(error: &MediaError) -> bool {
    let mut current: Option<&(dyn Error + 'static)> = Some(error);
    while let Some(cause) = current {
        if matches!(
            cause.downcast_ref::<MediaError>(),
            Some(MediaError::Cancelled)
        ) {
            return true;
        }
        current = cause.source();
    }
    false
}

pub(crate) fn same_outputs(
    first: &Path,
    baseline: &Index,
    second: &Path,
    observed: &Index,
) -> TestResult {
    assert_eq!(
        serde_json::to_value(&baseline.kinds)?,
        serde_json::to_value(&observed.kinds)?
    );
    let receipts = baseline
        .field_origins
        .as_ref()
        .ok_or("baseline origin receipts")?;
    assert_eq!(
        serde_json::to_value(receipts)?,
        serde_json::to_value(
            observed
                .field_origins
                .as_ref()
                .ok_or("observed origin receipts")?
        )?
    );
    for artifact in baseline.kinds.iter().map(|kind| &kind.artifact).chain([
        &baseline.source_manifest,
        &baseline.artifact_manifest,
        &receipts.sources,
        &receipts.uses,
        &receipts.fields,
    ]) {
        same_file(first, second, &artifact.path)?;
    }
    for line in fs::read_to_string(first.join(&baseline.artifact_manifest.path))?.lines() {
        let artifact: Artifact = serde_json::from_str(line)?;
        same_file(first, second, &artifact.path)?;
    }
    Ok(())
}

fn same_file(first: &Path, second: &Path, relative: &str) -> TestResult {
    assert_eq!(
        fs::read(first.join(relative))?,
        fs::read(second.join(relative))?,
        "the actual noop adapter preserves {relative} bytes"
    );
    Ok(())
}
pub(crate) struct PreviousGeneration {
    pub(crate) folder: tempfile::TempDir,
    pub(crate) output: PathBuf,
    archive: Archive,
    index: Index,
    committed: Vec<u8>,
    registry: ProviderRegistry,
}

impl PreviousGeneration {
    pub(crate) fn new() -> TestResult<Self> {
        let folder = tempfile::tempdir()?;
        let archive = archive(folder.path(), "previous.p4k", b"<Material/>")?;
        let output = folder.path().join("media");
        let registry = standard()?;
        let index = published(crate::export_with(
            &archive,
            &output,
            &options(),
            &registry,
        )?)?;
        let committed = fs::read(output.join("media-index.json"))?;
        Ok(Self {
            folder,
            output,
            archive,
            index,
            committed,
            registry,
        })
    }

    pub(crate) fn assert_unchanged(&self) -> TestResult {
        assert_eq!(
            fs::read(self.output.join("media-index.json"))?,
            self.committed,
            "the prior complete index survives"
        );
        let mut reuse = options();
        reuse.conflict = ConflictPolicy::Skip;
        let ExportOutcome::Skipped { index } =
            crate::export_with(&self.archive, &self.output, &reuse, &self.registry)?
        else {
            return Err("previous complete receipt reuse was not preserved".into());
        };
        assert_eq!(index.generation, self.index.generation);
        Ok(())
    }
}
