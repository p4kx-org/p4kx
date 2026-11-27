// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_dataforge::query::{MAX_PATH_VALUES, Query, QueryByteUse, QuerySource, Row};
use p4kx_dataforge::{DataForge, Value, ValueError};
use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};

use super::bind_cell_inputs;
use crate::{InputScope, SourceInput, SourceLocation, SourceUse, SourceUseRole, SourceUses};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

#[test]
fn observed_pointer_cells_keep_the_real_instance_prefix_and_bound_failures() -> TestResult {
    let bytes = fixture();
    let forge = DataForge::open(&bytes)?;
    let query = Query::parse("from Station select good.ready, bad.ready")?;
    let compiled = query.compile(&forge)?;
    let rows = compiled.run()?.rows_observed(0..1);
    let [row] = rows.as_slice() else {
        return Err("expected one physical query row".into());
    };
    let [available, unavailable] = row.cell_sources().unwrap_or_default() else {
        return Err("expected original routes for both query cells".into());
    };

    let uses = SourceUses::with_compact_use_refs();
    let scope = uses.observe(SourceInput::Unknown, InputScope::Payload)?;
    let observed = bind_cell_inputs(&forge, &scope, available.traversed())?;
    assert!(
        observed.is_complete(),
        "the readable pointer traversal has exact inputs"
    );
    assert_instance_prefix(&forge, observed.uses())?;

    let attempted = bind_cell_inputs(&forge, &scope, unavailable.traversed())?;
    assert!(
        !attempted.is_complete(),
        "unavailable target bytes make the observed cell incomplete"
    );
    assert!(
        attempted
            .uses()
            .iter()
            .any(|input| input.role() == SourceUseRole::Attempted),
        "the failed traversal keeps its actual readable attempted operand"
    );
    Ok(())
}

#[test]
fn projected_row_budget_preserves_partial_evidence_and_marks_the_cutoff() -> TestResult {
    let count = MAX_PATH_VALUES / 2 + 1;
    let bytes = DataForgeBuilder::new()
        .structure(TestStruct::new("TagSet").array("tags", TestType::Int32))
        .record(
            TestRecord::new(1, "TagSet", "TagSet.Source", "synthetic/tags.xml")
                .value("tags", TestValue::Array(vec![TestValue::Int32(7); count])),
        )
        .build();
    let forge = DataForge::open(&bytes)?;
    let query = Query::parse("from TagSet select tags[*], tags[*]")?;
    let shared = query.compile(&forge)?;
    assert!(
        !shared.shared_prefixes().is_empty(),
        "both columns replay an array"
    );
    let separate = query.compile(&forge)?.without_shared_prefixes();
    for compile in [&shared, &separate] {
        let run = compile.run()?;
        let observed = run.rows_observed(0..1);
        let ordinary = run.rows(0..1);
        let ([row], [plain]) = (observed.as_slice(), ordinary.as_slice()) else {
            return Err("a single budget-controlled fixture Record".into());
        };
        assert_budget_outcome(row, count)?;
        assert_budget_outcome(plain, count)?;
        assert!(
            plain.cell_sources().is_none(),
            "ordinary output allocates no sources"
        );
        let [available, cutoff] = row.cell_sources().unwrap_or_default() else {
            return Err("both projected cell traces".into());
        };
        let uses = SourceUses::with_compact_use_refs();
        let scope = uses.observe(SourceInput::Unknown, InputScope::Payload)?;
        assert!(bind_cell_inputs(&forge, &scope, available.traversed())?.is_complete());
        let cutoff_inputs = bind_cell_inputs(&forge, &scope, cutoff.traversed())?;
        assert!(
            !cutoff_inputs.is_complete(),
            "budget interruption leaves an incomplete cell"
        );
        assert_eq!(
            cutoff_inputs.uses().len(),
            2,
            "only the actual array header and pool bound"
        );
        assert!(
            cutoff.traversed().any(|source| matches!(
                source,
                QuerySource::RowBudgetExhausted {
                    limit: MAX_PATH_VALUES,
                    ..
                }
            )),
            "the cutoff has no fabricated physical operand"
        );
    }
    Ok(())
}

fn assert_budget_outcome(row: &Row<'_>, count: usize) -> TestResult {
    let [
        Some(Value::Array(first)),
        Some(Value::Error(ValueError::PathTooWide { limit, .. })),
    ] = row.cells().map_err(ToString::to_string)?
    else {
        return Err("the first array is read and the second exhausts the same row budget".into());
    };
    assert_eq!(first.len(), count, "the first array keeps every value");
    assert_eq!(
        *limit, MAX_PATH_VALUES,
        "the second retains the canonical error"
    );
    Ok(())
}

#[test]
fn projected_text_binds_original_text_and_retains_unavailable_slots() -> TestResult {
    let bytes = text_fixture();
    let forge = DataForge::open(&bytes)?;
    let query = Query::parse("from Card select title, bad, captions[*]")?;
    let compiled = query.compile(&forge)?;
    let run = compiled.run()?;
    let observed = run.rows_observed(0..1);
    let ordinary = run.rows(0..1);
    let ([row], [plain]) = (observed.as_slice(), ordinary.as_slice()) else {
        return Err("expected the one fixture Record in both modes".into());
    };
    assert_eq!(
        format!("{:?}", row.cells()),
        format!("{:?}", plain.cells()),
        "source reading preserves projected output in both modes"
    );
    assert!(plain.cell_sources().is_none() && plain.root_sources().is_none());

    let [title, bad, captions] = row.cell_sources().unwrap_or_default() else {
        return Err("expected direct text, direct bad slot and pooled text".into());
    };
    let [caption, bad_caption] = captions.values() else {
        return Err("expected both array element routes".into());
    };
    let uses = SourceUses::with_compact_use_refs();
    let scope = uses.observe(SourceInput::Unknown, InputScope::Payload)?;
    for original in [title.traversed().collect::<Vec<_>>().as_slice(), caption] {
        let bound = bind_cell_inputs(&forge, &scope, original.iter().copied())?;
        assert!(bound.is_complete(), "the readable text source is complete");
        assert_text_operands(&bytes, original)?;
    }
    for original in [bad.traversed().collect::<Vec<_>>().as_slice(), bad_caption] {
        let bound = bind_cell_inputs(&forge, &scope, original.iter().copied())?;
        assert!(
            !bound.is_complete(),
            "no text exists at the original offset"
        );
        assert!(original.contains(&QuerySource::TextLength));
        let attempted = text_input(&bytes, original, QueryByteUse::AttemptedValue)?;
        assert_eq!(
            attempted,
            u32::MAX.to_le_bytes(),
            "the actual corrupt slot remains"
        );
    }
    Ok(())
}

fn assert_text_operands(bytes: &[u8], observations: &[QuerySource]) -> TestResult {
    assert!(observations.contains(&QuerySource::TextLength));
    assert_eq!(
        text_input(bytes, observations, QueryByteUse::Text)?,
        b"Visible",
        "the borrowed original text is a contributor"
    );
    assert_eq!(
        text_input(bytes, observations, QueryByteUse::TextTerminator)?,
        b"\0",
        "the original terminating byte is a selector"
    );
    Ok(())
}

fn text_input<'a>(
    bytes: &'a [u8],
    observations: &[QuerySource],
    want: QueryByteUse,
) -> TestResult<&'a [u8]> {
    let span = observations.iter().find_map(|source| match *source {
        QuerySource::Bytes {
            start, end, kind, ..
        } if kind == want => {
            let start = usize::try_from(start).ok()?;
            let end = usize::try_from(end).ok()?;
            bytes.get(start..end)
        }
        _ => None,
    });
    span.ok_or_else(|| format!("no original input for {want:?}").into())
}

fn text_fixture() -> Vec<u8> {
    let malformed = TestValue::Raw(u32::MAX.to_le_bytes().to_vec());
    DataForgeBuilder::new()
        .structure(
            TestStruct::new("Card")
                .field("title", TestType::String)
                .field("bad", TestType::String)
                .array("captions", TestType::String),
        )
        .record(
            TestRecord::new(1, "Card", "Card.Source", "synthetic/card.xml")
                .value("title", TestValue::text("Visible"))
                .value("bad", malformed.clone())
                .value(
                    "captions",
                    TestValue::Array(vec![TestValue::text("Visible"), malformed]),
                ),
        )
        .build()
}

fn assert_instance_prefix(forge: &DataForge<'_>, inputs: &[SourceUse]) -> TestResult {
    let target = forge
        .data_mappings()
        .find(|mapping| mapping.struct_index == 1)
        .ok_or("target physical mapping")?;
    assert!(
        contains(
            inputs,
            "dataforgeStructDefinition",
            "size",
            &[0],
            SourceUseRole::Selector
        ) && contains(
            inputs,
            "dataforgeStructDefinition",
            "size",
            &[1],
            SourceUseRole::Selector
        ) && contains(
            inputs,
            "dataforgeDataMapping",
            "instanceCount",
            &[u64::from(target.index)],
            SourceUseRole::Selector
        ),
        "the earlier block and target size and count determine the actual extent"
    );
    assert!(
        !contains(
            inputs,
            "dataforgeStructDefinition",
            "size",
            &[2],
            SourceUseRole::Selector
        ),
        "the later block is outside the physical mapping prefix"
    );

    Ok(())
}

#[test]
fn only_actual_count_inputs_and_bound_reference_attempts_are_retained() -> TestResult {
    let bytes = fixture();
    let forge = DataForge::open(&bytes)?;
    let uses = SourceUses::with_compact_use_refs();
    let scope = uses.observe(SourceInput::Unknown, InputScope::Payload)?;
    let count = bind_cell_inputs(
        &forge,
        &scope,
        [QuerySource::InstanceBlockBound { structure: 1 }],
    )?;
    assert!(
        count.is_complete(),
        "the actual count does not require a physical extent"
    );
    assert_eq!(
        count.uses().len(),
        2,
        "only the matching original mapping words"
    );
    assert!(
        count
            .uses()
            .iter()
            .all(|source| matches!(source.description().location(),
                SourceLocation::Subresource { namespace, .. } if namespace == "dataforgeDataMapping"
            )),
        "size and mapping prefix were not consulted for the count"
    );

    let gap = bind_cell_inputs(
        &forge,
        &scope,
        [
            QuerySource::ReferenceGap {
                record: 1,
                ordinal: Some(3),
            },
            QuerySource::AncestorGap { index: 1 },
        ],
    )?;
    assert!(
        !gap.is_complete(),
        "an unmapped ancestry outcome prevents complete evidence"
    );
    assert_eq!(
        gap.uses().len(),
        1,
        "no source coordinate is fabricated for unknown ancestry"
    );
    assert!(
        contains(
            gap.uses(),
            "dataforgeReferenceIndex",
            "unreadableValue",
            &[1, 3],
            SourceUseRole::Attempted
        ),
        "the known gap is attempted evidence"
    );
    Ok(())
}

fn contains(
    sources: &[SourceUse],
    namespace: &str,
    field: &str,
    indices: &[u64],
    role: SourceUseRole,
) -> bool {
    sources.iter().any(|source| {
        source.role() == role
            && matches!(
                source.description().location(),
                SourceLocation::Subresource { namespace: observed, field: label, indices: at }
                    if observed == namespace && label == field && at.as_slice() == indices
            )
    })
}

fn fixture() -> Vec<u8> {
    DataForgeBuilder::new()
        .structure(TestStruct::new("Before").field("offset", TestType::Double))
        .structure(TestStruct::new("Target").field("ready", TestType::Bool))
        .structure(TestStruct::new("After").field("later", TestType::Int64))
        .structure(
            TestStruct::new("Station")
                .field("good", TestType::StrongPointer("Target".into()))
                .field("bad", TestType::StrongPointer("Target".into())),
        )
        .record(TestRecord::new(
            1,
            "Before",
            "Earlier",
            "synthetic/before.xml",
        ))
        .record(
            TestRecord::new(2, "Target", "Ready", "synthetic/target.xml")
                .value("ready", TestValue::Bool(true)),
        )
        .record(TestRecord::new(3, "After", "Later", "synthetic/after.xml"))
        .record(
            TestRecord::new(4, "Station", "Station", "synthetic/station.xml")
                .value("good", pointer(0))
                .value("bad", pointer(99)),
        )
        .build()
}

fn pointer(instance: u32) -> TestValue {
    let mut bytes = 1_u32.to_le_bytes().to_vec();
    bytes.extend_from_slice(&instance.to_le_bytes());
    TestValue::Raw(bytes)
}
