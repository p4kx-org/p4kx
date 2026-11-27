// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;

use p4kx_media::{
    Capability, ExportOptions, Identity, MediaError, MediaKind, MediaWriter, Projection, Provider,
    ProviderRegistry, Source, SourceContext,
};

use super::support::{self, TestResult};

struct SkippingProvider;

impl Provider for SkippingProvider {
    fn capability(&self) -> Capability {
        Capability::new(
            MediaKind::Material,
            "synthetic.skipping",
            "1.0.0",
            "central selected-source inventory preserves decoded bytes independently of typed projection",
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
fn selected_unknown_payloads_keep_identity_and_usable_bytes_when_every_provider_skips() -> TestResult
{
    let folder = tempfile::tempdir()?;
    let archive = support::archive(folder.path())?;
    let output = folder.path().join("media");
    let providers = ProviderRegistry::try_new(vec![Box::new(SkippingProvider)])?;
    let index = support::published(p4kx_media::export_with(
        &archive,
        &output,
        &ExportOptions::default(),
        &providers,
    )?)?;
    let kind = index.kinds.first().ok_or("selected kind")?;
    assert_eq!(
        (
            kind.counts.source_entries.enumerated,
            kind.counts.source_entries.skipped,
            kind.counts.records.written
        ),
        (3, 3, 0)
    );
    assert_eq!(
        (
            index.source_entries.enumerated,
            index.source_entries.written
        ),
        (3, 3)
    );
    let lines = fs::read_to_string(
        output
            .join(&index.artifact_base)
            .join(&index.source_manifest.path),
    )?;
    let sources: Vec<Source> = lines
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()?;
    let validator = jsonschema::validator_for(&p4kx_media::source_schema())?;
    let mut bytes = Vec::new();
    for (physical_index, source) in sources.iter().enumerate() {
        validator
            .validate(&serde_json::to_value(source)?)
            .map_err(|error| error.to_string())?;
        assert!(
            matches!(&source.identity, Identity::Entry { containers, entry } if containers.is_empty() && entry.entry_index == physical_index)
        );
        let raw = source.raw.as_ref().ok_or("usable retained source")?;
        bytes.push(fs::read(output.join(&index.artifact_base).join(&raw.path))?);
    }
    assert_eq!(bytes, [b"first".to_vec(), b"second".to_vec(), Vec::new()]);
    Ok(())
}
