// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;
use std::error::Error;

use p4kx_comparison::{CompareOptions, Filter, compare};
use serde_json::{Value, json};

use super::entries::{entries_only, pair};

type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn totals_and_groups_use_the_observed_members() -> TestResult {
    let (_dir, old, new) = pair()?;
    let report = serde_json::to_value(compare(&old, &new, None, &entries_only())?.report())?;
    let added = refs(&report, "/entries/changes/0/change")?;
    let checksum = refs(&report, "/entries/changes/1/change")?;
    let size = refs(&report, "/entries/changes/2/change")?;
    let removed = refs(&report, "/entries/changes/3/change")?;
    let exhausted = refs(&report, "/entries/changes/4/change")?;
    let matched: BTreeSet<_> = checksum.union(&size).copied().collect();
    assert_eq!(
        refs(&report, "/entries/counts/changed")?,
        matched,
        "only matched positives"
    );
    assert_eq!(
        refs(&report, "/entries/counts/added")?,
        added.union(&exhausted).copied().collect(),
        "ordered and exhausted positive adds",
    );
    assert_eq!(
        refs(&report, "/entries/counts/removed")?,
        removed,
        "only removed row"
    );
    marker(&report, "/entries/counts/changed", Some(false), true)?;
    marker(&report, "/entries/counts/added", None, false)?;
    marker(&report, "/entries/counts/removed", None, false)?;

    let anchor = grouped_path_use(&report, 0, "selector")?;
    assert_eq!(
        refs(&report, "/entries/directories/1/changed")?,
        matched.into_iter().chain([anchor]).collect(),
        "the first Data path sets the group spelling, even when added",
    );
    let label = grouped_path_use(&report, 0, "conversion")?;
    assert_eq!(
        refs(&report, "/entries/directories/1/directory")?,
        BTreeSet::from([label]),
        "first spelling"
    );
    marker(&report, "/entries/directories/1/changed", Some(false), true)?;
    marker(&report, "/entries/directories/1/added", None, false)?;

    let root_path = grouped_path_use(&report, 4, "selector")?;
    assert_eq!(
        refs(&report, "/entries/directories/0/added")?,
        exhausted.into_iter().chain([root_path]).collect(),
        "the exhausted branch still has an actual grouping path",
    );
    let root_label = grouped_path_use(&report, 4, "conversion")?;
    assert_eq!(
        refs(&report, "/entries/directories/0/directory")?,
        BTreeSet::from([root_label]),
        "empty root label has a source"
    );
    marker(&report, "/entries/directories/0/added", None, false)?;
    assert_zero(&report, "/entries/directories/0/changed")
}

#[test]
fn filtered_and_empty_aggregates_have_no_unobserved_contributors() -> TestResult {
    let (_dir, old, new) = pair()?;
    let filter = Filter::new("DATA/LIBS/*.xml");
    let options = CompareOptions {
        filter: Some(&filter),
        ..entries_only()
    };
    let report = serde_json::to_value(compare(&old, &new, None, &options)?.report())?;
    let expected: BTreeSet<_> = refs(&report, "/entries/changes/0/change")?
        .into_iter()
        .chain(refs(&report, "/entries/changes/1/change")?)
        .collect();
    for path in ["/entries/counts/changed", "/entries/directories/0/changed"] {
        assert_eq!(
            refs(&report, path)?,
            expected,
            "the filter admitted only these matches"
        );
        marker(&report, path, Some(false), true)?;
    }
    for path in [
        "/entries/counts/added",
        "/entries/counts/removed",
        "/entries/directories/0/added",
        "/entries/directories/0/removed",
    ] {
        assert_zero(&report, path)?;
    }

    let nonmatching = Filter::new("nothing/*.xml");
    let options = CompareOptions {
        filter: Some(&nonmatching),
        ..entries_only()
    };
    let empty = serde_json::to_value(compare(&old, &new, None, &options)?.report())?;
    assert_eq!(
        empty.pointer("/entries/directories"),
        Some(&json!([])),
        "no groups were emitted"
    );
    for kind in ["added", "removed", "changed"] {
        assert_zero(&empty, &format!("/entries/counts/{kind}"))?;
    }
    Ok(())
}

fn origin<'a>(report: &'a Value, path: &str) -> Result<&'a Value, &'static str> {
    report
        .pointer("/entries/fieldOrigins")
        .and_then(|origins| origins.get(path))
        .ok_or("field origin")
}

fn refs<'a>(report: &'a Value, path: &str) -> Result<BTreeSet<&'a str>, &'static str> {
    origin(report, path)?
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or("origin references")?
        .iter()
        .map(|value| value.as_str().ok_or("origin reference string"))
        .collect()
}

fn marker(report: &Value, path: &str, encrypted: Option<bool>, complete: bool) -> TestResult {
    let item = origin(report, path)?;
    assert_eq!(
        item.get("containsEncryptedSource"),
        Some(&json!(encrypted)),
        "effective encryption at {path}",
    );
    assert_eq!(
        item.get("sourceSetComplete"),
        Some(&json!(complete)),
        "contributor completeness at {path}",
    );
    Ok(())
}

fn assert_zero(report: &Value, path: &str) -> TestResult {
    assert_eq!(
        report.pointer(path),
        Some(&json!(0)),
        "no emitted member at {path}"
    );
    assert!(
        refs(report, path)?.is_empty(),
        "no population proof was captured for {path}"
    );
    marker(report, path, None, false)
}

fn grouped_path_use<'a>(
    report: &'a Value,
    row: usize,
    role: &str,
) -> Result<&'a str, &'static str> {
    let path = format!("/entries/changes/{row}/path");
    let sources = refs(report, &path)?;
    let matching = report
        .pointer("/entries/provenance/uses")
        .and_then(Value::as_array)
        .ok_or("source inventory")?;
    let path_fact = matching
        .iter()
        .find(|fact| {
            fact.get("id")
                .and_then(Value::as_str)
                .is_some_and(|id| sources.contains(id))
        })
        .ok_or("original selected path fact")?;
    matching
        .iter()
        .find(|fact| {
            fact.get("source") == path_fact.get("source")
                && fact.get("location") == path_fact.get("location")
                && fact.get("role").and_then(Value::as_str) == Some(role)
        })
        .and_then(|fact| fact.get("id").and_then(Value::as_str))
        .ok_or("selected path in expected role")
}
