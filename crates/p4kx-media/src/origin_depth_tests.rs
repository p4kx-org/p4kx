// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;

use p4kx_extraction::Archive;
use p4kx_testkit::{ArchiveBuilder, TestContainer, TestEntry};

use crate::origin_test_support::{self as support, TestResult};
use crate::{
    Capability, ExportOptions, MediaError, MediaKind, MediaWriter, Projection, Provider,
    ProviderRegistry, SourceContext,
};

struct SkippedProvider;

impl Provider for SkippedProvider {
    fn capability(&self) -> Capability {
        Capability::new(
            MediaKind::Model,
            "synthetic.depth-only",
            "1.0.0",
            "retained raw sources",
        )
    }

    fn source(
        &self,
        _source: &SourceContext<'_>,
        _writer: &mut MediaWriter<'_>,
    ) -> Result<Projection, MediaError> {
        Ok(Projection::Skipped)
    }
}

#[test]
fn low_record_depth_preserves_nested_source_and_artifact_manifest_admission() -> TestResult {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("Data.p4k");
    let inner = TestContainer::new()
        .entry(TestEntry::stored("leaf.bin", b"data"))
        .build();
    ArchiveBuilder::new()
        .entry(TestEntry::stored("container.socpak", inner))
        .write_to(&path)?;
    let archive = Archive::open(path)?;
    let options = ExportOptions {
        limits: crate::Limits {
            value_depth: 1,
            ..crate::Limits::default()
        },
        ..ExportOptions::default()
    };
    let registry = ProviderRegistry::try_new(vec![Box::new(SkippedProvider)])?;
    let output = folder.path().join("media");
    let crate::ExportOutcome::Published { index, .. } =
        crate::export_with(&archive, &output, &options, &registry)?
    else {
        return Err("raw sources remain supported when the provider writes no records".into());
    };
    assert_eq!(index.limits.value_depth, 1);
    assert_eq!(
        fs::read(output.join(&index.artifact_base).join("model.jsonl"))?,
        Vec::<u8>::new()
    );
    assert_eq!(support::rows(&output, &index, "sources.jsonl")?.len(), 2);
    assert_eq!(support::rows(&output, &index, "artifacts.jsonl")?.len(), 2);
    assert!(
        support::rows(&output, &index, "field-origins.jsonl")?
            .iter()
            .any(|row| row.get("field").and_then(serde_json::Value::as_str)
                == Some("/identity/containers/0/path")),
        "original nested identity nodes keep their required exact field addresses"
    );
    Ok(())
}
