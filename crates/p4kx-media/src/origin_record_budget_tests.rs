// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;
use std::fs;

use p4kx_extraction::{Archive, ConflictPolicy};
use p4kx_testkit::{ArchiveBuilder, TestEntry};

use crate::origin_test_support::{self as support, TestResult};
use crate::{
    Capability, ExportOptions, MediaError, MediaKind, MediaWriter, Projection, Provider,
    ProviderRegistry, SourceContext, Status, Subresource,
};

struct WideProvider;

impl Provider for WideProvider {
    fn capability(&self) -> Capability {
        Capability::new(
            MediaKind::Model,
            "synthetic.wide-address",
            "1.0.0",
            "retained source",
        )
    }

    fn source(
        &self,
        _source: &SourceContext<'_>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<Projection, MediaError> {
        let data = BTreeMap::from([("/~".repeat(4_096), 1_u8)]);
        writer.record(
            &Subresource::new("wide-field", Vec::new()),
            Status::Complete,
            &data,
            &[],
        )?;
        Ok(Projection::Written)
    }
}

#[test]
fn one_original_record_byte_boundary_does_not_limit_the_larger_required_origin_line() -> TestResult
{
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("plain.bin", b"data"))
        .write_to(&path)?;
    let archive = Archive::open(path)?;
    let registry = ProviderRegistry::try_new(vec![Box::new(WideProvider)])?;
    let first_output = folder.path().join("first");
    let first = published(crate::export_with(
        &archive,
        &first_output,
        &ExportOptions::default(),
        &registry,
    )?)?;
    let original = fs::read(first_output.join(&first.artifact_base).join("model.jsonl"))?;
    let record_bytes = u64::try_from(original.len())?;
    let options = ExportOptions {
        limits: crate::Limits {
            record_bytes,
            ..crate::Limits::default()
        },
        conflict: ConflictPolicy::Overwrite,
        ..ExportOptions::default()
    };
    let output = folder.path().join("exact");
    let index = published(crate::export_with(&archive, &output, &options, &registry)?)?;
    assert_eq!(
        fs::read(output.join(&index.artifact_base).join("model.jsonl"))?,
        original
    );
    let fields = fs::read_to_string(
        output
            .join(&index.artifact_base)
            .join("field-origins.jsonl"),
    )?;
    assert!(
        fields
            .lines()
            .any(|line| u64::try_from(line.len() + 1).is_ok_and(|size| size > record_bytes)),
        "a required escaped pointer can exceed the original whole-record byte limit"
    );
    let below = ExportOptions {
        limits: crate::Limits {
            record_bytes: record_bytes - 1,
            ..crate::Limits::default()
        },
        conflict: ConflictPolicy::Overwrite,
        ..ExportOptions::default()
    };
    let destination = folder.path().join("below");
    let error = crate::export_with(&archive, &destination, &below, &registry)
        .err()
        .ok_or("one byte below original record admission must fail")?;
    assert!(support::has_limit(&error, "record_bytes"));
    assert!(!destination.join("media-index.json").exists());
    Ok(())
}

fn published(outcome: crate::ExportOutcome) -> Result<crate::Index, Box<dyn std::error::Error>> {
    if let crate::ExportOutcome::Published { index, .. } = outcome {
        Ok(*index)
    } else {
        Err("complete synthetic wide-address generation".into())
    }
}
