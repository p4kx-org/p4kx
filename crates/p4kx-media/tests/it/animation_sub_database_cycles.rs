// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::path::Path;

use p4kx_extraction::{Archive, Exclusion, OpenOptions};
use p4kx_media::{ExportOptions, MediaKind, MediaRecord};
use p4kx_testkit::{ArchiveBuilder, TestContainer, TestEntry, test_key};
use serde_json::Value;

use super::support::{self, TestResult};

const STREAM: &str = "animation-definition.jsonl";

#[test]
fn sub_database_cycles_keep_parallel_edges_and_separate_an_acyclic_diamond() -> TestResult {
    let scratch = tempfile::tempdir()?;
    let path = scratch.path().join("Data.p4k");
    let expected_edges = [
        ("0", "0"),
        ("0", "1"),
        ("0", "1"),
        ("1", "0"),
        ("2", "3"),
        ("2", "4"),
        ("3", "5"),
        ("4", "5"),
    ];
    ArchiveBuilder::new()
        .entry(adb(
            "a",
            &[
                r"ANIMATIONS\A.ADB",
                "Data/Animations/b.adb",
                "Animations/B.adb",
            ],
        ))
        .entry(adb("b", &["Animations/a.adb"]).encrypted())
        .entry(adb(
            "head",
            &["Animations/left.adb", "Animations/right.adb"],
        ))
        .entry(adb("left", &["Animations/end.adb"]))
        .entry(adb("right", &["Animations/end.adb"]))
        .entry(adb("end", &[]))
        .write_to(&path)?;
    let archive = Archive::open_with(path, OpenOptions::new().key(Some(test_key())))?;
    let output = scratch.path().join("out");
    let (rows, base) = project(&archive, &output, &options())?;
    assert_eq!(
        edges(&rows),
        expected_edges,
        "physical declarations and original ordering"
    );
    assert_eq!(
        cycles(&rows),
        [("0", "2", "4")],
        "including the self edge and repeated declarations"
    );
    verify_encryption(
        &rows,
        &base,
        "sub-database-cycle",
        "/data/payload/edge_count",
        true,
    )?;
    verify_encryption(
        &rows,
        &base,
        "sub-database-edge",
        "/data/payload/source_element_index",
        false,
    )
}

#[test]
fn sub_database_cycles_preserve_frontier_and_do_not_open_excluded_encrypted_targets() -> TestResult
{
    let scratch = tempfile::tempdir()?;
    let bytes = br#"<AnimDB><SubADBs>
        <SubADB File="Animations/self.adb"/>
        <SubADB File="Animations/secret.adb"/>
        <SubADB File="Animations/missing.adb"/>
        <SubADB File="Animations/ambiguous.adb"/>
        <SubADB File="Animations/bad.adb"/>
        <SubADB File="Animations/self.adb" file="Animations/self.adb"/>
        <SubADB/>
        </SubADBs><SubADB File="Animations/self.adb"/>
    </AnimDB>"#;
    let archive = support::archive_with(
        scratch.path(),
        [
            TestEntry::stored("Data/Animations/self.adb", bytes),
            adb("secret", &["Animations/self.adb"]).encrypted(),
            adb("ambiguous", &["Animations/self.adb"]),
            adb("AMBIGUOUS", &["Animations/self.adb"]),
            TestEntry::stored("Data/Animations/bad.adb", b"<OtherRoot/>"),
        ],
    )?;
    let mut options = options();
    options.exclude = Exclusion::parse(["*secret.adb"])?;
    let output = scratch.path().join("out");
    let (rows, _) = project(&archive, &output, &options)?;
    assert_eq!(
        cycles(&rows),
        [("0", "1", "1")],
        "only the one canonical self edge proves a cycle"
    );
    assert_eq!(
        edges(&rows),
        [("0", "0"), ("2", "0"), ("3", "0")],
        "ambiguous target and noncanonical edges remain outside the observed adjacency"
    );
    assert_eq!(
        typed(&rows, "sub-database-frontier")
            .map(|row| datum(row, "/payload/reason"))
            .collect::<Vec<_>>(),
        ["duplicate-file", "missing-file", "unsupported-target-root"],
        "explicit reasons for malformed local selectors and unavailable selected target projection"
    );
    verify_original_link_states(&rows);
    Ok(())
}

fn verify_original_link_states(rows: &[MediaRecord]) {
    let states = typed(rows, "link")
        .map(|row| datum(row, "/payload/reference/resolution/status"))
        .collect::<Vec<_>>();
    assert_eq!(
        states,
        [
            "resolved",
            "not-selected",
            "missing",
            "ambiguous",
            "resolved",
            "resolved",
            "resolved",
            "resolved",
            "resolved",
            "resolved"
        ],
        "existing lookup keeps directory facts and noncanonical addressable declarations"
    );
}

#[test]
fn sub_database_cycles_do_not_cross_physical_archive_boundaries() -> TestResult {
    let scratch = tempfile::tempdir()?;
    let nested = TestContainer::new()
        .entry(adb("a", &["Animations/b.adb"]))
        .entry(adb("b", &["Animations/a.adb"]));
    let archive = support::archive_with(
        scratch.path(),
        [
            adb("a", &["Animations/b.adb"]),
            adb("b", &[]),
            TestEntry::stored("fixture.pak", nested.build()),
        ],
    )?;
    let (rows, _) = project(&archive, &scratch.path().join("out"), &options())?;
    let found = typed(&rows, "sub-database-cycle").collect::<Vec<_>>();
    let [row] = found.as_slice() else {
        return Err("expected one cycle in the nested directory".into());
    };
    assert!(
        matches!(&row.source.identity, p4kx_media::Identity::Entry { entry, .. } if entry.path == "fixture.pak"),
        "proven cycle retains its own physical Archive"
    );
    assert_eq!(
        cycles(&rows),
        [("0", "2", "2")],
        "immediately owning directory ordinal namespace"
    );
    Ok(())
}

#[test]
fn sub_database_cycles_walk_a_deep_graph_and_stop_on_budget_or_cancellation() -> TestResult {
    const COUNT: usize = 256;
    let scratch = tempfile::tempdir()?;
    let make_entries = || {
        (0..COUNT).map(|index| {
            adb(
                &index.to_string(),
                &[&format!("Animations/{}.adb", (index + 1) % COUNT)],
            )
        })
    };
    let archive = support::archive_with(scratch.path(), make_entries())?;
    let mut options = options();
    let output = scratch.path().join("unlimited");
    let (rows, _) = project(&archive, &output, &options)?;
    assert_eq!(
        cycles(&rows),
        [("0", "256", "256")],
        "long cycle uses no recursive depth cap"
    );

    options.limits.metadata_work = 256;
    let output = scratch.path().join("below-required-work");
    assert!(
        p4kx_media::export(&archive, &output, &options)
            .is_err_and(|error| error.to_string().contains("metadata_work")),
        "the export consumes the caller's shared budget"
    );
    assert!(
        !output.join("media-index.json").exists(),
        "budget exhaustion cannot publish a complete graph"
    );
    options.cancel.cancel();
    let output = scratch.path().join("cancelled");
    assert!(
        matches!(
            p4kx_media::export(&archive, &output, &options),
            Err(p4kx_media::MediaError::Cancelled)
        ),
        "cancellation is fatal before publication"
    );
    Ok(())
}

fn options() -> ExportOptions {
    ExportOptions {
        kinds: vec![MediaKind::AnimationDefinition],
        ..ExportOptions::default()
    }
}

fn adb(name: &str, targets: &[&str]) -> TestEntry {
    use std::fmt::Write as _;

    let mut xml = "<AnimDB><SubADBs>".to_owned();
    for target in targets {
        let _ = write!(xml, "<SubADB File=\"{target}\"/>");
    }
    xml.push_str("</SubADBs></AnimDB>");
    TestEntry::stored(format!("Data/Animations/{name}.adb"), xml)
}

fn project(
    archive: &Archive,
    output: &Path,
    options: &ExportOptions,
) -> Result<(Vec<MediaRecord>, std::path::PathBuf), Box<dyn std::error::Error>> {
    let index = support::published(p4kx_media::export(archive, output, options)?)?;
    let rows = support::records(output, &index, STREAM)?;
    let validator = jsonschema::validator_for(&p4kx_media::builtin_record_schema()?)?;
    for row in &rows {
        validator
            .validate(&serde_json::to_value(row)?)
            .map_err(|error| error.to_string())?;
    }
    Ok((rows, output.join(index.artifact_base)))
}

fn typed<'a>(
    rows: &'a [MediaRecord],
    record_type: &'a str,
) -> impl Iterator<Item = &'a MediaRecord> {
    rows.iter()
        .filter(move |row| datum(row, "/record_type") == record_type)
}

fn datum<'a>(row: &'a MediaRecord, path: &str) -> &'a str {
    row.data
        .pointer(path)
        .and_then(Value::as_str)
        .unwrap_or_default()
}

fn edges(rows: &[MediaRecord]) -> Vec<(&str, &str)> {
    typed(rows, "sub-database-edge")
        .map(|row| {
            (
                datum(row, "/payload/source_entry_index"),
                datum(row, "/payload/target_entry_index"),
            )
        })
        .collect()
}

fn cycles(rows: &[MediaRecord]) -> Vec<(&str, &str, &str)> {
    typed(rows, "sub-database-cycle")
        .map(|row| {
            (
                datum(row, "/payload/representative_entry_index"),
                datum(row, "/payload/member_count"),
                datum(row, "/payload/edge_count"),
            )
        })
        .collect()
}

fn verify_encryption(
    rows: &[MediaRecord],
    base: &Path,
    record_type: &str,
    field: &str,
    encrypted: bool,
) -> TestResult {
    let ordinal = rows
        .iter()
        .position(|row| datum(row, "/record_type") == record_type)
        .ok_or("expected original graph occurrence")?
        .to_string();
    let receipts = fs::read_to_string(base.join("field-origins.jsonl"))?;
    for line in receipts.lines() {
        let value: Value = serde_json::from_str(line)?;
        if value.pointer("/scope/ordinal").and_then(Value::as_str) == Some(&ordinal)
            && value.pointer("/scope/stream/kind").and_then(Value::as_str)
                == Some("animation-definition")
            && value.pointer("/field").and_then(Value::as_str) == Some(field)
        {
            assert_eq!(
                value
                    .get("containsEncryptedSource")
                    .and_then(Value::as_bool),
                Some(encrypted),
                "actual graph derivation sources"
            );
            assert_eq!(
                value.get("sourceSetComplete").and_then(Value::as_bool),
                Some(true),
                "all actual graph sources are retained"
            );
            return Ok(());
        }
    }
    Err("graph occurrence missing its immediate field origin".into())
}
