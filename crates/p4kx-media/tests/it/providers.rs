// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::io::Read;

use p4kx_media::{
    ByteOrder, Capability, ChunkLayout, DataType, ExportOptions, ExportOutcome, FieldGroup,
    Float32, Float64, MediaError, MediaIssue, MediaKind, MediaWriter, Projection, Provider,
    ProviderRegistry, Severity, SourceContext, Status, Subresource, ValueOrigin,
};
use serde::Serialize;

use super::support::{self, TestResult};

struct TextProvider;

#[derive(Serialize)]
struct Metadata {
    bytes: u64,
    references: Vec<p4kx_media::Reference>,
}

impl Provider for TextProvider {
    fn capability(&self) -> Capability {
        let mut capability = Capability::new(
            MediaKind::Material,
            "synthetic.text",
            "2.3.4",
            "centrally retained decoded source bytes",
        );
        capability
            .field_groups
            .push(FieldGroup::new("byte-count", "streamed byte count"));
        capability
    }

    fn source(
        &self,
        context: &SourceContext<'_>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<Projection, MediaError> {
        let mut input = fs::File::open(context.payload).map_err(|source| MediaError::Io {
            path: context.payload.into(),
            source,
        })?;
        let mut size = 0_u64;
        let mut buffer = [0; 2];
        loop {
            writer.checkpoint()?;
            let count = input.read(&mut buffer).map_err(|source| MediaError::Io {
                path: context.payload.into(),
                source,
            })?;
            if count == 0 {
                break;
            }
            size += count as u64;
        }

        let metadata = Metadata {
            bytes: size,
            references: super::references::cases(context, writer)?,
        };
        writer.record(
            &Subresource::new("text", vec![0]),
            Status::Complete,
            &metadata,
            &[],
        )?;
        Ok(Projection::Written)
    }
}

pub(super) struct SampleProvider {
    pub(super) partial: bool,
}

#[derive(Serialize)]
struct Samples {
    references: Vec<p4kx_media::Reference>,
    original_id: u32,
    missing: ValueOrigin<u32>,
    zero: ValueOrigin<u32>,
    sample: Float32,
    infinity: Float64,
    chunk: p4kx_media::ChunkReference,
}

fn samples(
    context: &SourceContext<'_>,
    writer: &mut MediaWriter<'_>,
) -> Result<Samples, MediaError> {
    let input = 0x7fc0_1234_u32.to_le_bytes();
    let chunk = writer.chunk(
        ChunkLayout::new(
            DataType::F32,
            ByteOrder::Little,
            1,
            "seconds; source sample order; exclusive end",
        ),
        &mut input.as_slice(),
    )?;
    Ok(Samples {
        references: super::references::cases(context, writer)?,
        original_id: 7,
        missing: ValueOrigin::Missing,
        zero: ValueOrigin::Explicit(0),
        sample: Float32::from(f32::from_bits(0x7fc0_1234)),
        infinity: Float64::from(f64::INFINITY),
        chunk,
    })
}

impl Provider for SampleProvider {
    fn capability(&self) -> Capability {
        let mut capability = Capability::new(
            MediaKind::Animation,
            "synthetic.samples",
            "4.5.6",
            "central raw payload plus exact IEEE-754 sample words",
        );
        capability.field_groups.push(FieldGroup::new(
            "samples",
            "synthetic ordered samples in seconds",
        ));
        capability
    }

    fn source(
        &self,
        context: &SourceContext<'_>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<Projection, MediaError> {
        let data = samples(context, writer)?;
        for index in 0..2 {
            let issues = if self.partial {
                vec![MediaIssue::new(
                    "synthetic.unsupported-channel",
                    "channels[2]",
                    Severity::Warning,
                    "populated channel has no decoder",
                )]
            } else {
                Vec::new()
            };
            writer.record(
                &Subresource::new("samples", vec![index]),
                if self.partial {
                    Status::Partial
                } else {
                    Status::Complete
                },
                &data,
                &issues,
            )?;
        }
        Ok(Projection::Written)
    }
}

pub(super) fn registry() -> Result<ProviderRegistry, MediaError> {
    ProviderRegistry::try_new(vec![
        Box::new(TextProvider),
        Box::new(SampleProvider { partial: false }),
    ])
}

#[test]
fn two_independent_providers_share_writer_identity_counts_chunks_and_float_contracts() -> TestResult
{
    let folder = tempfile::tempdir()?;
    let archive = support::archive(folder.path())?;
    let output = folder.path().join("media");
    let providers = ProviderRegistry::try_new(vec![
        Box::new(TextProvider),
        Box::new(SampleProvider { partial: false }),
    ])?;
    let index = support::published(p4kx_media::export_with(
        &archive,
        &output,
        &ExportOptions::default(),
        &providers,
    )?)?;
    assert_eq!(index.kinds.len(), 2);
    let material = index.kinds.first().ok_or("material summary")?;
    let animation = index.kinds.get(1).ok_or("animation summary")?;
    assert_eq!(
        (
            material.counts.records.written,
            animation.counts.records.written
        ),
        (3, 6)
    );
    assert_eq!(animation.counts.converted_outputs.enumerated, 3);
    assert_eq!(animation.counts.converted_outputs.written, 1);
    assert_eq!(
        animation.counts.converted_outputs.skipped, 2,
        "identical converted chunks share one actual artifact"
    );
    assert_samples(&output, &index)?;
    Ok(())
}

fn assert_samples(output: &std::path::Path, index: &p4kx_media::Index) -> TestResult {
    let rows = support::records(output, index, "animation.jsonl")?;
    let first = rows.first().ok_or("sample record")?;
    assert_eq!(
        first.data.pointer("/sample/value"),
        Some(&serde_json::Value::Null),
        "non-finite samples use null in the numeric JSON field"
    );
    assert_eq!(
        first.data.pointer("/sample/raw_bits"),
        Some(&serde_json::json!("7fc01234")),
        "the original NaN payload bits remain available"
    );
    assert_ne!(
        first.data.get("missing"),
        first.data.get("zero"),
        "missing differs from explicit zero"
    );
    let second = rows.get(1).ok_or("second subresource")?;
    assert_eq!(
        first.source.identity, second.source.identity,
        "subresources retain their common physical source"
    );
    assert_ne!(
        first.subresource, second.subresource,
        "duplicate short IDs preserve original ordinals"
    );
    let chunk = first
        .data
        .pointer("/chunk/artifact/path")
        .and_then(serde_json::Value::as_str)
        .ok_or("chunk path")?;
    assert_eq!(
        fs::read(output.join(&index.artifact_base).join(chunk))?,
        0x7fc0_1234_u32.to_le_bytes(),
        "the binary chunk preserves exact source float bits"
    );
    Ok(())
}

#[test]
fn partial_records_get_a_diagnostic_generation_without_replacing_complete_index() -> TestResult {
    let folder = tempfile::tempdir()?;
    let archive = support::archive(folder.path())?;
    let output = folder.path().join("media");
    p4kx_media::export(&archive, &output, &ExportOptions::default())?;
    let old = fs::read(output.join("media-index.json"))?;
    let providers = ProviderRegistry::try_new(vec![Box::new(SampleProvider { partial: true })])?;
    let options = ExportOptions {
        conflict: p4kx_extraction::ConflictPolicy::Overwrite,
        diagnostic: true,
        ..ExportOptions::default()
    };
    let result = p4kx_media::export_with(&archive, &output, &options, &providers)?;
    let ExportOutcome::Incomplete { index, diagnostic } = result else {
        return Err("expected diagnostic generation".into());
    };
    assert!(
        !index.complete,
        "projection loss cannot publish a successful root index"
    );
    assert!(
        diagnostic.is_file(),
        "diagnostic index is durable and independently inspectable"
    );
    assert_eq!(fs::read(output.join("media-index.json"))?, old);
    Ok(())
}
