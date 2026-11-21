// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;
use std::fs;

use serde_json::Value;

use super::build_origin_fixture::{self as fixture, MANIFEST, Run, TestResult};

pub(super) fn file_field(
    run: &Run,
    stream: &str,
    pointer: &str,
    logical: Option<&str>,
) -> TestResult {
    let field = run.field(stream, pointer)?;
    assert_eq!(
        field.get("containsEncryptedSource"),
        Some(&Value::Null),
        "standalone manifest lineage remains explicitly unknown"
    );
    assert_eq!(
        field.get("sourceSetComplete"),
        Some(&Value::Bool(true)),
        "all actual admitted manifest contributors are retained"
    );
    let uses = run.contributions(field)?;
    assert!(
        !uses.is_empty(),
        "a source-backed Build datum needs actual original file uses"
    );
    for used in &uses {
        let source = run.physical(used)?;
        assert_eq!(
            source.get("kind").and_then(Value::as_str),
            Some("file"),
            "Build contributors retain the actual standalone File identity"
        );
        assert_eq!(
            source.pointer("/name/value").and_then(Value::as_str),
            Some(MANIFEST),
            "the original input is the adjacent manifest, independent of same-name Entries"
        );
        assert_eq!(
            used.get("effectiveEncryption").and_then(Value::as_str),
            Some("unknown"),
            "a standalone file does not acquire an inferred P4K encryption state"
        );
    }
    if let Some(logical) = logical {
        logical_uses(&uses, logical);
    }
    Ok(())
}

pub(super) fn independent(run: &Run, stream: &str, pointer: &str) -> TestResult {
    let field = run.field(stream, pointer)?;
    assert_eq!(
        field.get("containsEncryptedSource"),
        Some(&Value::Bool(false)),
        "an explicit tool default has no encrypted physical input"
    );
    assert_eq!(
        field.get("sourceSetComplete"),
        Some(&Value::Bool(true)),
        "an explicit tool default has a complete empty contributor set"
    );
    let expected: &[Value] = &[];
    assert_eq!(
        fixture::references(field)?.as_slice(),
        expected,
        "the independent default cites no physical input"
    );
    Ok(())
}

pub(super) fn unknown(run: &Run, stream: &str, pointer: &str) -> TestResult {
    let field = run.field(stream, pointer)?;
    assert_eq!(
        field.get("containsEncryptedSource"),
        Some(&Value::Null),
        "missing original proof remains explicitly unknown"
    );
    assert_eq!(
        field.get("sourceSetComplete"),
        Some(&Value::Bool(false)),
        "missing original proof never certifies an empty source set"
    );
    assert!(
        fixture::references(field)?.is_empty(),
        "failed/absent file reads and the P4K root do not become Build payload contributors"
    );
    Ok(())
}

pub(super) fn payloads(run: &Run) -> TestResult<BTreeMap<String, Vec<u8>>> {
    let mut paths = BTreeMap::new();
    for artifact in run
        .index
        .kinds
        .iter()
        .map(|kind| &kind.artifact)
        .chain([&run.index.source_manifest, &run.index.artifact_manifest])
    {
        paths.insert(
            artifact.path.clone(),
            fs::read(run.base.join(&artifact.path))?,
        );
    }
    for row in super::build_origin_fixture::rows(run.base.join(&run.index.artifact_manifest.path))?
    {
        let path = row
            .get("path")
            .and_then(Value::as_str)
            .ok_or("actual artifact path")?;
        paths.insert(path.to_owned(), fs::read(run.base.join(path))?);
    }
    Ok(paths)
}
fn logical_uses(uses: &[&Value], logical: &str) {
    assert!(
        uses.iter().any(
            |used| used.get("role").and_then(Value::as_str) == Some("payload")
                && used.pointer("/location/field").and_then(Value::as_str) == Some(logical)
        ),
        "the retained datum keeps its own actual JSON field"
    );
    assert!(
        uses.iter()
            .filter(|used| used.get("role").and_then(Value::as_str) == Some("payload"))
            .all(|used| used.pointer("/location/field").and_then(Value::as_str) == Some(logical)),
        "an immediate child must not inherit another Build field's Payload"
    );
    assert!(
        uses.iter()
            .any(|used| used.get("role").and_then(Value::as_str) == Some("selector")),
        "successful typed Build selection retains its actual parse/shape predicate"
    );
}
