// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! `->` follows References and weak pointers into the Records and instances they link to, in
//! `select` and in `where`, chained and behind array elements.

use std::error::Error;
use std::sync::Mutex;

use crate::fleet::{database, rows};
use p4kx_dataforge::DataForge;
use p4kx_dataforge::query::{
    MAX_PATH_REFERENCES, Query, QueryByteUse, QueryError, QueryErrorKind, QuerySource, Segment,
};

type TestResult = Result<(), Box<dyn Error>>;

fn assert_rows(forge: &DataForge<'_>, text: &str, expected: &[&str]) -> TestResult {
    assert_eq!(rows(forge, text)?, expected, "{text}");
    Ok(())
}

/// Returns the message and the marked text of the error `text` compiles to.
fn error(forge: &DataForge<'_>, text: &str) -> Result<(String, String), Box<dyn Error>> {
    let query = Query::parse(text)?;
    let Err(error) = query.compile(forge) else {
        return Err(format!("{text} compiles").into());
    };
    let marked = text
        .get(error.span().range())
        .unwrap_or_default()
        .to_owned();
    Ok((error.to_string(), marked))
}

#[test]
fn an_arrow_continues_the_path_in_the_referenced_record_and_chains() -> TestResult {
    let bytes = database();
    let forge = DataForge::open(&bytes)?;

    assert_rows(
        &forge,
        "from Ship select maker->name, maker->home->name, maker->home->mass, mass",
        &[
            "alpha: Aegis | Terra | 5.5 | 100 (0 unresolved)",
            "beta: null | null | null | 50 (1 unresolved)",
            "gamma: null | null | null | 0 (1 unresolved)",
        ],
    )
}

#[test]
fn an_arrow_follows_each_element_of_an_array_of_references() -> TestResult {
    let bytes = database();
    let forge = DataForge::open(&bytes)?;

    assert_rows(
        &forge,
        "from Ship select escorts[*]->name, escorts[0]->mass, escorts[*]->escorts[*]->name",
        &[
            "alpha: [Beta, null, null] | 50 | [Alpha, null, null] (2 unresolved)",
            "beta: [Alpha] | 100 | [Beta, null, null] (2 unresolved)",
            "gamma: [] | null | [] (0 unresolved)",
        ],
    )
}

#[test]
fn an_arrow_reads_the_declared_type_and_checks_the_target_against_it() -> TestResult {
    let bytes = database();
    let forge = DataForge::open(&bytes)?;
    let rows = rows(&forge, "from Ship select shield->name")?;

    assert_eq!(
        rows.get(..2),
        Some(
            &[
                "alpha: Bubble (0 unresolved)".to_owned(),
                "beta: null (1 unresolved)".to_owned()
            ][..]
        ),
        "a Shield is a Component, and a null Reference reaches null"
    );
    let gamma = rows.get(2).ok_or("three rows")?;
    assert!(
        gamma.starts_with("gamma: error: the Reference refers to the Record ")
            && gamma.ends_with(
                " of struct 1, which does not derive from struct 2, the type of the field \
                 (0 unresolved)"
            ),
        "a Manufacturer is no Component: {gamma}"
    );
    Ok(())
}

#[test]
fn an_arrow_follows_a_weak_pointer_to_its_instance() -> TestResult {
    let bytes = database();
    let forge = DataForge::open(&bytes)?;

    assert_rows(
        &forge,
        "from Ship select pilot->role",
        &[
            "alpha: ace (0 unresolved)",
            "beta: null (1 unresolved)",
            "gamma: null (1 unresolved)",
        ],
    )
}

#[test]
fn a_weak_pointer_to_another_struct_or_to_no_instance_reaches_an_error() -> TestResult {
    let bytes = database();
    let forge = DataForge::open(&bytes)?;

    assert_rows(
        &forge,
        "from Dock select name, pilot->role",
        &[
            "lost: Lost | error: the pointer refers to instance 99 of struct 0, which does not exist \
             (0 unresolved)",
            "planet: Planet | error: the pointer refers to struct 0, which does not derive from \
             struct 4, the type of the field (0 unresolved)",
        ],
    )
}

#[test]
fn an_arrow_works_in_where_and_a_missing_target_is_unknown() -> TestResult {
    let bytes = database();
    let forge = DataForge::open(&bytes)?;
    let cases: [(&str, &[&str]); 5] = [
        ("maker->home->name = \"Terra\"", &["alpha:  (0 unresolved)"]),
        ("escorts[*]->name = \"Alpha\"", &["beta:  (0 unresolved)"]),
        ("not maker->name = \"Aegis\"", &[]),
        ("not maker->name = \"Other\"", &["alpha:  (0 unresolved)"]),
        ("pilot->role starts with \"A\"", &["alpha:  (0 unresolved)"]),
    ];
    for (condition, expected) in cases {
        assert_rows(&forge, &format!("from Ship where {condition}"), expected)?;
    }
    assert_rows(
        &forge,
        "from Ship select name where maker->name = \"Aegis\" or escorts[*]->name = \"Alpha\"",
        &["alpha: Alpha (0 unresolved)", "beta: Beta (0 unresolved)"],
    )
}

#[test]
fn where_reference_lookup_is_observed_only_with_record_admissions() -> TestResult {
    let bytes = database();
    let forge = DataForge::open(&bytes)?;
    let query = Query::parse("from Ship where maker->home->mass = 5.5")?;
    let compiled = query.compile(&forge)?;
    let ordinary = Mutex::new(Vec::new());
    let detailed = Mutex::new(Vec::new());
    let ordinary_run = compiled
        .run_observed(
            || true,
            |event| {
                ordinary
                    .lock()
                    .expect("ordinary fixture events")
                    .push(event);
            },
        )?
        .ok_or("ordinary selection stopped")?;
    let detailed_run = compiled
        .run_observing_records(
            || true,
            |event| {
                detailed
                    .lock()
                    .expect("detailed fixture events")
                    .push(event);
            },
            |_| {},
        )?
        .ok_or("detailed selection stopped")?;
    assert_eq!(ordinary_run.indexes(), detailed_run.indexes());
    assert_eq!(ordinary_run.indexes().len(), 1);

    let ordinary = ordinary.into_inner()?;
    assert!(ordinary.iter().any(|event| matches!(
        event,
        QuerySource::Bytes {
            kind: QueryByteUse::Value,
            ..
        }
    )));
    assert!(!ordinary.iter().any(|event| matches!(
        event,
        QuerySource::ReferenceTarget { .. }
            | QuerySource::RecordLookup { .. }
            | QuerySource::RecordType { .. }
            | QuerySource::InstanceExtent { .. }
    )));
    let detailed = detailed.into_inner()?;
    let target = detailed
        .iter()
        .find_map(|event| match *event {
            QuerySource::ReferenceTarget { index, .. } => Some(index),
            _ => None,
        })
        .ok_or("detailed traversal records the GUID lookup target")?;
    assert!(detailed.contains(&QuerySource::RecordType { index: target }));
    assert!(ordinary.contains(&QuerySource::Record { index: target }));
    Ok(())
}

#[test]
fn without_from_each_type_resolves_the_arrow_itself() -> TestResult {
    let bytes = database();
    let forge = DataForge::open(&bytes)?;

    assert_rows(
        &forge,
        "at \"libs\" select home->name, maker->home->name where name starts with \"A\"",
        &[
            "aegis: Terra | - (0 unresolved)",
            "alpha: - | Terra (0 unresolved)",
        ],
    )
}

#[test]
fn an_arrow_where_no_reference_stands_is_marked() -> TestResult {
    let bytes = database();
    let forge = DataForge::open(&bytes)?;
    let cases = [
        (
            "from Ship select mass->name",
            "field `mass` of `Ship` holds a Single value, which is no Reference; `->` follows \
             References and weak pointers only",
            "->",
        ),
        (
            "from Ship select escorts->name",
            "field `escorts` of `Ship` holds an array of References; select its elements with \
             `[n]` or `[*]` before `->`",
            "->",
        ),
        (
            "from Ship select maker->home.name",
            "field `home` of `Manufacturer` holds a Reference; follow it with `->name` to reach \
             the field `name` of what it links to",
            "name",
        ),
        (
            "from Ship select maker->nam",
            "unknown field `nam` of `Manufacturer`; did you mean `name`?",
            "nam",
        ),
        (
            "from Ship select shield->hp",
            "unknown field `hp` of `Component`",
            "hp",
        ),
        (
            "from Ship select plain->name",
            "field `plain` of `Ship` holds a Reference without a declared type, so `->` cannot \
             reach its field `name`",
            "->",
        ),
        (
            "from Ship select maker->home",
            "field `home` of `Manufacturer` holds a Reference; a path has to end at a scalar \
             value",
            "home",
        ),
        (
            "from Ship where maker->name > 5",
            "`>` cannot compare field `maker->name` of `Ship`, which holds String values, with \
             an integer",
            "maker->name > 5",
        ),
    ];
    for (text, message, marked) in cases {
        assert_eq!(
            error(&forge, text)?,
            (message.to_owned(), marked.to_owned()),
            "{text}"
        );
    }
    Ok(())
}

#[test]
fn an_arrow_prints_without_spaces_and_carries_its_spans() -> TestResult {
    let query = Query::parse("from Ship select maker -> home->`name`, escorts [*] -> name")?;
    assert_eq!(
        query.to_string(),
        "from Ship select maker->home->name, escorts[*]->name"
    );
    assert_eq!(
        Query::parse(&query.to_string())?,
        Query::parse("from Ship select maker->home->name, escorts[*]->name")?
    );
    let Some(path) = query.select().first() else {
        return Err("a path".into());
    };
    let spans: Vec<String> = path
        .tail()
        .iter()
        .map(|segment| match segment {
            Segment::Follow { arrow, name } => format!(
                "{} {:?} {:?}",
                name.as_str(),
                arrow.range(),
                segment.span().range()
            ),
            _ => "?".to_owned(),
        })
        .collect();
    assert_eq!(spans, ["home 23..25 23..30", "name 30..32 30..38"]);
    Ok(())
}

#[test]
fn a_path_follows_a_bounded_number_of_references() -> TestResult {
    let hops = |count: usize| format!("from Ship select a{}", "->b".repeat(count));
    assert!(Query::parse(&hops(MAX_PATH_REFERENCES)).is_ok());

    let text = hops(MAX_PATH_REFERENCES + 1);
    let error = Query::parse(&text).err().ok_or("too many References")?;
    assert_eq!(
        error.kind(),
        &QueryErrorKind::TooManyReferences {
            limit: MAX_PATH_REFERENCES
        }
    );
    assert_eq!(text.get(error.span().range()), Some("->"));
    assert_eq!(
        error.span().start(),
        text.len() - 3,
        "the first arrow beyond the limit"
    );
    Ok(())
}

#[test]
fn an_arrow_needs_a_field_name_behind_it() {
    let kind = |text: &str| {
        Query::parse(text)
            .err()
            .as_ref()
            .map(QueryError::kind)
            .cloned()
    };

    assert!(
        matches!(
            kind("from Ship select maker->[0]"),
            Some(QueryErrorKind::Expected { .. })
        ),
        "a bracket is no field"
    );
    assert!(
        matches!(
            kind("from Ship select maker->"),
            Some(QueryErrorKind::Expected { .. })
        ),
        "the end is no field"
    );
}
