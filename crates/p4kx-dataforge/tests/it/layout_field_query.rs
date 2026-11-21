// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_dataforge::query::{Query, QueryErrorKind};
use p4kx_dataforge::{DataForge, DataForgeError, Value};

use crate::layout_field_fixture::{self as fixture, DERIVED, TestResult};
use crate::layout_field_original as original;
use crate::layout_field_raw as raw;

#[test]
fn a_query_projects_original_slots_after_the_inherited_u16_boundary() -> TestResult {
    projection(32_769)
}

#[test]
fn the_existing_inherited_boundary_keeps_the_same_query_values() -> TestResult {
    projection(32_768)
}

fn projection(own: usize) -> TestResult {
    let bytes = fixture::database(own, 0)?;
    raw::original(&bytes, own, 0)?;
    let forge = DataForge::open(&bytes)?;
    let record = original::record(&forge, own, 0)?;
    let last = own - 1;
    let text = format!("from {DERIVED} select `base-0`, `base-{last}`, `own-0`, `own-{last}`");
    let compiled = Query::parse(&text)?.compile(&forge)?;
    let run = compiled.run()?;
    assert_eq!(
        run.len(),
        1,
        "the actual descendant Record is selected once"
    );
    let rows = run.rows(0..run.len());
    let row = rows.first().ok_or("the original query row is absent")?;
    assert_eq!(
        row.record().index(),
        record.index(),
        "projection keeps the actual original Record"
    );
    assert_eq!(
        row.record_path(),
        record.record_path()?,
        "projection keeps the original path"
    );
    assert_eq!(
        row.cells().map_err(ToString::to_string)?,
        &[
            Some(Value::UInt8(11)),
            Some(Value::UInt8(22)),
            Some(Value::UInt8(33)),
            Some(Value::UInt8(44)),
        ],
        "the first and last inherited and own slots keep their exact source bytes"
    );
    assert_eq!(
        row.unresolved_references(),
        0,
        "scalar slot projection introduces no unresolved target"
    );
    Ok(())
}

#[test]
fn a_query_keeps_the_specific_wide_layout_error() -> TestResult {
    layout_failure(32_769)
}

#[test]
fn a_query_keeps_the_same_specific_small_layout_error() -> TestResult {
    layout_failure(2)
}

fn layout_failure(own: usize) -> TestResult {
    let bytes = fixture::database(own, 1)?;
    raw::original(&bytes, own, 1)?;
    let forge = DataForge::open(&bytes)?;
    original::record(&forge, own, 1)?;
    let query = Query::parse(&format!("from {DERIVED} select `own-0`"))?;
    let error = query
        .compile(&forge)
        .err()
        .ok_or("the actual padded layout must remain invalid")?;
    assert_eq!(
        error.kind(),
        &QueryErrorKind::DataForge(DataForgeError::StructLayout {
            struct_index: 1,
            declared: u32::try_from(2 * own + 1)?,
            fields: u64::try_from(2 * own)?,
        }),
        "schema preparation retains the actual source error instead of a field-count policy"
    );
    Ok(())
}
