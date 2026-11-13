// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::path::Path;

use p4kx_extraction::{Archive, OpenOptions};
use p4kx_media::{ExportOptions, MediaKind, MediaRecord};
use p4kx_testkit::{ArchiveBuilder, TestEntry, test_key};
use serde_json::Value;

use super::support::{self, TestResult};

const MIXED: &[u8] = br#"<ParaGroup version="2">
<Blendable><Face p0="1" p1="2" p2="8" p3="run" p4="mix" p5="name-only"
                       p6="3" p7=" 0" p8="Case" p9="duplicated" p10=""/></Blendable>
<ExamplePseudo><Pseudo Alias="8" p0="3" p1="run" p2="2" p3="Mix"/></ExamplePseudo>
<ExampleList>
<Example AName="name-only" Alias="1"/><Example AName="direct.caf" Alias="run"/>
<Example AName="any" Alias="2"/><Example Alias="Mix"/><Example Alias="CASE"/>
<Example Alias="duplicated"/><Example Alias="duplicated"/>
</ExampleList>
</ParaGroup>"#;
const FRONTIER: &[u8] = br#"<ParaGroup version="3">
<ExampleList><Example Alias="one"/></ExampleList>
<ExampleList><Unknown/><Example Alias="0"/></ExampleList>
<Blendable><Face p0="0" p1="one" p2="ONE"/></Blendable>
</ParaGroup>"#;

#[test]
fn local_blend_references_separate_ordinals_exact_aliases_variants_and_conflicts() -> TestResult {
    let scratch = tempfile::tempdir()?;
    let path = scratch.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Animations/mixed.bspace", MIXED).encrypted())
        .entry(TestEntry::stored(
            "Data/Animations/direct.caf",
            b"unselected",
        ))
        .write_to(&path)?;
    let archive = Archive::open_with(path, OpenOptions::new().key(Some(test_key())))?;
    let output = scratch.path().join("out");
    let (rows, base) = project(&archive, &output)?;
    verify_face_points(&rows)?;
    verify_pseudo_points(&rows)?;
    verify_fields(&rows, &base)
}

fn verify_face_points(rows: &[MediaRecord]) -> TestResult {
    let selection = [
        ("p0", "conflict", "candidate", "exact"),
        ("p1", "agreement", "candidate", "exact"),
        ("p2", "exact-alias-only", "out-of-range", "exact"),
        ("p3", "exact-alias-only", "non-integer", "exact"),
        (
            "p4",
            "tentative-case-variant",
            "non-integer",
            "case-variant",
        ),
        ("p5", "unresolved", "non-integer", "missing"),
        ("p6", "ordinal-only", "candidate", "missing"),
        ("p7", "ordinal-only", "candidate", "missing"),
        (
            "p8",
            "tentative-case-variant",
            "non-integer",
            "case-variant",
        ),
        ("p9", "conflict", "non-integer", "repeated-exact"),
        ("p10", "unresolved", "non-integer", "missing"),
    ];
    let rows_by_point = reference_rows(rows, "face");
    assert_eq!(
        rows_by_point.len(),
        selection.len(),
        "every original face point, including unknown"
    );
    for (row, (slot, interpretation, ordinal, alias)) in rows_by_point.iter().zip(selection) {
        assert_eq!(
            datum(row, "attribute"),
            slot,
            "original face declaration order"
        );
        assert_eq!(
            datum(row, "interpretations"),
            interpretation,
            "interpretations for {slot}"
        );
        assert_eq!(
            datum(row, "ordinal"),
            ordinal,
            "documented ordinal phase for {slot}"
        );
        assert_eq!(
            datum(row, "alias"),
            alias,
            "exact or tentative alias for {slot}"
        );
    }
    verify_numeric_candidates(rows, &rows_by_point)?;
    assert_eq!(
        datum(point(&rows_by_point, "p7")?, "value"),
        " 0",
        "original whitespace"
    );
    Ok(())
}

fn verify_numeric_candidates(rows: &[MediaRecord], points: &[&MediaRecord]) -> TestResult {
    let conflict = candidates(rows, point(points, "p0")?);
    let [ordinal, alias] = conflict.as_slice() else {
        return Err("conflicting numeric alias must have two explicit candidates".into());
    };
    assert_eq!(
        datum(ordinal, "basis"),
        "local-ordinal",
        "ordinal evidence comes first"
    );
    assert_eq!(
        datum(alias, "basis"),
        "exact-alias",
        "exact evidence remains distinct"
    );
    assert_ne!(
        datum(ordinal, "target_element_index"),
        datum(alias, "target_element_index"),
        "no guessed choice between different nodes"
    );
    let agreement = candidates(rows, point(points, "p1")?);
    let [first, second] = agreement.as_slice() else {
        return Err("both observations when interpretations agree".into());
    };
    assert_eq!(
        datum(first, "target_element_index"),
        datum(second, "target_element_index"),
        "same physical source node"
    );
    let pseudo = candidates(rows, point(points, "p2")?);
    let [pseudo] = pseudo.as_slice() else {
        return Err("out-of-range integer has only the observed alias".into());
    };
    assert_eq!(
        datum(pseudo, "target_kind"),
        "pseudo",
        "face may reference a pseudo example"
    );
    Ok(())
}

fn verify_pseudo_points(rows: &[MediaRecord]) -> TestResult {
    let references = reference_rows(rows, "pseudo");
    assert_eq!(references.len(), 4, "four ordered pseudo inputs");
    for candidate in references.iter().flat_map(|row| candidates(rows, row)) {
        assert_eq!(
            datum(candidate, "target_kind"),
            "real",
            "pseudo inputs only use real examples"
        );
    }
    assert_eq!(
        datum(point(&references, "p2")?, "interpretations"),
        "agreement",
        "numeric and exact alias can agree"
    );
    assert_eq!(
        typed(rows, "link").count(),
        0,
        "local declarations are independent of loose CAF files"
    );
    Ok(())
}

#[test]
fn local_blend_references_report_an_unproven_ordinal_layout_and_keep_aliases() -> TestResult {
    let scratch = tempfile::tempdir()?;
    let archive = support::archive_with(
        scratch.path(),
        [TestEntry::stored(
            "Data/Animations/frontier.bspace",
            FRONTIER,
        )],
    )?;
    let (rows, _) = project(&archive, &scratch.path().join("out"))?;
    let references = reference_rows(&rows, "face");
    let zero = point(&references, "p0")?;
    assert_eq!(
        datum(zero, "ordinal"),
        "unavailable-layout",
        "multiple or unsupported tables cannot prove numeric order"
    );
    assert_eq!(
        datum(zero, "interpretations"),
        "exact-alias-only",
        "original alias remains observable"
    );
    let exact = candidates(&rows, zero);
    let [alias] = exact.as_slice() else {
        return Err("one authored candidate".into());
    };
    assert_eq!(
        datum(alias, "basis"),
        "exact-alias",
        "no hypothetical numeric candidate"
    );
    let variant = candidates(&rows, point(&references, "p2")?);
    let [variant] = variant.as_slice() else {
        return Err("one case variant".into());
    };
    assert_eq!(
        datum(variant, "basis"),
        "case-variant-alias",
        "case variants are independently labelled"
    );
    Ok(())
}

fn verify_fields(rows: &[MediaRecord], base: &Path) -> TestResult {
    let ordinal = rows
        .iter()
        .position(|row| datum(row, "basis") == "exact-alias")
        .ok_or("exact candidate occurrence")?
        .to_string();
    let receipts = fs::read_to_string(base.join("field-origins.jsonl"))?;
    let mut found = false;
    for line in receipts.lines() {
        let value: Value = serde_json::from_str(line)?;
        if value.pointer("/scope/ordinal").and_then(Value::as_str) != Some(&ordinal)
            || value.pointer("/scope/stream/kind").and_then(Value::as_str)
                != Some("animation-definition")
            || value.get("field").and_then(Value::as_str)
                != Some("/data/payload/target_element_index")
        {
            continue;
        }
        assert_eq!(
            value
                .get("containsEncryptedSource")
                .and_then(Value::as_bool),
            Some(true),
            "encrypted exact use"
        );
        assert_eq!(
            value.get("sourceSetComplete").and_then(Value::as_bool),
            Some(true),
            "complete attribute+target proof"
        );
        assert!(
            value
                .get("sourceUsesRefs")
                .and_then(Value::as_array)
                .is_some_and(|refs| refs.len() >= 3),
            "selector, target and alias retain distinct original locations"
        );
        found = true;
    }
    assert!(found, "candidate has its own immediate receipt");
    Ok(())
}

fn project(
    archive: &Archive,
    output: &Path,
) -> Result<(Vec<MediaRecord>, std::path::PathBuf), Box<dyn std::error::Error>> {
    let options = ExportOptions {
        kinds: vec![MediaKind::AnimationDefinition],
        ..ExportOptions::default()
    };
    let index = support::published(p4kx_media::export(archive, output, &options)?)?;
    let rows = support::records(output, &index, "animation-definition.jsonl")?;
    let validator = jsonschema::validator_for(&p4kx_media::builtin_record_schema()?)?;
    for row in &rows {
        validator
            .validate(&serde_json::to_value(row)?)
            .map_err(|error| error.to_string())?;
    }
    Ok((rows, output.join(index.artifact_base)))
}

fn datum<'a>(row: &'a MediaRecord, key: &str) -> &'a str {
    row.data
        .get("payload")
        .and_then(|row| row.get(key))
        .and_then(Value::as_str)
        .unwrap_or_default()
}

fn typed<'a>(rows: &'a [MediaRecord], kind: &'a str) -> impl Iterator<Item = &'a MediaRecord> {
    rows.iter()
        .filter(move |row| row.data.get("record_type").and_then(Value::as_str) == Some(kind))
}

fn reference_rows<'a>(rows: &'a [MediaRecord], kind: &str) -> Vec<&'a MediaRecord> {
    typed(rows, "blend-space-reference")
        .filter(|row| datum(row, "kind") == kind)
        .collect()
}

fn point<'a>(rows: &[&'a MediaRecord], name: &str) -> Result<&'a MediaRecord, &'static str> {
    rows.iter()
        .copied()
        .find(|row| datum(row, "attribute") == name)
        .ok_or("original point")
}

fn candidates<'a>(rows: &'a [MediaRecord], reference: &MediaRecord) -> Vec<&'a MediaRecord> {
    typed(rows, "blend-space-candidate")
        .filter(|row| {
            row.subresource
                .indices
                .starts_with(&reference.subresource.indices)
        })
        .collect()
}
