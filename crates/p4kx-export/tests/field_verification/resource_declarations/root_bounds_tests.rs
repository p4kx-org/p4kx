// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::Cell;

use p4kx_extraction::DataForge;
use p4kx_testkit::{DataForgeBuilder, TestStruct};

use super::super::error::{Error, Kind};
use super::Scope;

fn database(count: usize) -> Vec<u8> {
    (0..count)
        .fold(DataForgeBuilder::new(), |builder, index| {
            builder.structure(TestStruct::new(format!("MissionPropertyValue_{index:05}")))
        })
        .build()
}

#[test]
fn matching_root_limit_is_checked_before_the_next_name_clone() -> super::super::Result<()> {
    let bytes = database(super::MAX_SELECTED + 2);
    let forge = DataForge::open(&bytes)?;
    let definitions = super::definitions(&forge, &|| false)?;
    assert_eq!(
        definitions.len(),
        super::MAX_SELECTED + 2,
        "the fixture really exceeds the requested root bound"
    );
    let polls = Cell::new(0_usize);
    let result = super::roots(&definitions, Scope::MissionProperties, &|| {
        polls.set(polls.get() + 1);
        false
    });
    let error = result.err().ok_or("the root limit was ignored")?;
    let error = error
        .downcast_ref::<Error>()
        .ok_or("unexpected root error")?;
    assert_eq!(
        error.kind,
        Kind::Bounds,
        "root overflow is a typed bound failure"
    );
    assert_eq!(
        polls.get(),
        super::MAX_SELECTED + 1,
        "selection stops on the overflowing candidate before cloning its name or scanning later roots"
    );
    Ok(())
}

#[test]
fn root_selection_polls_cancellation_before_collecting_each_name() -> super::super::Result<()> {
    let bytes = database(10);
    let forge = DataForge::open(&bytes)?;
    let definitions = super::definitions(&forge, &|| false)?;
    let polls = Cell::new(0_usize);
    let result = super::roots(&definitions, Scope::MissionProperties, &|| {
        polls.set(polls.get() + 1);
        polls.get() == 3
    });
    let error = result.err().ok_or("root cancellation was ignored")?;
    let error = error
        .downcast_ref::<Error>()
        .ok_or("unexpected root error")?;
    assert_eq!(
        error.kind,
        Kind::Cancelled,
        "root selection preserves cancellation"
    );
    assert_eq!(
        polls.get(),
        3,
        "no later root name is scanned or cloned after cancellation"
    );
    Ok(())
}
