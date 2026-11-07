// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;

use p4kx_media::{
    Capability, ExportOptions, MediaError, MediaKind, MediaWriter, Projection, Provider,
    ProviderRegistry, SourceContext, Status, Subresource,
};
use serde::Serialize;

use super::support::{self, TestResult};

#[derive(Debug, Serialize)]
enum UnwrappedValue {
    Float32(f32),
    Float64(f64),
    Nested(Box<Self>),
}

struct UnwrappedFloat(UnwrappedValue);

impl Provider for UnwrappedFloat {
    fn capability(&self) -> Capability {
        Capability::new(
            MediaKind::Audio,
            "synthetic.unwrapped",
            "1.0.0",
            "central raw payload",
        )
    }

    fn source(
        &self,
        _source: &SourceContext<'_>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<Projection, MediaError> {
        #[derive(Serialize)]
        enum Envelope<'a> {
            Samples {
                tracks: Option<Vec<(u32, &'a UnwrappedValue)>>,
            },
        }
        writer.record(
            &Subresource::new("unwrapped", vec![0]),
            Status::Complete,
            &Envelope::Samples {
                tracks: Some(vec![(7, &self.0)]),
            },
            &[],
        )?;
        Ok(Projection::Written)
    }
}

#[test]
fn unwrapped_non_finite_values_are_rejected_in_actual_nested_provider_writes() -> TestResult {
    let folder = tempfile::tempdir()?;
    let archive = support::archive(folder.path())?;
    let output = folder.path().join("media");
    p4kx_media::export(&archive, &output, &ExportOptions::default())?;
    let previous = fs::read(output.join("media-index.json"))?;
    let options = ExportOptions {
        conflict: p4kx_extraction::ConflictPolicy::Overwrite,
        ..ExportOptions::default()
    };
    for value in [
        UnwrappedValue::Float32(f32::NAN),
        UnwrappedValue::Float32(f32::INFINITY),
        UnwrappedValue::Float32(f32::NEG_INFINITY),
        UnwrappedValue::Float64(f64::NAN),
        UnwrappedValue::Float64(f64::INFINITY),
        UnwrappedValue::Float64(f64::NEG_INFINITY),
    ] {
        let registry = ProviderRegistry::try_new(vec![Box::new(UnwrappedFloat(value))])?;
        assert!(
            p4kx_media::export_with(&archive, &output, &options, &registry).is_err(),
            "bare non-finite floats must not silently become JSON null"
        );
        assert_eq!(
            fs::read(output.join("media-index.json"))?,
            previous,
            "failed provider cannot obscure complete generation"
        );
    }
    Ok(())
}

#[test]
fn nested_provider_values_obey_the_central_depth_budget() -> TestResult {
    let folder = tempfile::tempdir()?;
    let archive = support::archive(folder.path())?;
    let output = folder.path().join("media");
    p4kx_media::export(&archive, &output, &ExportOptions::default())?;
    let previous = fs::read(output.join("media-index.json"))?;
    let options = ExportOptions {
        conflict: p4kx_extraction::ConflictPolicy::Overwrite,
        ..ExportOptions::default()
    };
    let value = (0..options.limits.value_depth).fold(UnwrappedValue::Float64(0.0), |inner, _| {
        UnwrappedValue::Nested(Box::new(inner))
    });
    let registry = ProviderRegistry::try_new(vec![Box::new(UnwrappedFloat(value))])?;
    assert!(p4kx_media::export_with(&archive, &output, &options, &registry).is_err());
    assert_eq!(fs::read(output.join("media-index.json"))?, previous);
    Ok(())
}
