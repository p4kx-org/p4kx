// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::io::Read;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use p4kx_extraction::{ConflictPolicy, Filter};
use p4kx_media::{
    Capability, ExportOptions, ExportOutcome, InputReceipt, MediaError, MediaKind, MediaWriter,
    OriginalIdentifier, Projection, Provider, ProviderRegistry, SourceContext, Status, Subresource,
};
use p4kx_testkit::TestEntry;

use super::support::{self, TestResult};

enum Mode {
    Observe,
    CorruptSameLength(Arc<AtomicBool>),
}

struct CacheProbe(Mode);

impl Provider for CacheProbe {
    fn capability(&self) -> Capability {
        Capability::new(
            MediaKind::Texture,
            "synthetic.cache-integrity",
            "1.0.0",
            "actual required-input and final generation integrity evidence",
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
        let query = context.directory.lookup_input(
            OriginalIdentifier::new_with_path("owner.dds.1"),
            "owner.dds.1",
            Some(MediaKind::Texture),
            writer,
        )?;
        let token = query
            .input
            .ok_or_else(|| MediaError::Contract("actual input token".into()))?;
        let receipt = context.with_required_input(
            &token,
            &Subresource::new("synthetic.cache", vec![0]),
            writer,
            |input, _writer| {
                let mut bytes = Vec::new();
                input
                    .read_to_end(&mut bytes)
                    .map_err(|source| crate_io("first", source))?;
                if bytes != b"needed" {
                    return Err(MediaError::Contract("wrong actual initial input".into()));
                }
                Ok(input.receipt().clone())
            },
        )?;
        if let Mode::CorruptSameLength(corrupted) = &self.0 {
            corrupt_cache(context, &receipt, corrupted)?;
            context.with_required_input(
                &token,
                &Subresource::new("synthetic.cache", vec![1]),
                writer,
                |input, _writer| {
                    let mut bytes = Vec::new();
                    input
                        .read_to_end(&mut bytes)
                        .map_err(|source| crate_io("second", source))?;
                    Ok(())
                },
            )?;
        }
        writer.record(
            &Subresource::new("synthetic.cache", vec![0]),
            Status::Complete,
            &receipt,
            &[],
        )?;
        Ok(Projection::Written)
    }
}

fn corrupt_cache(
    context: &SourceContext<'_>,
    receipt: &InputReceipt,
    corrupted: &AtomicBool,
) -> Result<(), MediaError> {
    let raw = receipt
        .source
        .raw
        .as_ref()
        .ok_or_else(|| MediaError::Contract("published input".into()))?;
    let stage = context
        .payload
        .parent()
        .and_then(Path::parent)
        .ok_or_else(|| MediaError::Contract("source staging parent".into()))?;
    let path = stage.join(&raw.path);
    fs::write(&path, b"other!").map_err(|source| crate_io("cached-raw", source))?;
    let changed = fs::read(&path).map_err(|source| crate_io("cached-readback", source))?;
    if changed != b"other!" {
        return Err(MediaError::Contract("cached corruption readback".into()));
    }
    corrupted.store(true, Ordering::Relaxed);
    Ok(())
}

fn crate_io(name: &str, source: std::io::Error) -> MediaError {
    MediaError::Io {
        path: name.into(),
        source,
    }
}

#[test]
fn same_length_cached_corruption_cannot_replace_a_complete_generation() -> TestResult {
    let folder = tempfile::tempdir()?;
    let archive = support::archive_with(
        folder.path(),
        [
            TestEntry::stored("owner.dds", b"H"),
            TestEntry::stored("owner.dds.1", b"needed"),
        ],
    )?;
    let output = folder.path().join("media");
    let mut options = ExportOptions {
        include: Some(Filter::parse("*.dds")?),
        ..ExportOptions::default()
    };
    let valid = ProviderRegistry::try_new(vec![Box::new(CacheProbe(Mode::Observe))])?;
    support::published(p4kx_media::export_with(
        &archive, &output, &options, &valid,
    )?)?;
    let previous = fs::read(output.join("media-index.json"))?;
    options.conflict = ConflictPolicy::Overwrite;
    let corrupted = Arc::new(AtomicBool::new(false));
    let corrupt = ProviderRegistry::try_new(vec![Box::new(CacheProbe(Mode::CorruptSameLength(
        Arc::clone(&corrupted),
    )))])?;
    let result = p4kx_media::export_with(&archive, &output, &options, &corrupt);
    assert!(
        corrupted.load(Ordering::Relaxed),
        "the actual retained payload was changed and read back",
    );
    assert!(
        result.is_err(),
        "cached corruption is rejected before any complete-generation publication",
    );
    assert_eq!(fs::read(output.join("media-index.json"))?, previous);
    options.conflict = ConflictPolicy::Skip;
    assert!(
        matches!(
            p4kx_media::export_with(&archive, &output, &options, &valid)?,
            ExportOutcome::Skipped { .. }
        ),
        "the earlier complete generation still passes actual artifact validation",
    );
    Ok(())
}
