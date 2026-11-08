// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_media::MediaKind;
use p4kx_testkit::{ArchiveBuilder, TestEntry};
use serde_json::json;

use super::shader_material_support::{self as support, TestResult};

const FLAGS: &[u8] =
    b"Property {\nName = %A\nMask = 0x1\n}\nProperty {\nName = %B\nMask = 0x2\n}\n";

#[test]
fn nested_definition_and_each_original_flag_resolve_to_the_emitted_physical_source() -> TestResult {
    let result = support::run(
        vec![
            support::material("  DEMO  ", "%A %B%A"),
            support::container("Data/shaders.pak", "demo", FLAGS),
        ],
        &support::options(),
    )?;
    assert!(result.index.complete);
    let links = support::links(&result)?;
    assert_eq!(links.len(), 4);
    assert_eq!(links[0]["reference"]["original"]["value"], "  DEMO  ");
    for (index, token) in ["%A ", "%B", "%A"].into_iter().enumerate() {
        let link = &links[index + 1];
        assert_eq!(link["token_index"], index);
        assert_eq!(link["reference"]["original"]["value"], token);
        assert_eq!(link["reference"]["resolution"]["status"], "resolved");
        assert_eq!(
            link["reference"]["resolution"]["target"]["subresource"]["namespace"],
            "shader-property"
        );
        assert_eq!(link["input"]["omission"], "reference-lookup");
    }
    let shader = result
        .shaders
        .iter()
        .find(|row| row.subresource.namespace == "shader-definition")
        .ok_or("definition row")?;
    let identity = serde_json::to_value(&shader.source.identity)?;
    let raw = shader.source.raw.as_ref().ok_or("raw shader source")?;
    for link in links {
        assert_eq!(
            link["reference"]["resolution"]["target"]["source"],
            identity
        );
        assert_eq!(link["input"]["digest"], serde_json::to_value(&raw.digest)?);
        assert_eq!(link["input"]["source"]["identity"], identity);
    }
    assert_selected_source_counts(&result)
}

fn assert_selected_source_counts(result: &support::Exported) -> TestResult {
    let source_lines = std::fs::read_to_string(
        result
            .output
            .join(&result.index.artifact_base)
            .join("sources.jsonl"),
    )?;
    assert_eq!(
        source_lines.lines().count() as u64,
        result.index.source_entries.written,
        "each written selected source has one manifest row"
    );
    assert_eq!(
        result.index.source_entries.written, 3,
        "lookup does not add selected callbacks"
    );
    assert_eq!(
        result
            .index
            .kinds
            .iter()
            .find(|kind| kind.provider.kind == MediaKind::Shader)
            .ok_or("shader kind")?
            .counts
            .source_entries
            .written,
        1,
        "private lookups do not publish additional shader sources"
    );
    Ok(())
}

#[test]
fn duplicate_names_across_physical_containers_are_ambiguous_without_first_match() -> TestResult {
    let result = support::run(
        vec![
            support::material("demo", "%A"),
            support::container("Data/first.pak", "demo", FLAGS),
            support::container("Data/second.pak", "DEMO", FLAGS),
        ],
        &support::options(),
    )?;
    let links = support::links(&result)?;
    for link in links {
        assert_eq!(link["reference"]["resolution"]["status"], "ambiguous");
        assert_eq!(link["reference"]["observed_candidates"], 2);
        let candidates = link["reference"]["resolution"]["candidates"]
            .as_array()
            .ok_or("candidates")?;
        assert_ne!(
            candidates[0]["target"]["source"],
            candidates[1]["target"]["source"]
        );
        assert!(
            link["input"].is_null(),
            "ambiguous sources were not read for guessing"
        );
    }
    Ok(())
}

#[test]
fn duplicate_property_names_keep_distinct_source_ordinals() -> TestResult {
    let definition = b"Property { Name = %A\n }\nProperty { Name = %a\n }\n";
    let result = support::run(
        vec![
            support::material("demo", "%A"),
            support::container("Data/flags.pak", "demo", definition),
        ],
        &support::options(),
    )?;
    let links = support::links(&result)?;
    let resolution = &links[1]["reference"]["resolution"];
    assert_eq!(resolution["status"], "ambiguous");
    let targets = resolution["candidates"]
        .as_array()
        .ok_or("duplicate properties")?;
    assert_eq!(targets[0]["target"]["subresource"]["indices"], json!([0]));
    assert_eq!(targets[1]["target"]["subresource"]["indices"], json!([1]));
    Ok(())
}

#[test]
fn duplicate_name_occurrences_in_one_property_are_not_deduplicated() -> TestResult {
    let definition = b"Property { Name = %A\n Name = %A\n }\n";
    let result = support::run(
        vec![
            support::material("demo", "%A"),
            support::container("Data/flags.pak", "demo", definition),
        ],
        &support::options(),
    )?;
    let links = support::links(&result)?;
    assert_eq!(links[1]["reference"]["resolution"]["status"], "ambiguous");
    assert_eq!(links[1]["reference"]["observed_candidates"], 2);
    Ok(())
}

#[test]
fn missing_shader_and_missing_local_flag_remain_different_lookups() -> TestResult {
    let result = support::run(
        vec![
            support::material("absent", "%A"),
            support::container("Data/flags.pak", "demo", FLAGS),
        ],
        &support::options(),
    )?;
    for link in support::links(&result)? {
        assert_eq!(link["reference"]["resolution"]["status"], "missing");
    }
    let result = support::run(
        vec![
            support::material("demo", "%ABSENT"),
            support::container("Data/flags.pak", "demo", FLAGS),
        ],
        &support::options(),
    )?;
    let links = support::links(&result)?;
    assert_eq!(links[0]["reference"]["resolution"]["status"], "resolved");
    assert_eq!(links[1]["reference"]["resolution"]["status"], "missing");
    Ok(())
}

#[test]
fn same_definition_name_case_collision_in_one_directory_is_ambiguous() -> TestResult {
    let nested = ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/demo.ext", FLAGS.to_vec()))
        .entry(TestEntry::stored("Data/DEMO.ext", FLAGS.to_vec()))
        .build();
    let result = support::run(
        vec![
            support::material("Demo", "%A"),
            TestEntry::stored("Data/flags.pak", nested),
        ],
        &support::options(),
    )?;
    assert_eq!(
        support::links(&result)?[0]["reference"]["resolution"]["status"],
        "ambiguous"
    );
    Ok(())
}
