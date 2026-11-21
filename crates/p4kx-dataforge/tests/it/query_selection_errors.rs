// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Demanded selection failures, lazy proofs, normal unknowns and authoritative cancellation.

use std::error::Error;
use std::sync::atomic::{AtomicUsize, Ordering};

use p4kx_dataforge::query::{Query, QueryRun};
use p4kx_dataforge::{DataForge, Value, ValueError};
use p4kx_testkit::query_selection_sample;
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

type TestResult = Result<(), Box<dyn Error>>;

fn selected(forge: &DataForge<'_>, condition: &str) -> Result<Vec<u32>, Box<dyn Error>> {
    let compiled = Query::parse(&format!("from Station where {condition}"))?.compile(forge)?;
    Ok(compiled.run()?.indexes().to_vec())
}

#[test]
fn demanded_read_failures_name_the_record_comparison_and_value_error() -> TestResult {
    let bytes = query_selection_sample::database().build();
    let forge = DataForge::open(&bytes)?;
    for (condition, comparison) in [
        ("ready = true", 0),
        ("not ready = true", 0),
        ("ready = true or id = 8", 0),
        ("id = 8 or ready = true", 1),
        ("ready = true and id = 7", 0),
        ("id = 7 and ready = true", 1),
        ("owner.ready = true or ready = true", 1),
        ("owner.ready = true and ready = true", 1),
        ("flags[*] = false", 0),
    ] {
        let compiled = Query::parse(&format!("from Station where {condition}"))?.compile(&forge)?;
        let error = compiled
            .run()
            .err()
            .ok_or("an undecidable condition succeeded")?;
        assert_eq!(error.record_index(), 0, "{condition}");
        assert_eq!(error.record_path(), "synthetic/station", "{condition}");
        assert_eq!(error.comparison_index(), comparison, "{condition}");
        assert_eq!(
            error.value_error(),
            &ValueError::InvalidBool { byte: 2 },
            "{condition}"
        );
        assert!(Error::source(&error).is_some(), "the cause is preserved");
    }
    Ok(())
}

#[test]
fn a_decisive_proof_does_not_demand_irrelevant_failures() -> TestResult {
    let bytes = query_selection_sample::database().build();
    let forge = DataForge::open(&bytes)?;
    for condition in [
        "ready = true or id = 7",
        "id = 7 or ready = true",
        "flags[*] = true",
        "owner.ready = true or flags[*] = true",
        "(ready = true and id = 8) or flags[*] = true",
    ] {
        assert_eq!(selected(&forge, condition)?, [0], "{condition}");
    }
    for condition in [
        "ready = true and id = 8",
        "id = 8 and ready = true",
        "not flags[*] = true",
    ] {
        assert!(selected(&forge, condition)?.is_empty(), "{condition}");
    }
    Ok(())
}

#[test]
fn normal_unknowns_retain_kleene_logic() -> TestResult {
    let bytes = query_selection_sample::database().build();
    let forge = DataForge::open(&bytes)?;
    for condition in [
        "owner.ready = true",
        "not owner.ready = true",
        "owner.ready = true and id = 8",
    ] {
        assert!(selected(&forge, condition)?.is_empty(), "{condition}");
    }
    assert_eq!(
        selected(&forge, "owner.ready = true or id = 7")?,
        [0],
        "true OR unknown is true"
    );
    for condition in ["ready = true", "not ready = true"] {
        let compiled =
            Query::parse(&format!("at \"synthetic/other\" where {condition}"))?.compile(&forge)?;
        assert!(compiled.run()?.is_empty(), "a missing field is unknown");
    }
    Ok(())
}

#[test]
fn cancellation_within_one_array_is_latched_and_drops_provisional_failures() -> TestResult {
    let bytes = query_selection_sample::database().build();
    let forge = DataForge::open(&bytes)?;
    for condition in ["tags[*] = 4096", "ready = true or tags[*] = -1"] {
        let calls = AtomicUsize::new(0);
        let compiled = Query::parse(&format!("from Station where {condition}"))?.compile(&forge)?;
        let run = compiled.run_while(|| calls.fetch_add(1, Ordering::Relaxed) != 31)?;
        assert!(
            run.is_none(),
            "the transient stop stays authoritative: {condition}"
        );
        assert!(calls.load(Ordering::Relaxed) >= 32, "work reached the stop");
    }
    let calls = AtomicUsize::new(0);
    let compiled = Query::parse("from Station where tags[*] = 1")?.compile(&forge)?;
    let run = compiled.run_while(|| calls.fetch_add(1, Ordering::Relaxed) < 32)?;
    assert_eq!(
        run.as_ref().map(QueryRun::len),
        Some(1),
        "an early witness ends the walk"
    );
    Ok(())
}

#[test]
fn cancellation_counts_elements_a_type_selector_skips() -> TestResult {
    let bytes = DataForgeBuilder::new()
        .structure(TestStruct::new("Part").field("ready", TestType::Bool))
        .structure(TestStruct::new("Special").parent("Part"))
        .structure(
            TestStruct::new("Station").array("items", TestType::StrongPointer("Part".to_owned())),
        )
        .record(
            TestRecord::new(1, "Station", "Station.Sample", "synthetic/station.xml").value(
                "items",
                TestValue::Array(
                    (0..4097)
                        .map(|_| TestValue::Struct(TestInstance::new("Part")))
                        .collect(),
                ),
            ),
        )
        .build();
    let forge = DataForge::open(&bytes)?;
    let compiled =
        Query::parse("from Station where items[Special].ready = true")?.compile(&forge)?;
    assert!(compiled.run()?.is_empty(), "no Part is Special");
    let calls = AtomicUsize::new(0);
    assert!(
        compiled
            .run_while(|| calls.fetch_add(1, Ordering::Relaxed) < 32)?
            .is_none(),
        "skipped elements remain cancellable"
    );
    Ok(())
}

#[test]
fn the_first_error_uses_record_path_order_rather_than_definition_order() -> TestResult {
    let bytes = DataForgeBuilder::new()
        .structure(TestStruct::new("Station").field("ready", TestType::Bool))
        .record(
            TestRecord::new(1, "Station", "Station.Zulu", "synthetic/zulu.xml")
                .value("ready", TestValue::Raw(vec![2])),
        )
        .record(
            TestRecord::new(2, "Station", "Station.Alpha", "synthetic/alpha.xml")
                .value("ready", TestValue::Raw(vec![3])),
        )
        .build();
    let forge = DataForge::open(&bytes)?;
    let compiled = Query::parse("from Station where ready = true")?.compile(&forge)?;
    let error = compiled.run().err().ok_or("both values are unreadable")?;
    assert_eq!(
        error.record_index(),
        1,
        "the first path is the second definition"
    );
    assert_eq!(
        error.record_path(),
        "synthetic/alpha",
        "path order determines the first error"
    );
    assert_eq!(
        error.value_error(),
        &ValueError::InvalidBool { byte: 3 },
        "the error retains that Record's value"
    );
    Ok(())
}

#[test]
fn an_unreadable_projected_cell_does_not_fail_a_decidable_selection() -> TestResult {
    let bytes = query_selection_sample::database().build();
    let forge = DataForge::open(&bytes)?;
    let compiled = Query::parse("from Station select ready where id = 7")?.compile(&forge)?;
    let run = compiled.run()?;
    let rows = run.rows(0..1);
    let row = rows.first().ok_or("the known id selects the Station")?;
    let cells = row.cells().map_err(ToString::to_string)?;
    assert_eq!(
        cells.first(),
        Some(&Some(Value::Error(ValueError::InvalidBool { byte: 2 }))),
        "an unreadable projection retains its error cell"
    );
    Ok(())
}
