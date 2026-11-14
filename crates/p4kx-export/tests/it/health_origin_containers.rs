// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::health_origin_assertions as check;
use crate::health_origin_entry_source as entries;
use crate::health_origin_fixture::TestResult;
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
