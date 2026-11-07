// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_media::{
    Capability, MaterialProvider, MediaError, MediaWriter, Projection, Provider, ProviderRegistry,
    SourceContext,
};
use p4kx_testkit::TestEntry;

use super::material_support::{self as support, TestResult};

struct CancelAtMaterial(p4kx_extraction::CancelToken);

impl Provider for CancelAtMaterial {
    fn capability(&self) -> Capability {
        MaterialProvider::default().capability()
    }

    fn source(
        &self,
        context: &SourceContext<'_>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<Projection, MediaError> {
        self.0.cancel();
        MaterialProvider::default().source(context, writer)
    }
}

fn is_cancelled(error: &MediaError) -> bool {
    match error {
        MediaError::Cancelled => true,
        MediaError::Source { cause, .. } => is_cancelled(cause),
        _ => false,
    }
}

#[test]
fn a_cancelled_actual_provider_callback_keeps_the_original_cancellation_category() -> TestResult {
    let folder = tempfile::tempdir()?;
    let archive = support::archive(
        folder.path(),
        [TestEntry::stored("source.mtl", b"<Material/>")],
    )?;
    let options = support::options();
    let registry =
        ProviderRegistry::try_new(vec![Box::new(CancelAtMaterial(options.cancel.clone()))])?;
    let output = folder.path().join("out");
    let error = p4kx_media::export_with(&archive, &output, &options, &registry)
        .err()
        .ok_or("cancelled material source published")?;
    assert!(
        is_cancelled(&error),
        "caller cancellation is not reclassified as malformed material data"
    );
    assert!(
        !output.join("media-index.json").exists(),
        "cancelled work publishes no complete generation"
    );
    Ok(())
}

#[test]
fn a_small_record_budget_refuses_publication_without_fabricating_an_empty_material() -> TestResult {
    let folder = tempfile::tempdir()?;
    let archive = support::archive(
        folder.path(),
        [TestEntry::stored(
            "source.mtl",
            b"<Material Name='bounded'/>",
        )],
    )?;
    let mut options = support::options();
    options.limits.record_bytes = 128;
    let output = folder.path().join("out");
    let result = p4kx_media::export_with(
        &archive,
        &output,
        &options,
        &support::registry(MaterialProvider::default())?,
    );
    assert!(
        result.is_err(),
        "actual serialized record bytes obey the common hard bound"
    );
    assert!(
        !output.join("media-index.json").exists(),
        "a failed bounded record cannot publish a complete empty source"
    );
    Ok(())
}
