// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! `referencing` combined with the other clauses, and how it gets the index it reads.

use std::cell::Cell;

use p4kx_dataforge::query::{Query, QueryErrorKind};
use p4kx_dataforge::{DataForge, ReferenceIndex, Value};
use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};

type TestResult = Result<(), Box<dyn std::error::Error>>;

const HUB: u128 = 1;
const ALPHA: u128 = 2;
const BETA: u128 = 3;

fn builder() -> DataForgeBuilder {
    DataForgeBuilder::new()
        .structure(TestStruct::new("Hub").field("name", TestType::String))
        .structure(
            TestStruct::new("Ship")
                .field("name", TestType::String)
                .field("mass", TestType::Int32)
                .field("referencing", TestType::Int32)
                .field("hub", TestType::Reference),
        )
        .record(TestRecord::new(HUB, "Hub", "Hub.Main", "libs/hub.xml"))
        .record(
            TestRecord::new(ALPHA, "Ship", "Ship.Alpha", "libs/ships/alpha.xml")
                .value("name", TestValue::text("alpha"))
                .value("mass", TestValue::Int32(10))
                .value("referencing", TestValue::Int32(7))
                .value("hub", TestValue::Reference(HUB)),
        )
        .record(
            TestRecord::new(BETA, "Ship", "Ship.Beta", "libs/ships/beta.xml")
                .value("name", TestValue::text("beta"))
                .value("mass", TestValue::Int32(30))
                .value("hub", TestValue::Reference(HUB)),
        )
}

fn with_forge(check: impl Fn(&DataForge<'_>) -> TestResult) -> TestResult {
    for version in [6, 8] {
        let bytes = builder().version(version).build();
        let forge = DataForge::open(&bytes)?;
        check(&forge).map_err(|error| format!("version {version}: {error}"))?;
    }
    Ok(())
}

fn rows(forge: &DataForge<'_>, text: &str) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let compiled = Query::parse(text)?.compile(forge)?;
    let run = compiled.run()?;
    let mut rows = Vec::new();
    for page in run.pages(std::num::NonZeroUsize::MIN.saturating_add(99)) {
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
    Ok(rows)
}

#[test]
fn referencing_combines_with_select_and_where() -> TestResult {
    with_forge(|forge| {
        let all = rows(
            forge,
            r#"referencing "libs/hub" select mass, `referencing`"#,
        )?;
        assert_eq!(
            all,
            ["libs/ships/alpha 10 7", "libs/ships/beta 30 0"],
            "the columns of the referrers, a field named like the keyword included"
        );
        let heavy = rows(
            forge,
            r#"from Ship referencing "libs/hub" select mass where mass > 20"#,
        )?;
        assert_eq!(heavy, ["libs/ships/beta 30"], "the condition filters them");
        Ok(())
    })
}

#[test]
fn compiling_without_the_index_is_an_error_for_referencing_only() -> TestResult {
    with_forge(|forge| {
        let text = r#"from Ship referencing "libs/hub""#;
        let error = Query::parse(text)?
            .compile_with(forge, None)
            .err()
            .ok_or("compiled without an index")?;
        assert_eq!(error.kind(), &QueryErrorKind::MissingReferenceIndex, "kind");
        assert_eq!(&text[error.span().range()], r#""libs/hub""#, "span");
        let plain = Query::parse("from Ship select name")?;
        assert!(plain.compile_with(forge, None).is_ok(), "no clause");
        let index = ReferenceIndex::build(forge);
        let given = Query::parse(text)?.compile_with(forge, Some(&index))?;
        assert_eq!(given.run()?.len(), 2, "with the index");
        Ok(())
    })
}

#[test]
fn the_index_is_asked_for_only_after_the_names_are_checked() -> TestResult {
    with_forge(|forge| {
        let asked = Cell::new(0);
        let ask = || {
            asked.set(asked.get() + 1);
            Some(ReferenceIndex::build(forge))
        };
        for text in [
            r#"from Nothing referencing "libs/hub""#,
            r#"from Ship referencing "libs/hub" select nothing"#,
            r#"from Ship referencing "libs/nowhere""#,
        ] {
            let failed = Query::parse(text)?.compile_lazily(forge, ask).is_err();
            assert!(failed, "{text}");
        }
        assert_eq!(asked.get(), 0, "no index built for a Query that fails");
        let compiled =
            Query::parse(r#"from Ship referencing "libs/hub""#)?.compile_lazily(forge, ask)?;
        assert_eq!(compiled.run()?.len(), 2, "the Query that compiles");
        assert_eq!(asked.get(), 1, "built once for it");
        Ok(())
    })
}
