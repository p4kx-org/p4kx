// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::io;

use p4kx_extraction::{CancelToken, ConflictPolicy};
use p4kx_media::{
    Capability, MediaError, MediaWriter, Projection, Provider, ProviderRegistry, ShaderProvider,
    SourceContext,
};
use p4kx_testkit::TestEntry;

use super::shader_support::{self as support, TestResult};

struct CancelBeforeShader(CancelToken);

impl Provider for CancelBeforeShader {
    fn capability(&self) -> Capability {
        ShaderProvider::default().capability()
    }

    fn source(
        &self,
        context: &SourceContext<'_>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<Projection, MediaError> {
        self.0.cancel();
        ShaderProvider::default().source(context, writer)
    }
}

struct RemoveSyntheticPayload;

impl Provider for RemoveSyntheticPayload {
    fn capability(&self) -> Capability {
        ShaderProvider::default().capability()
    }

    fn source(
        &self,
        context: &SourceContext<'_>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<Projection, MediaError> {
        fs::remove_file(context.payload).map_err(|source| MediaError::Io {
            path: context.payload.to_path_buf(),
            source,
        })?;
        ShaderProvider::default().source(context, writer)
    }
}

fn cancelled(error: &MediaError) -> bool {
    match error {
        MediaError::Cancelled => true,
        MediaError::Source { cause, .. } => cancelled(cause),
        _ => false,
    }
}

fn missing_payload(error: &MediaError) -> bool {
    match error {
        MediaError::Io { source, .. } => source.kind() == io::ErrorKind::NotFound,
        MediaError::Source { cause, .. } => missing_payload(cause),
        _ => false,
    }
}

fn record_budget(error: &MediaError) -> bool {
    match error {
        MediaError::Limit {
            budget: "record_bytes",
            ..
        } => true,
        MediaError::Json(error) => {
            error.is_io() && error.to_string() == "media record_bytes budget exceeded"
        }
        MediaError::Source { cause, .. } => record_budget(cause),
        _ => false,
    }
}

#[test]
fn cancellation_at_the_real_shader_callback_preserves_original_cancel_failure() -> TestResult {
    let folder = tempfile::tempdir()?;
    let archive = support::archive(
        folder.path(),
        [TestEntry::stored("source.ext", b"Property { Name(%A); }")],
    )?;
    let options = support::options();
    let registry =
        ProviderRegistry::try_new(vec![Box::new(CancelBeforeShader(options.cancel.clone()))])?;
    let output = folder.path().join("out");
    let error = p4kx_media::export_with(&archive, &output, &options, &registry)
        .err()
        .ok_or("cancelled callback published output")?;
    assert!(
        cancelled(&error),
        "shader callback does not stringify cancellation or relabel it as syntax failure"
    );
    assert!(
        !output.join("media-index.json").exists(),
        "cancelled callback cannot publish a complete generation"
    );
    Ok(())
}

#[test]
fn retained_payload_io_failure_keeps_the_original_io_kind() -> TestResult {
    let folder = tempfile::tempdir()?;
    let archive = support::archive(
        folder.path(),
        [TestEntry::stored("source.ext", b"Property { Name(%A); }")],
    )?;
    let registry = ProviderRegistry::try_new(vec![Box::new(RemoveSyntheticPayload)])?;
    let output = folder.path().join("out");
    let error = p4kx_media::export_with(&archive, &output, &support::options(), &registry)
        .err()
        .ok_or("missing retained source was treated as empty shader")?;
    assert!(
        missing_payload(&error),
        "shader source failure retains NotFound through common physical-source wrapping"
    );
    assert!(
        !output.join("media-index.json").exists(),
        "unreadable retained payload cannot claim a complete generation"
    );
    Ok(())
}

#[test]
fn bounded_record_failure_does_not_replace_a_prior_complete_index() -> TestResult {
    let folder = tempfile::tempdir()?;
    let source = format!("Property {{ Description(\"{}\"); }}", "x".repeat(24 << 10));
    let archive = support::archive(
        folder.path(),
        [TestEntry::stored("source.ext", source.as_bytes())],
    )?;
    let output = folder.path().join("out");
    let registry = support::registry(ShaderProvider::default())?;
    support::published(p4kx_media::export_with(
        &archive,
        &output,
        &support::options(),
        &registry,
    )?)?;
    let original = fs::read(output.join("media-index.json"))?;
    let mut options = support::options();
    options.conflict = ConflictPolicy::Overwrite;
    options.limits.record_bytes = 16 << 10;
    let error = p4kx_media::export_with(&archive, &output, &options, &registry)
        .err()
        .ok_or("oversized shader metadata row was published")?;
    assert!(
        record_budget(&error),
        "large authored value hits the actual bounded writer, not truncation or subtree duplication"
    );
    assert_eq!(
        fs::read(output.join("media-index.json"))?,
        original,
        "failed shader row preserves the existing complete commit point"
    );
    Ok(())
}
