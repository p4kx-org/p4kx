// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::health_origin_assertions as check;
use crate::health_origin_entry_source as entries;
use crate::health_origin_fixture::{self as fixture, Fixture, TestResult};
use p4kx_export::{CatalogError, ExportDiagnosticCode, build_catalog_from};
use p4kx_testkit::{TestContainer, TestEntry};
use serde_json::{Value, json};

#[test]
fn failed_plain_children_retain_distinct_original_encrypted_container_ancestry() -> TestResult {
    let source = entries::with_starmap(vec![
        entries::container("alpha", b"<broken>", true),
        entries::container("beta", b"<broken>", true),
    ])?;
    let mut originals = Vec::new();
    for name in ["alpha", "beta"] {
        let outer = entries::original(&source.archive, &entries::outer(name))?;
        assert!(
            outer.is_encrypted(),
            "actual enclosing payload is encrypted"
        );
        let container = source.archive.open_container(outer)?;
        let inner = entries::original(&container, entries::INNER)?;
        assert!(
            !inner.is_encrypted(),
            "the child directory genuinely declares plain payload"
        );
        assert_eq!(
            entries::read(&container, inner)?,
            b"<broken>",
            "the exact original malformed child bytes are readable"
        );
        originals.push((outer.index(), inner.index(), outer.path().to_owned()));
    }
    let (_, catalog) = source.diagnostic()?;
    let failed = entries::failure(&catalog, "lootContainers", "entityLinks")?;
    assert_eq!(
        failed.len(),
        2,
        "equal child names in separate original containers are distinct failures"
    );
    let paired = originals.iter().zip(failed);
    for ((outer_index, inner_index, outer_path), row) in paired {
        let path = check::present_text(
            &catalog,
            row,
            "path",
            &json!(format!("{outer_path}!{}", entries::INNER)),
            Some(true),
        )?;
        let actual = entries::metadata_paths(&catalog, path)?;
        assert_eq!(
            actual,
            vec![outer_path.clone(), entries::INNER.to_owned()],
            "the composite identity uses both actual original directory paths"
        );
        join_conversions(&catalog, path, outer_path)?;
        check::present(row, "entryIndex", &json!(outer_index), Some(false))?;
        check::present(row, "containerEntryIndex", &json!(inner_index), Some(true))?;
        check::independent(check::cell(row, "targetGuid", "absent")?)?;
    }
    check::closure(&catalog)
}

// The target lies outside the independently enumerated shop containers.
const TARGET: &str = "Data\\FixtureContainers\\ambiguous.socpak";

fn loot_route(copies: usize) -> TestResult<Fixture> {
    let xml = format!("<Entity objectContainer=\"{TARGET}\"/>");
    let mut selected = vec![
        entries::container("alpha", xml.as_bytes(), true),
        TestEntry::stored(fixture::REGISTRY, b"<LayerBackups/>"),
    ];
    for ordinal in 0..copies {
        let container = TestContainer::new()
            .entry(TestEntry::stored("contents.txt", ordinal.to_string()))
            .build();
        selected.push(TestEntry::stored(TARGET, container));
    }
    entries::with_starmap(selected)
}

#[test]
fn ambiguous_external_loot_selection_blocks_normal_and_retains_selector_and_candidates()
-> TestResult {
    let unique = loot_route(1)?;
    build_catalog_from(&unique.archive, &unique.entry, &fixture::options())?;

    let duplicate = loot_route(2)?;
    let result = build_catalog_from(&duplicate.archive, &duplicate.entry, &fixture::options());
    let Err(CatalogError::IncompleteExport(refusal)) = result else {
        return Err("normal publication must refuse the duplicate candidate set".into());
    };
    assert!(
        refusal
            .health()
            .failed_entries()
            .iter()
            .any(|entry| { entry.code() == ExportDiagnosticCode::AmbiguousIdentity }),
        "the refusal is about ambiguity",
    );

    let (_, catalog) = duplicate.diagnostic()?;
    let failed = entries::failure(&catalog, "lootContainers", "enumeration")?;
    let physical: Vec<_> = duplicate
        .archive
        .entries()
        .iter()
        .filter(|entry| entry.path() == TARGET)
        .map(p4kx_extraction::Entry::index)
        .collect();
    assert_eq!(failed.len(), 2);
    assert_eq!(failed.len(), physical.len());
    for (row, index) in failed.into_iter().zip(physical) {
        assert_eq!(check::at(row, "/code")?, &json!("ambiguousIdentity"));
        assert_eq!(check::at(row, "/stage")?, &json!("selection"));
        check::present(row, "entryIndex", &json!(index), Some(false))?;
        let path = check::present_text(&catalog, row, "path", &json!(TARGET), Some(true))?;
        assert!(has_original_loot_selector(&catalog, path)?);
    }
    assert_no_candidate_payload_attempts(&catalog)?;
    check::closure(&catalog)
}

fn assert_no_candidate_payload_attempts(catalog: &Value) -> TestResult {
    for used in check::rows(catalog, "/provenance/uses")? {
        if used.get("role") == Some(&json!("attempted"))
            && used.get("scope") == Some(&json!("payload"))
        {
            let source = check::source(catalog, used)?;
            assert_ne!(
                source.pointer("/path/value"),
                Some(&json!(TARGET)),
                "enumerating physical candidates does not open their payloads"
            );
        }
    }
    Ok(())
}

fn has_original_loot_selector(catalog: &Value, path: &Value) -> TestResult<bool> {
    Ok(check::uses(catalog, path)?.iter().any(|used| {
        used.get("scope") == Some(&json!("payload"))
            && used
                .pointer("/location/field")
                .and_then(Value::as_str)
                .is_some_and(|field| field.contains("objectContainer"))
            && check::source(catalog, used)
                .is_ok_and(|source| source.pointer("/path/value") == Some(&json!(entries::INNER)))
    }))
}

fn join_conversions(catalog: &Value, cell: &Value, outer: &str) -> TestResult {
    let uses = check::uses(catalog, cell)?;
    let mut paths = Vec::new();
    for used in uses {
        if used.get("role") != Some(&json!("conversion")) {
            continue;
        }
        assert_eq!(
            used.get("scope"),
            Some(&json!("archiveDirectoryMetadata")),
            "joining admitted paths uses their actual directory facts"
        );
        assert_eq!(
            used.pointer("/location/type"),
            Some(&json!("metadata")),
            "the composite conversion keeps an original metadata coordinate"
        );
        assert_eq!(
            used.pointer("/location/field"),
            Some(&json!("path")),
            "only the joined path facts participate in this conversion"
        );
        let source = check::source(catalog, used)?;
        assert_eq!(
            source.get("kind"),
            Some(&json!("archiveEntry")),
            "the composite path does not invent an unrelated conversion source"
        );
        paths.push(
            check::at(source, "/path/value")?
                .as_str()
                .ok_or("original joined path")?,
        );
    }
    paths.sort_unstable();
    let mut expected = [outer, entries::INNER];
    expected.sort_unstable();
    assert_eq!(
        paths, expected,
        "the actual outer!inner join retains each original path's conversion once"
    );
    Ok(())
}
