// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::path::Path;

use p4kx_extraction::{Archive, OpenOptions};
use p4kx_media::{ExportOptions, MediaKind, MediaRecord};
use p4kx_testkit::{ArchiveBuilder, TestEntry, test_key};
use serde_json::Value;

use super::support::{self, TestResult};

const STREAM: &str = "animation-definition.jsonl";

#[test]
fn named_caf_links_keep_physical_candidates_and_exact_selector_origins() -> TestResult {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("Data.p4k");
    let params = br##"<Params><AnimationList>
      <Animation name="#FilePath" path="Animations/First"/>
      <Animation name="same" path="Same.caf"/>
      <Animation name="#filepath" path="Animations\Second\"/>
      <Animation name="same" path="Same.caf"/>
      <Animation name="third" path="Data/Animations/Third/Other.CAF"/>
      <Animation name="missing" path="Absent.caf"/>
      <Animation name="ignored*" path="Same.caf"/>
      <Animation name="alsoIgnored" path="*.caf"/>
      <Animation name="invalid" path="../Same.caf"/>
      <Animation name="#filepath" path=""/>
      <Animation name="reset" path="Same.caf"/>
      </AnimationList><AnimationList><Animation name="separate" path="Same.caf"/>
      </AnimationList></Params>"##;
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Objects/fixture.chrparams", params).encrypted())
        .entry(TestEntry::stored(
            "Data/Animations/First/Same.caf",
            b"first",
        ))
        .entry(TestEntry::stored(
            "Data/Animations/Second/Same.caf",
            b"second",
        ))
        .entry(TestEntry::stored(
            "data/animations/second/same.CAF",
            b"duplicate",
        ))
        .entry(TestEntry::stored("Data/Animations/Third/Other.CAF", b"absolute").encrypted())
        .write_to(&path)?;
    let archive = Archive::open_with(path, OpenOptions::new().key(Some(test_key())))?;
    let out = folder.path().join("out");
    let options = ExportOptions {
        kinds: vec![MediaKind::AnimationDefinition],
        ..ExportOptions::default()
    };
    let index = support::published(p4kx_media::export(&archive, &out, &options)?)?;
    let rows = support::records(&out, &index, STREAM)?;
    let validator = jsonschema::validator_for(&p4kx_media::builtin_record_schema()?)?;
    let links: Vec<_> = rows
        .iter()
        .enumerate()
        .filter(|(_, row)| row.subresource.namespace == "animation-definition-named-caf")
        .collect();
    verify_named_caf_rows(&links, &validator)?;
    let [(relative, _), _, (absolute, _), ..] = links.as_slice() else {
        return Err("relative and absolute CAF rows are absent".into());
    };
    verify_named_caf_origins(&out.join(index.artifact_base), *relative, *absolute)
}

fn verify_named_caf_rows(
    links: &[(usize, &MediaRecord)],
    validator: &jsonschema::Validator,
) -> TestResult {
    let cases = named_caf_cases();
    assert_eq!(links.len(), cases.len(), "wildcards have no named CAF link");
    for ((_, row), case) in links.iter().zip(cases) {
        validator
            .validate(&serde_json::to_value(row)?)
            .map_err(|error| error.to_string())?;
        for (path, expected) in [
            ("/payload/name", Some(case.name)),
            ("/payload/filepath_scope", case.scope),
            ("/payload/archive_path", case.archive_path),
            ("/payload/authored_path", Some(case.authored_path)),
            (
                "/payload/reference/original/value",
                Some(case.authored_path),
            ),
            ("/payload/reference/resolution/status", Some(case.status)),
        ] {
            assert_eq!(
                row.data.pointer(path).and_then(Value::as_str),
                expected,
                "named CAF field {path} for {}",
                case.name,
            );
        }
        assert_eq!(
            row.data
                .pointer("/payload/reference/observed_candidates")
                .and_then(Value::as_u64),
            Some(case.candidates),
            "physical CAF candidates for {}",
            case.name,
        );
    }
    Ok(())
}

struct CafCase {
    name: &'static str,
    authored_path: &'static str,
    scope: Option<&'static str>,
    archive_path: Option<&'static str>,
    status: &'static str,
    candidates: u64,
}

const fn named_caf_cases() -> [CafCase; 7] {
    let second = Some("Animations\\Second\\");
    [
        CafCase {
            name: "same",
            authored_path: "Same.caf",
            scope: Some("Animations/First"),
            archive_path: Some("Data/Animations/First/Same.caf"),
            status: "not-selected",
            candidates: 1,
        },
        CafCase {
            name: "same",
            authored_path: "Same.caf",
            scope: second,
            archive_path: Some("Data/Animations/Second/Same.caf"),
            status: "ambiguous",
            candidates: 2,
        },
        CafCase {
            name: "third",
            authored_path: "Data/Animations/Third/Other.CAF",
            scope: second,
            archive_path: Some("Data/Animations/Third/Other.CAF"),
            status: "not-selected",
            candidates: 1,
        },
        CafCase {
            name: "missing",
            authored_path: "Absent.caf",
            scope: second,
            archive_path: Some("Data/Animations/Second/Absent.caf"),
            status: "missing",
            candidates: 0,
        },
        CafCase {
            name: "invalid",
            authored_path: "../Same.caf",
            scope: second,
            archive_path: None,
            status: "unavailable",
            candidates: 0,
        },
        CafCase {
            name: "reset",
            authored_path: "Same.caf",
            scope: Some(""),
            archive_path: None,
            status: "unavailable",
            candidates: 0,
        },
        CafCase {
            name: "separate",
            authored_path: "Same.caf",
            scope: None,
            archive_path: None,
            status: "unavailable",
            candidates: 0,
        },
    ]
}

fn verify_named_caf_origins(base: &Path, relative: usize, absolute: usize) -> TestResult {
    let relative = relative.to_string();
    let absolute = absolute.to_string();
    let file = fs::read_to_string(base.join("field-origins.jsonl"))?;
    let mut matches = 0;
    for row in file.lines().map(serde_json::from_str::<Value>) {
        let row = row?;
        if row.pointer("/scope/stream/kind").and_then(Value::as_str) != Some("animation-definition")
        {
            continue;
        }
        let ordinal = row.pointer("/scope/ordinal").and_then(Value::as_str);
        let field = row.get("field").and_then(Value::as_str);
        let count = match (ordinal, field) {
            (Some(ordinal), Some("/data/payload/archive_path")) if ordinal == relative => 3,
            (Some(ordinal), Some("/data/payload/archive_path")) if ordinal == absolute => 1,
            (
                Some(ordinal),
                Some("/data/payload/reference/resolution/target/source/entry/path"),
            ) if ordinal == absolute => 4,
            (
                Some(ordinal),
                Some(
                    "/data/payload/name"
                    | "/data/payload/authored_path"
                    | "/data/payload/filepath_scope"
                    | "/data/payload/reference/original/value",
                ),
            ) if ordinal == relative => 1,
            _ => continue,
        };
        assert_eq!(
            row.get("containsEncryptedSource").and_then(Value::as_bool),
            Some(true),
            "source at {field:?}"
        );
        assert_eq!(
            row.get("sourceUsesRefs")
                .and_then(Value::as_array)
                .map(Vec::len),
            Some(count),
            "contributors at {field:?}"
        );
        matches += 1;
    }
    assert_eq!(
        matches, 7,
        "original and normalized paths each have source evidence"
    );
    Ok(())
}

#[test]
fn malformed_filepath_declarations_suspend_relative_lookup_with_exact_evidence() -> TestResult {
    let temporary = tempfile::tempdir()?;
    let archive_path = temporary.path().join("Data.p4k");
    let params = br##"<Params><AnimationList>
      <Animation name="#filepath" path="Animations/Before"/>
      <Animation name="before" path="open.caf"/>
      <Animation Path="Animations/DuplicateA" name="#filepath" path="Animations/DuplicateB"/>
      <Animation name="duplicate" path="open.caf"/>
      <Animation name="rooted" path="Animations/Before/open.caf"/>
      <Animation name="#filepath" path="Animations/Between"/>
      <Animation name="#filepath"/>
      <Animation name="absent" path="open.caf"/>
      <Animation name="#filepath" path="Animations/After"/>
      <Animation name="after" path="open.caf"/>
      </AnimationList></Params>"##;
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Objects/fixture.chrparams", params).encrypted())
        .entry(TestEntry::stored(
            "Data/Animations/Before/open.caf",
            b"before",
        ))
        .entry(TestEntry::stored(
            "Data/Animations/After/open.caf",
            b"after",
        ))
        .write_to(&archive_path)?;
    let archive = Archive::open_with(archive_path, OpenOptions::new().key(Some(test_key())))?;
    let output = temporary.path().join("out");
    let options = ExportOptions {
        kinds: vec![MediaKind::AnimationDefinition],
        ..ExportOptions::default()
    };
    let index = support::published(p4kx_media::export(&archive, &output, &options)?)?;
    let rows = support::records(&output, &index, STREAM)?;
    let links: Vec<_> = rows
        .iter()
        .enumerate()
        .filter(|(_, row)| row.subresource.namespace == "animation-definition-named-caf")
        .collect();
    verify_malformed_scope_rows(&links);
    verify_malformed_scope_evidence(&output.join(index.artifact_base), &links)
}

fn verify_malformed_scope_rows(links: &[(usize, &MediaRecord)]) {
    let expected = [
        (
            "before",
            Some("Data/Animations/Before/open.caf"),
            "not-selected",
        ),
        ("duplicate", None, "unavailable"),
        (
            "rooted",
            Some("Data/Animations/Before/open.caf"),
            "not-selected",
        ),
        ("absent", None, "unavailable"),
        (
            "after",
            Some("Data/Animations/After/open.caf"),
            "not-selected",
        ),
    ];
    assert_eq!(
        links.len(),
        expected.len(),
        "only concrete animation paths are linked"
    );
    for ((_, row), (name, path, status)) in links.iter().zip(expected) {
        let string = |field| row.data.pointer(field).and_then(Value::as_str);
        assert_eq!(
            string("/payload/name"),
            Some(name),
            "original selector order"
        );
        assert_eq!(
            string("/payload/archive_path"),
            path,
            "normalized path for {name}"
        );
        assert_eq!(
            string("/payload/reference/resolution/status"),
            Some(status),
            "lookup state for {name}"
        );
        if path.is_none() {
            assert_eq!(
                row.data.pointer("/payload/filepath_scope"),
                Some(&Value::Null),
                "no usable scope for {name}"
            );
        }
    }
}

fn verify_malformed_scope_evidence(base: &Path, links: &[(usize, &MediaRecord)]) -> TestResult {
    let [_, (duplicate, _), (rooted, _), (absent, _), _] = links else {
        return Err("expected bounded fixture rows".into());
    };
    let duplicate = duplicate.to_string();
    let rooted = rooted.to_string();
    let absent = absent.to_string();
    let file = fs::read_to_string(base.join("field-origins.jsonl"))?;
    let mut checked = 0;
    for row in file.lines().map(serde_json::from_str::<Value>) {
        let row = row?;
        if row.pointer("/scope/stream/kind").and_then(Value::as_str) != Some("animation-definition")
            || row.get("field").and_then(Value::as_str) != Some("/data/payload/archive_path")
        {
            continue;
        }
        let expected = match row.pointer("/scope/ordinal").and_then(Value::as_str) {
            Some(value) if value == duplicate => 4,
            Some(value) if value == rooted => 1,
            Some(value) if value == absent => 2,
            _ => continue,
        };
        assert_eq!(
            row.get("sourceUsesRefs")
                .and_then(Value::as_array)
                .map(Vec::len),
            Some(expected),
            "only read relevant current source attributes contribute"
        );
        assert_eq!(
            row.get("containsEncryptedSource").and_then(Value::as_bool),
            Some(true),
            "malformed directive evidence remains encrypted"
        );
        checked += 1;
    }
    assert_eq!(
        checked, 3,
        "both malformed directives and rooted lookup proved"
    );
    Ok(())
}
