// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Compiling and running Queries over a synthetic DataForge: selection, order, paging and
//! compile errors, also for random Queries.

use std::error::Error;
use std::num::NonZeroUsize;

use p4kx_dataforge::query::{Query, QueryError, QueryErrorKind, Row};
use p4kx_dataforge::{DataForge, Value};
use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};

type TestResult = Result<(), Box<dyn Error>>;

const RECORDS: u32 = 300;

fn database() -> Vec<u8> {
    let mut builder = DataForgeBuilder::new()
        .structure(TestStruct::new("Base").field("id", TestType::UInt32))
        .structure(
            TestStruct::new("Derived")
                .parent("Base")
                .field("weight", TestType::Double),
        )
        .structure(TestStruct::new("Leaf").parent("Derived"))
        .structure(TestStruct::new("Other").field("id", TestType::String));
    for index in 0..RECORDS {
        let kind = match index % 4 {
            0 => "Base",
            1 => "Derived",
            2 => "Leaf",
            _ => "Other",
        };
        let path = format!(
            "libs/records/{}/r{:03}.xml",
            kind.to_lowercase(),
            RECORDS - index
        );
        let id = match kind {
            "Other" => TestValue::text(format!("other {index}")),
            _ => TestValue::UInt32(index),
        };
        let mut record = TestRecord::new(
            u128::from(index) + 1,
            kind,
            format!("{kind}.R{index}"),
            &path,
        )
        .value("id", id);
        if matches!(kind, "Derived" | "Leaf") {
            record = record.value("weight", TestValue::Double(f64::from(index) / 2.0));
        }
        builder = builder.record(record);
    }
    builder.build()
}

fn page_size(size: usize) -> NonZeroUsize {
    NonZeroUsize::new(size).unwrap_or(NonZeroUsize::MIN)
}

/// Returns every row as its record path and cell texts.
fn rows(forge: &DataForge<'_>, text: &str, size: usize) -> Result<Vec<String>, Box<dyn Error>> {
    let compiled = Query::parse(text)?.compile(forge)?;
    let run = compiled.run()?;
    let mut rows = Vec::new();
    for page in run.pages(page_size(size)) {
        assert!(page.len() <= size, "a page holds at most {size} rows");
        for row in page {
            let cells: Vec<String> = row
                .cells()
                .map_err(ToString::to_string)?
                .iter()
                .map(|cell| {
                    cell.as_ref()
                        .map_or_else(|| "-".to_owned(), Value::to_string)
                })
                .collect();
            rows.push(format!("{} {}", row.record_path(), cells.join(" ")));
        }
    }
    assert_eq!(rows.len(), run.len(), "one row per selected Record");
    Ok(rows)
}

#[test]
fn from_selects_a_type_and_every_struct_deriving_from_it() -> TestResult {
    let bytes = database();
    let forge = DataForge::open(&bytes)?;

    let base = rows(&forge, "from Base select id", 1000)?;
    let derived = rows(&forge, "from Derived select weight, id", 1000)?;
    let leaf = rows(&forge, "from Leaf", 1000)?;

    assert_eq!(base.len(), 225, "Base, Derived and Leaf");
    assert_eq!(derived.len(), 150, "Derived and Leaf");
    assert_eq!(leaf.len(), 75, "Leaf only");
    assert_eq!(
        derived.first().map(String::as_str),
        Some("libs/records/derived/r003 148.5 297"),
        "{derived:?}"
    );
    Ok(())
}

#[test]
fn rows_are_ordered_by_record_path() -> TestResult {
    let bytes = database();
    let forge = DataForge::open(&bytes)?;

    let rows = rows(&forge, "at \"*/leaf/*\"", 1000)?;
    let mut sorted = rows.clone();
    sorted.sort();

    assert_eq!(rows, sorted, "record path order");
    Ok(())
}

#[test]
fn pages_of_any_size_give_the_same_rows() -> TestResult {
    let bytes = database();
    let forge = DataForge::open(&bytes)?;
    let text = "at \"libs/records\" select id, weight";

    let whole = rows(&forge, text, 1000)?;

    assert_eq!(whole.len(), RECORDS as usize, "every Record");
    for size in [1, 7, 64, 299, 300] {
        assert_eq!(rows(&forge, text, size)?, whole, "page size {size}");
    }
    Ok(())
}

#[test]
fn any_range_of_rows_is_built_on_its_own() -> TestResult {
    let bytes = database();
    let forge = DataForge::open(&bytes)?;
    let compiled = Query::parse("from Leaf select id")?.compile(&forge)?;
    let run = compiled.run()?;
    let paths: Vec<&str> = run.record_paths().collect();

    let middle: Vec<&str> = run.rows(10..13).iter().map(Row::record_path).collect();
    let tail = run.rows(73..500);

    assert_eq!(
        middle,
        paths.get(10..13).unwrap_or_default(),
        "rows 10 to 12"
    );
    assert_eq!(tail.len(), 2, "cut at the last row");
    assert!(run.rows(500..600).is_empty(), "past the end");
    Ok(())
}

#[test]
fn without_from_each_record_uses_its_own_type() -> TestResult {
    let bytes = database();
    let forge = DataForge::open(&bytes)?;

    let rows = rows(&forge, "at \"*/r300\" select id, weight", 10)?;
    let other = rows_of(&forge, "at \"*/other/r297\" select id, weight")?;

    assert_eq!(rows, ["libs/records/base/r300 0 -"], "Base has no weight");
    assert_eq!(other, ["libs/records/other/r297 other 3 -"], "a string id");
    Ok(())
}

fn rows_of(forge: &DataForge<'_>, text: &str) -> Result<Vec<String>, Box<dyn Error>> {
    rows(forge, text, 10)
}

#[test]
fn cells_hold_the_values_of_the_record_tree() -> TestResult {
    let bytes = database();
    let forge = DataForge::open(&bytes)?;
    let compiled = Query::parse("at \"*\" select weight, id")?.compile(&forge)?;
    let run = compiled.run()?;

    for page in run.pages(page_size(50)) {
        for row in page {
            let tree = row.record().tree()?;
            let expected: Vec<Option<Value<'_>>> = ["weight", "id"]
                .iter()
                .map(|name| {
                    tree.fields()
                        .iter()
                        .find(|field| field.name() == *name)
                        .map(|field| field.value().clone())
                })
                .collect();
            assert_eq!(
                row.cells().map_err(ToString::to_string)?,
                expected.as_slice(),
                "{}",
                row.record_path()
            );
        }
    }
    Ok(())
}

#[test]
fn a_field_of_a_subtype_is_unknown_to_its_parent() -> TestResult {
    let bytes = database();
    let forge = DataForge::open(&bytes)?;

    let error = Query::parse("from Base select wieght")?
        .compile(&forge)
        .err();

    assert!(
        matches!(
            error.as_ref().map(QueryError::kind),
            Some(QueryErrorKind::UnknownField {
                suggestion: None,
                owner: Some(_),
                ..
            })
        ),
        "{error:?}"
    );
    let error = Query::parse("at \"*\" select wieght")?
        .compile(&forge)
        .err();
    assert!(
        matches!(
            error.as_ref().map(QueryError::kind),
            Some(QueryErrorKind::UnknownField { suggestion: Some(name), .. }) if name == "weight"
        ),
        "{error:?}"
    );
    Ok(())
}

#[test]
fn random_queries_compile_or_fail_with_a_span_inside_the_query() -> TestResult {
    let bytes = database();
    let forge = DataForge::open(&bytes)?;
    let words = [
        "Base", "Derived", "Leaf", "Other", "id", "weight", "x", "base", "Wieght",
    ];
    let mut state = 0x0123_4567_89ab_cdef_u64;
    let mut next = move |bound: usize| {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        usize::try_from(state % bound as u64).unwrap_or(0)
    };
    let mut word = move || words.get(next(words.len())).copied().unwrap_or_default();
    for round in 0..500_usize {
        let mut text = format!("from {}", word());
        if round % 2 == 0 {
            text += " at \"*/r1*\"";
        }
        text += " select ";
        let paths: Vec<String> = (0..=round % 3)
            .map(|path| {
                let mut path_text = word().to_owned();
                if (round + path) % 4 == 0 {
                    path_text.push('.');
                    path_text.push_str(word());
                }
                path_text
            })
            .collect();
        text += &paths.join(", ");
        match Query::parse(&text)?.compile(&forge) {
            Ok(compiled) => {
                let run = compiled.run()?;
                let count: usize = run.pages(page_size(16)).map(|page| page.len()).sum();
                assert_eq!(count, run.len(), "{text}");
            }
            Err(error) => {
                assert!(error.span().end() <= text.len(), "{text}: {error:?}");
                assert!(error.span().start() >= 5, "{text}: {error:?}");
            }
        }
    }
    Ok(())
}

#[test]
fn a_run_that_is_stopped_gives_nothing() -> TestResult {
    let bytes = database();
    let forge = DataForge::open(&bytes)?;
    let compiled = Query::parse("from Base select id")?.compile(&forge)?;

    assert!(compiled.run_while(|| false)?.is_none(), "stopped at once");
    assert_eq!(
        compiled.run_while(|| true)?.map(|run| run.len()),
        Some(225),
        "never stopped"
    );
    Ok(())
}

#[test]
fn a_selection_resumes_to_the_same_rows() -> TestResult {
    let bytes = database();
    let forge = DataForge::open(&bytes)?;
    let compiled = Query::parse("from Derived select weight")?.compile(&forge)?;
    let run = compiled.run()?;

    let resumed = compiled.resume(run.indexes().to_vec());

    assert_eq!(resumed.len(), run.len(), "the same selection");
    assert_eq!(
        resumed.record_paths().collect::<Vec<_>>(),
        run.record_paths().collect::<Vec<_>>(),
        "in the same order"
    );
    Ok(())
}
