// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::ConflictPolicy;
use p4kx_extraction::model::MaterialSourceLimits;
use p4kx_media::MediaError;
use serde_json::Value;

use super::material_origin_support::Fixture;
use super::shader_material_support::{self as support, TestResult};

#[path = "shader_material_schema.rs"]
mod schema;

const FLAGS: &[u8] = b"Property { Name = %A\n }\n";

#[test]
fn candidate_limit_remains_unavailable_and_never_selects_one_duplicate() -> TestResult {
    let mut options = support::options();
    options.limits.reference_candidates = 1;
    let first = "Data/z-first.pak";
    let fixture = Fixture::with(
        [
            support::material("demo", "%A"),
            support::container(first, "Demo", FLAGS),
            support::container("Data/unrelated.pak", "unrelated", FLAGS),
            support::container("Data/a-overage.pak", "DEMO", FLAGS),
            support::container("Data/m-unseen.pak", "demo", FLAGS),
        ],
        options,
        MaterialSourceLimits::default(),
    )?;
    let material = "Data/materials.mtl";
    let prefix = "/data/payload/shader_references";
    let links = fixture
        .record(material)?
        .1
        .pointer(prefix)
        .and_then(Value::as_array)
        .ok_or("shader links")?;
    assert_eq!(links.len(), 2, "the definition and flag are both checked");
    for (ordinal, link) in links.iter().enumerate() {
        check_cutoff_reference(&link["reference"], first)?;
        let field = format!("{prefix}/{ordinal}/reference/resolution/status");
        check_cutoff_contributors(&fixture, material, &field)?;
    }
    Ok(())
}

fn check_cutoff_reference(reference: &Value, first: &str) -> TestResult {
    let resolution = &reference["resolution"];
    assert_eq!(
        resolution["status"], "unavailable",
        "cutoff remains unavailable"
    );
    assert_eq!(
        resolution.pointer("/reason/type").and_then(Value::as_str),
        Some("candidate-limit"),
        "cutoff reason"
    );
    assert_eq!(
        reference["candidates_complete"], false,
        "coverage past the cutoff is unknown"
    );
    assert_eq!(
        reference["observed_candidates"], 2,
        "only one overage was examined"
    );
    let [retained] = resolution["known_candidates"]
        .as_array()
        .ok_or("retained candidate list")?
        .as_slice()
    else {
        return Err("one retained candidate before the cutoff".into());
    };
    assert_eq!(
        retained
            .pointer("/target/source/containers/0/path")
            .and_then(Value::as_str),
        Some(first),
        "the admitted candidate is the earliest physical occurrence"
    );
    Ok(())
}

fn check_cutoff_contributors(fixture: &Fixture, material: &str, field: &str) -> TestResult {
    for path in [r"Data\Demo.ext", r"Data\DEMO.ext"] {
        assert!(
            fixture.has_use(
                material,
                field,
                path,
                "archiveDirectoryMetadata",
                "selector"
            )?,
            "the retained and first discarded occurrences contribute: {path}"
        );
    }
    assert!(
        !fixture
            .contributions(material, field)?
            .iter()
            .any(|used| fixture.input_path(used) == Some(r"Data\demo.ext")),
        "the third candidate's identity is outside the consulted prefix"
    );
    Ok(())
}

#[test]
fn failed_definition_parse_retains_its_actual_lookup_receipt_and_partial_generation() -> TestResult
{
    let mut options = support::options();
    options.diagnostic = true;
    let result = support::run(
        vec![
            support::material("demo", "%A"),
            support::container(
                "Data/shaders.pak",
                "demo",
                b"Property { Name = \"unterminated",
            ),
        ],
        &options,
    )?;
    assert!(!result.index.complete);
    for link in support::links(&result)? {
        assert_eq!(link["reference"]["resolution"]["status"], "unavailable");
        assert_eq!(
            link["reference"]["resolution"]["reason"]["type"],
            "unsupported-layout"
        );
        assert_eq!(link["input"]["omission"], "reference-lookup");
    }
    Ok(())
}

#[test]
fn generation_work_exhaustion_during_lookup_preserves_the_last_complete_index() -> TestResult {
    let directory = tempfile::tempdir()?;
    let archive = support::archive(
        directory.path(),
        vec![
            support::material("demo", "%A"),
            support::container("Data/shaders.pak", "demo", FLAGS),
        ],
    )?;
    let output = directory.path().join("media");
    let options = support::options();
    p4kx_media::export(&archive, &output, &options)?;
    let before = std::fs::read(output.join("media-index.json"))?;
    let mut limited = options;
    limited.conflict = ConflictPolicy::Overwrite;
    limited.limits.metadata_work = 1;
    let failure = p4kx_media::export(&archive, &output, &limited)
        .err()
        .ok_or("lookup budget unexpectedly succeeded")?;
    assert!(
        metadata_limit(&failure),
        "the shared work limit remains typed: {failure}"
    );
    let MediaError::Source { entry, .. } = &failure else {
        return Err("lookup failure lost the requesting material".into());
    };
    assert!(
        matches!(&entry.identity, p4kx_media::Identity::Entry { entry, .. }
        if entry.path == "Data/materials.mtl"),
        "the exhausted lookup belongs to the requesting material"
    );
    assert_eq!(std::fs::read(output.join("media-index.json"))?, before);
    Ok(())
}

fn metadata_limit(error: &MediaError) -> bool {
    match error {
        MediaError::Limit {
            budget: "metadata_work",
            ..
        } => true,
        MediaError::Source { cause, .. } => metadata_limit(cause),
        _ => false,
    }
}

#[test]
fn repeated_exports_do_not_reuse_another_roots_definition_index() -> TestResult {
    for name in ["first", "second"] {
        let definition = format!("Property {{ Name = %{name}\n }}\n");
        let flags = format!("%{name}");
        let result = support::run(
            vec![
                support::material("demo", &flags),
                support::container("Data/shaders.pak", "demo", definition.as_bytes()),
            ],
            &support::options(),
        )?;
        assert_eq!(
            support::links(&result)?[1]["reference"]["resolution"]["status"],
            "resolved"
        );
    }
    Ok(())
}

#[test]
fn long_retained_identity_paths_consume_the_shared_lookup_work_budget() -> TestResult {
    let long = "x".repeat(2048);
    let mut options = support::options();
    options.limits.metadata_work = 4096;
    let directory = tempfile::tempdir()?;
    let nested = p4kx_testkit::ArchiveBuilder::new()
        .entry(p4kx_testkit::TestEntry::stored(
            format!("Data/{long}/demo.ext"),
            FLAGS,
        ))
        .build();
    let archive = support::archive(
        directory.path(),
        vec![
            support::material("demo", "%A"),
            p4kx_testkit::TestEntry::stored(format!("Data/{long}.pak"), nested),
        ],
    )?;
    let failure = p4kx_media::export(&archive, &directory.path().join("media"), &options)
        .err()
        .ok_or("identity storage escaped lookup work budget")?;
    assert!(metadata_limit(&failure));
    Ok(())
}

#[test]
fn shader_crc_failure_preserves_both_requesting_material_and_actual_target_source() -> TestResult {
    let directory = tempfile::tempdir()?;
    let nested = p4kx_testkit::ArchiveBuilder::new()
        .entry(p4kx_testkit::TestEntry::stored("Data/demo.ext", FLAGS).with_crc32c(0))
        .build();
    let archive = support::archive(
        directory.path(),
        vec![
            support::material("demo", "%A"),
            p4kx_testkit::TestEntry::stored("Data/shaders.pak", nested),
        ],
    )?;
    let failure = p4kx_media::export(
        &archive,
        &directory.path().join("media"),
        &support::options(),
    )
    .err()
    .ok_or("invalid shader CRC was accepted")?;
    let MediaError::Source { entry, cause } = &failure else {
        return Err("requesting source was lost".into());
    };
    let p4kx_media::Identity::Entry { entry, .. } = &entry.identity else {
        return Err("material identity was lost".into());
    };
    assert!(entry.path.ends_with("materials.mtl"));
    assert!(
        has_shader_source(cause),
        "actual shader input source was lost"
    );
    Ok(())
}

fn has_shader_source(error: &MediaError) -> bool {
    match error {
        MediaError::Source { entry, cause } => {
            matches!(&entry.identity, p4kx_media::Identity::Entry { containers, entry }
                if !containers.is_empty() && entry.path.ends_with("demo.ext"))
                || has_shader_source(cause)
        }
        _ => false,
    }
}
