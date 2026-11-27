// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::{LoadoutDocumentKind, LoadoutFileObservation, LoadoutPathOperation};

use crate::loadout_request_authorship_support as request_source;
use crate::loadout_request_authorship_support::{EventKind, State, TestResult};

pub(super) fn verify_vehicle_receipts(state: &State, patch: &str) -> TestResult {
    let reads: Vec<_> = state
        .events
        .iter()
        .filter(|event| event.kind == EventKind::Read)
        .collect();
    let reused: Vec<_> = state
        .events
        .iter()
        .filter(|event| event.kind == EventKind::Reused)
        .collect();
    assert_eq!(
        reads.len(),
        2,
        "the actual base and explicit patch are read once"
    );
    assert_eq!(
        reused.len(),
        2,
        "the second caller reuses both original operations"
    );
    for event in &reads {
        request_source::assert_current_caller(state, event, 0)?;
    }
    for event in &reused {
        request_source::assert_current_caller(state, event, 1)?;
        let original = reads
            .iter()
            .find(|read| read.observation.is_same_operation(&event.observation))
            .ok_or("the reused original candidate is absent")?;
        assert_eq!(
            event.request.selection(),
            original.request.selection(),
            "the cached candidate is never relabelled as today's definition getter"
        );
    }
    assert!(
        reads.iter().any(
            |event| event.observation.kind() == LoadoutDocumentKind::VehiclePatch
                && event.request.value() == patch
        ),
        "the actual explicit patch path reaches its own read"
    );
    assert!(
        state.derived.contains(&LoadoutPathOperation::VehiclePatch),
        "the explicit patch's actual builder emits its source receipt"
    );
    let base = reads
        .iter()
        .find(|event| event.observation.kind() == LoadoutDocumentKind::VehicleImplementation)
        .ok_or("the original base read is absent")?;
    verify_vehicle_attributes(state, &base.observation)
}

fn verify_vehicle_attributes(state: &State, base: &LoadoutFileObservation) -> TestResult {
    let [file, name] = state.xml.as_slice() else {
        return Err("each selected attribute is observed once across both callers".into());
    };
    assert_eq!(
        (
            file.name.as_str(),
            file.value.as_str(),
            file.selection.operation()
        ),
        (
            "patchFile",
            "Modifications/shared_patch",
            LoadoutPathOperation::Copy
        ),
        "the original patch filename remains the path input"
    );
    assert_eq!(
        (
            name.name.as_str(),
            name.value.as_str(),
            name.selection.operation()
        ),
        (
            "name",
            "test",
            LoadoutPathOperation::VehicleModificationName
        ),
        "the actual selected XML name supplies the selector"
    );
    assert!(
        file.observation.is_same_operation(base),
        "the patch filename comes from the actual base read"
    );
    assert!(
        name.observation.is_same_operation(base),
        "the Modification selector comes from the actual base read"
    );
    assert_eq!(
        file.node, name.node,
        "both attributes come from the same original node"
    );
    Ok(())
}
