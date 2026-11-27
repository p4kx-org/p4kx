// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_dataforge::{DataForge, DataForgeError, Record, TreeBuilder};

use super::{
    COMPONENT, OTHER, TestResult, UNPOPULATED, entity, forge_builder, forge_bytes, instance,
};

const COMPONENTS: &str = "Components";
const INVALID_INSTANCE: u64 = 77;

#[test]
fn strict_probe_checks_all_slots_after_a_positive_match() -> TestResult {
    let bytes = forge_bytes();
    let forge = DataForge::open(&bytes)?;
    let probe = forge
        .strict_pointer_probe(COMPONENTS, COMPONENT)
        .ok_or("the exact populated component family")?;
    let mut trees = TreeBuilder::new();
    for case in [1, 12] {
        assert!(
            trees.holds(&probe, &record(&forge, case)?)?,
            "a valid exact component survives known other-family slots in case {case}"
        );
    }
    for case in [7, 11] {
        assert!(
            matches!(
                trees.holds(&probe, &record(&forge, case)?),
                Err(DataForgeError::IndexOutOfRange {
                    part: "strong pointer pool",
                    target: "struct instance",
                    target_index: INVALID_INSTANCE,
                    ..
                })
            ),
            "an exact-family pointer to a missing instance fails in case {case}"
        );
    }
    for case in [6, 14] {
        assert!(
            matches!(
                trees.holds(&probe, &record(&forge, case)?),
                Err(DataForgeError::IndexOutOfRange {
                    part: "strong pointer pool",
                    target: "struct",
                    target_index: 99,
                    ..
                })
            ),
            "an unclassifiable pointer fails in case {case}"
        );
    }
    assert!(
        !trees.holds(&probe, &record(&forge, 13)?)?,
        "a known other family with a bad instance has no selected component"
    );
    Ok(())
}

#[test]
fn strict_probe_distinguishes_absence_and_unpopulated_declared_types() -> TestResult {
    let bytes = forge_bytes();
    let forge = DataForge::open(&bytes)?;
    assert!(
        forge.strict_pointer_probe(COMPONENTS, "Missing").is_none(),
        "readable declarations prove genuine absence"
    );
    assert!(
        forge.pointer_probe(COMPONENTS, UNPOPULATED).is_none(),
        "legacy matching skips declarations without instances"
    );
    let probe = forge
        .strict_pointer_probe(COMPONENTS, UNPOPULATED)
        .ok_or("the unpopulated family can still be named by a pointer")?;
    let mut trees = TreeBuilder::new();
    assert!(
        !trees.holds(&probe, &record(&forge, 4)?)?,
        "an unreferenced empty declaration has no selected component"
    );
    assert!(
        matches!(
            trees.holds(&probe, &record(&forge, 15)?),
            Err(DataForgeError::IndexOutOfRange {
                target: "struct instance",
                target_index: 0,
                ..
            })
        ),
        "a pointer to a declared but unpopulated exact type is a fault"
    );
    Ok(())
}

#[test]
fn strict_probe_uses_only_reached_unreadable_declarations() -> TestResult {
    let mut bytes = forge_bytes();
    let offset = {
        let forge = DataForge::open(&bytes)?;
        let name = forge
            .structs()
            .find_map(|item| item.name().ok().filter(|name| *name == OTHER))
            .ok_or("the original other type")?;
        name.as_ptr()
            .addr()
            .checked_sub(bytes.as_ptr().addr())
            .ok_or("the original type name is inside the fixture")?
    };
    let first = bytes.get_mut(offset).ok_or("the original type name byte")?;
    assert_eq!(*first, b'O', "the edit addresses the other type name");
    *first = 0xff;
    let forge = DataForge::open(&bytes)?;
    let mut trees = TreeBuilder::new();
    for family in [COMPONENT, "Missing"] {
        let probe = forge
            .strict_pointer_probe(COMPONENTS, family)
            .ok_or("the corrupt declaration leaves candidate classification open")?;
        assert!(
            !trees.holds(&probe, &record(&forge, 4)?)?,
            "an unreadable declaration never reached by this owner does not poison its result"
        );
        assert!(
            matches!(
                trees.holds(&probe, &record(&forge, 12)?),
                Err(DataForgeError::InvalidString { .. })
            ),
            "a reached unreadable type cannot be excluded even after evaluating a valid sibling"
        );
    }
    Ok(())
}

fn record<'source>(forge: &'source DataForge<'source>, case: u32) -> TestResult<Record<'source>> {
    let name = format!("Entity.E{case}");
    forge
        .records()
        .find(|record| record.name() == Ok(name.as_str()))
        .ok_or_else(|| "the physical probe fixture owner".into())
}

#[test]
fn strict_probe_reports_missing_owner_storage_at_the_original_record() -> TestResult {
    let bytes = forge_builder()
        .record(entity(16, vec![instance(COMPONENT)]).with_instance_index(u16::MAX))
        .build();
    let forge = DataForge::open(&bytes)?;
    let owner = record(&forge, 16)?;
    let legacy = forge
        .pointer_probe(COMPONENTS, COMPONENT)
        .ok_or("the legacy probe for the fixture component")?;
    let strict = forge
        .strict_pointer_probe(COMPONENTS, COMPONENT)
        .ok_or("the strict probe for the fixture component")?;
    let expected = DataForgeError::IndexOutOfRange {
        part: "Record",
        index: owner.index() as usize,
        target: "instance",
        target_index: u64::from(u16::MAX),
    };
    assert_eq!(
        TreeBuilder::new().holds(&legacy, &owner),
        Ok(false),
        "the legacy probe does not change missing-owner behavior"
    );
    assert_eq!(
        TreeBuilder::new().holds(&strict, &owner),
        Err(expected),
        "the reached Record advertises the array but its original instance is missing"
    );
    Ok(())
}
