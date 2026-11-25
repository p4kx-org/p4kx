// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;
use std::path::Path;

use p4kx_extraction::{ConflictPolicy, Exclusion};
use p4kx_media::{AudioOutput, ExportOptions, ExportOutcome, Index, MediaKind};
use serde_json::Value;

use super::support;

#[path = "mixed_media_fixture.rs"]
mod fixture;
#[path = "mixed_media_verifier.rs"]
mod verify;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

#[test]
fn production_providers_repeat_one_complete_generation_with_identical_manifests_and_artifacts()
-> TestResult {
    let scratch = tempfile::tempdir()?;
    let source = scratch.path().join("Data.p4k");
    fixture::create(&source)?;
    let first = scratch.path().join("first");
    let second = scratch.path().join("second");

    let options = ExportOptions {
        exclude: Exclusion::parse([fixture::FILTERED])?,
        audio_output: AudioOutput::Ogg,
        texture_mip: Some(0),
        ..ExportOptions::default()
    };
    let first_index = export(&source, &first, &options)?;
    let second_index = export(
        &source,
        &second,
        &ExportOptions {
            kinds: first_index
                .kinds
                .iter()
                .rev()
                .map(|kind| kind.provider.kind)
                .collect(),
            ..options.clone()
        },
    )?;
    let first_index_bytes = check_indexes(&first, &second, &first_index, &second_index)?;
    verify::generation(
        &first.join(&first_index.artifact_base),
        &second.join(&second_index.artifact_base),
        &first_index,
    )?;

    let outcome = p4kx_media::export(
        &fixture::open(&source)?,
        &first,
        &ExportOptions {
            conflict: ConflictPolicy::IfChanged,
            ..options
        },
    )?;
    assert!(
        matches!(outcome, ExportOutcome::Published { changed: false, .. }),
        "IfChanged validates and retains the complete mixed generation"
    );
    assert_eq!(
        fs::read(first.join("media-index.json"))?,
        first_index_bytes,
        "committed index retained"
    );
    Ok(())
}

fn check_indexes(
    first: &Path,
    second: &Path,
    first_index: &Index,
    second_index: &Index,
) -> TestResult<Vec<u8>> {
    assert_eq!(
        first_index.generation, second_index.generation,
        "same generation digest"
    );
    assert_eq!(
        first_index.artifact_base, second_index.artifact_base,
        "same content-addressed placement"
    );
    let first_index_bytes = fs::read(first.join("media-index.json"))?;
    assert_eq!(
        first_index_bytes,
        fs::read(second.join("media-index.json"))?,
        "fresh implicit and explicit production selections emit the same index bytes"
    );
    let schema = jsonschema::validator_for(&p4kx_media::index_schema())?;
    let json: Value = serde_json::from_slice(&first_index_bytes)?;
    schema.validate(&json).map_err(|error| error.to_string())?;
    Ok(first_index_bytes)
}

fn export(source: &Path, output: &Path, options: &ExportOptions) -> TestResult<Index> {
    let index = support::published(p4kx_media::export(
        &fixture::open(source)?,
        output,
        options,
    )?)?;
    assert!(
        index.complete && index.enumeration_complete,
        "the combined generation is complete"
    );
    assert_eq!(
        index.kinds.len(),
        9,
        "all built-in providers are part of the generation"
    );
    assert!(
        index
            .kinds
            .iter()
            .any(|kind| kind.provider.kind == MediaKind::Audio),
        "normal audio provider selected"
    );
    Ok(index)
}
